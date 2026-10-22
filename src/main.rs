mod ui;
mod url;

use std::error::Error;
use tao::{
    event::{ElementState, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    keyboard::{KeyCode, ModifiersState},
    window::{Fullscreen, WindowBuilder},
};
use wry::{http, WebViewBuilder};

#[cfg(target_os = "linux")]
use gtk::prelude::*;
#[cfg(target_os = "linux")]
use tao::platform::unix::WindowExtUnix;
#[cfg(target_os = "linux")]
use wry::WebViewBuilderExtUnix;

#[derive(Debug, Clone)]
pub enum UserEvent {
    Navigate(String),
    ToggleFullscreen,
    Close,
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

    let webview_builder = WebViewBuilder::new()
        .with_custom_protocol("focus".to_string(), |_id, _req| {
            http::Response::builder()
                .header("content-type", "text/html; charset=UTF-8")
                .body(std::borrow::Cow::Owned(
                    ui::get_start_page_html().into_bytes(),
                ))
                .unwrap()
        })
        .with_url("focus://localhost/index.html")
        .with_initialization_script(ui::INIT_SCRIPT)
        .with_focused(true)
        .with_devtools(true)
        .with_ipc_handler(move |request: http::Request<String>| {
            let body = request.body();
            if let Some(url_str) = body.strip_prefix("navigate:") {
                let _ = proxy.send_event(UserEvent::Navigate(url_str.to_string()));
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
        vbox.show_all();
        window.gtk_window().show_all();
        let _ = wv.focus();
        wv
    };

    #[cfg(not(target_os = "linux"))]
    let webview = webview_builder.build(&window)?;

    let mut modifiers = ModifiersState::empty();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::UserEvent(user_event) => match user_event {
                UserEvent::Navigate(raw_url) => {
                    let formatted_url = url::normalize_url(&raw_url);
                    if let Err(e) = webview.load_url(&formatted_url) {
                        eprintln!("Failed to load url {}: {:?}", formatted_url, e);
                    }
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
