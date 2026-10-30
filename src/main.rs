mod ui;
mod url;

use std::error::Error;
use tao::{
    event::{ElementState, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    keyboard::{KeyCode, ModifiersState},
    window::{Fullscreen, WindowBuilder},
};
use wry::{http, NewWindowResponse, WebViewBuilder};

#[cfg(target_os = "linux")]
use gtk::prelude::*;
#[cfg(target_os = "linux")]
use tao::platform::unix::WindowExtUnix;
#[cfg(target_os = "linux")]
use wry::{WebViewBuilderExtUnix, WebViewExtUnix};

const START_URL: &str = "focus://localhost/index.html";

#[derive(Debug, Clone)]
pub enum UserEvent {
    Navigate(String),
    Zoom(String),
    ToggleFullscreen,
    Close,
}

/// WebKit aborts the page process (frozen, unclickable page) when media starts
/// without gst-plugins-good's autoaudiosink, which lives in libgstautodetect.so.
// ponytail: checks common plugin dirs, not the GStreamer registry; unusual layouts
// count as missing and only lose video/audio.
#[cfg(target_os = "linux")]
fn has_autoaudiosink() -> bool {
    let triplet = format!("/usr/lib/{}-linux-gnu/gstreamer-1.0", std::env::consts::ARCH);
    ["GST_PLUGIN_PATH", "GST_PLUGIN_SYSTEM_PATH_1_0"]
        .into_iter()
        .filter_map(std::env::var_os)
        .flat_map(|v| std::env::split_paths(&v).collect::<Vec<_>>())
        .chain(["/usr/lib/gstreamer-1.0", "/usr/lib64/gstreamer-1.0", &triplet].map(Into::into))
        .any(|dir| dir.join("libgstautodetect.so").exists())
}

fn main() -> Result<(), Box<dyn Error>> {
    // WebKitGTK's DMABUF renderer crashes with Wayland "Error 71" (notably on NVIDIA).
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        // SAFETY: single-threaded at this point, before GTK/WebKit init.
        unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
    }

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();

    let window = WindowBuilder::new()
        .with_title("Focus Browser")
        .with_fullscreen(Some(Fullscreen::Borderless(None)))
        .build(&event_loop)?;

    let proxy = event_loop.create_proxy();
    let new_window_proxy = event_loop.create_proxy();

    let blocklist = url::load_blocklist();
    let nav_blocklist = blocklist.clone();

    let start_url = std::env::args()
        .nth(1)
        .map(|arg| url::normalize_url(&arg))
        .filter(|u| !url::is_blocked(u, &blocklist))
        .unwrap_or_else(|| START_URL.to_string());

    let webview_builder = WebViewBuilder::new()
        .with_custom_protocol("focus".to_string(), |_id, _req| {
            http::Response::builder()
                .header("content-type", "text/html; charset=UTF-8")
                .body(std::borrow::Cow::Owned(
                    ui::get_start_page_html().into_bytes(),
                ))
                .unwrap()
        })
        .with_initialization_script(ui::INIT_SCRIPT)
        .with_focused(true)
        .with_devtools(true)
        // Silently drops links, redirects and iframes to blocked sites.
        .with_navigation_handler(move |uri| !url::is_blocked(&uri, &nav_blocklist))
        // ponytail: target=_blank and window.open load in the same view; popup flows
        // relying on window.opener (some OAuth logins) break. Add a real popup window if needed.
        .with_new_window_req_handler(move |uri, _features| {
            if !uri.is_empty() && uri != "about:blank" {
                let _ = new_window_proxy.send_event(UserEvent::Navigate(uri));
            }
            NewWindowResponse::Deny
        })
        .with_ipc_handler(move |request: http::Request<String>| {
            let body = request.body();
            if let Some(url_str) = body.strip_prefix("navigate:") {
                let _ = proxy.send_event(UserEvent::Navigate(url_str.to_string()));
            } else if let Some(dir) = body.strip_prefix("zoom:") {
                let _ = proxy.send_event(UserEvent::Zoom(dir.to_string()));
            } else if body == "toggle_fullscreen" {
                let _ = proxy.send_event(UserEvent::ToggleFullscreen);
            } else if body == "close" {
                let _ = proxy.send_event(UserEvent::Close);
            }
        });

    #[cfg(target_os = "linux")]
    let webview = {
        let vbox = window.default_vbox().expect("default_vbox");
        let wv = webview_builder.build_gtk(vbox)?;
        if !has_autoaudiosink() {
            use webkit2gtk::{SettingsExt, WebViewExt};
            eprintln!("gst-plugins-good not found: video/audio disabled");
            if let Some(s) = WebViewExt::settings(&wv.webview()) {
                // Media alone is not enough: MSE/EME/capability probes still build GStreamer pipelines.
                s.set_enable_media(false);
                s.set_enable_webaudio(false);
                s.set_enable_mediasource(false);
                s.set_enable_encrypted_media(false);
                s.set_enable_media_capabilities(false);
                s.set_enable_media_stream(false);
                s.set_enable_webrtc(false);
            }
        }
        vbox.show_all();
        window.gtk_window().show_all();
        let _ = wv.focus();
        wv
    };

    #[cfg(not(target_os = "linux"))]
    let webview = webview_builder.build(&window)?;

    // Loaded after build so the media settings above apply from the first page.
    webview.load_url(&start_url)?;

    let mut modifiers = ModifiersState::empty();
    let mut zoom = 1.0_f64;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::UserEvent(user_event) => match user_event {
                UserEvent::Navigate(raw_url) => {
                    let formatted_url = url::normalize_url(&raw_url);
                    let result = if url::is_blocked(&formatted_url, &blocklist) {
                        webview.load_html(&ui::blocked_page_html(&formatted_url))
                    } else {
                        webview.load_url(&formatted_url)
                    };
                    if let Err(e) = result {
                        eprintln!("Failed to load url {}: {:?}", formatted_url, e);
                    }
                }
                UserEvent::Zoom(dir) => {
                    zoom = match dir.as_str() {
                        "in" => (zoom + 0.1).min(3.0),
                        "out" => (zoom - 0.1).max(0.3),
                        _ => 1.0,
                    };
                    let _ = webview.zoom(zoom);
                }
                UserEvent::ToggleFullscreen => {
                    let is_fullscreen = window.fullscreen().is_some();
                    if is_fullscreen {
                        window.set_fullscreen(None);
                    } else {
                        window.set_fullscreen(Some(Fullscreen::Borderless(None)));
                    }
                }
                UserEvent::Close => {
                    *control_flow = ControlFlow::Exit;
                }
            },
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    *control_flow = ControlFlow::Exit;
                }
                WindowEvent::ModifiersChanged(m) => {
                    modifiers = m;
                }
                WindowEvent::KeyboardInput {
                    event: key_event, ..
                } if key_event.state == ElementState::Pressed => {
                    let is_ctrl = modifiers.contains(ModifiersState::CONTROL)
                        || modifiers.contains(ModifiersState::SUPER);
                    if is_ctrl && key_event.physical_key == KeyCode::KeyG {
                        let _ = webview.evaluate_script(
                            "if (window.__focusBrowserToggle) { window.__focusBrowserToggle(); }",
                        );
                        let _ = webview.focus();
                    } else if key_event.physical_key == KeyCode::F11 {
                        let is_fullscreen = window.fullscreen().is_some();
                        if is_fullscreen {
                            window.set_fullscreen(None);
                        } else {
                            window.set_fullscreen(Some(Fullscreen::Borderless(None)));
                        }
                    } else if key_event.physical_key == KeyCode::Escape {
                        let _ = webview.evaluate_script(
                            "if (window.__focusBrowserHide) { window.__focusBrowserHide(); }",
                        );
                    }
                }
                _ => {}
            },
            _ => {}
        }
    });
}
