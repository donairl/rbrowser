/// Default start page: minimalist dark mode focus screen.
pub const START_PAGE_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>Focus Browser</title>
  <style>
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      background-color: #0c0d0e;
      color: #e4e4e7;
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
      height: 100vh;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      user-select: none;
      overflow: hidden;
    }
    .container {
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 1.5rem;
      max-width: 500px;
      text-align: center;
    }
    .clock {
      font-size: 5rem;
      font-weight: 200;
      letter-spacing: -2px;
      color: #fafafa;
    }
    .brand {
      font-size: 1.1rem;
      font-weight: 500;
      letter-spacing: 0.2em;
      text-transform: uppercase;
      color: #71717a;
    }
    .hint-card {
      margin-top: 1rem;
      padding: 0.9rem 1.6rem;
      background: rgba(255, 255, 255, 0.05);
      border: 1px solid rgba(255, 255, 255, 0.1);
      border-radius: 9999px;
      display: flex;
      align-items: center;
      gap: 0.75rem;
      font-size: 0.95rem;
      color: #a1a1aa;
      cursor: pointer;
      transition: all 0.2s ease;
    }
    .hint-card:hover {
      background: rgba(255, 255, 255, 0.1);
      border-color: rgba(255, 255, 255, 0.2);
      color: #f4f4f5;
      transform: translateY(-1px);
    }
    kbd {
      background: #27272a;
      border: 1px solid #3f3f46;
      border-radius: 6px;
      padding: 0.2rem 0.5rem;
      font-size: 0.8rem;
      font-family: inherit;
      color: #f4f4f5;
      box-shadow: 0 1px 2px rgba(0,0,0,0.4);
    }
  </style>
</head>
<body>
  <div class="container">
    <div class="brand">Focus Browser</div>
    <div class="clock" id="clock">00:00</div>
    <div class="hint-card" onclick="window.__focusBrowserToggle && window.__focusBrowserToggle()">
      <span>Press <kbd>Ctrl</kbd> + <kbd>G</kbd> to go to URL</span>
    </div>
  </div>
  <script>
    function updateClock() {
      const now = new Date();
      const h = String(now.getHours()).padStart(2, '0');
      const m = String(now.getMinutes()).padStart(2, '0');
      const el = document.getElementById('clock');
      if (el) el.textContent = `${h}:${m}`;
    }
    updateClock();
    setInterval(updateClock, 1000);
  </script>
</body>
</html>
"#;

/// Injected script: attached to all loaded documents to provide the Ctrl+G overlay.
pub const INIT_SCRIPT: &str = r#"
(function() {
  if (window.__focus_browser_initialized) return;
  window.__focus_browser_initialized = true;

  const postIpc = (msg) => {
    try {
      if (window.ipc && typeof window.ipc.postMessage === 'function') {
        window.ipc.postMessage(msg);
      } else if (window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.ipc) {
        window.webkit.messageHandlers.ipc.postMessage(msg);
      }
    } catch (e) {
      console.error('IPC error:', e);
    }
  };

  let host = null;
  let backdrop = null;
  let input = null;

  function setupOverlay() {
    if (host && host.parentNode) return host;
    if (!host) {
      host = document.createElement('div');
      host.id = '__focus_browser_host__';
      host.style.position = 'fixed';
      host.style.top = '0';
      host.style.left = '0';
      host.style.width = '100vw';
      host.style.height = '100vh';
      host.style.zIndex = '2147483647';
      host.style.display = 'none';
      host.style.pointerEvents = 'none';

      const shadow = host.attachShadow({ mode: 'open' });
      shadow.innerHTML = `
        <style>
          * { box-sizing: border-box; margin: 0; padding: 0; }
          .backdrop {
            position: fixed;
            inset: 0;
            background: rgba(0, 0, 0, 0.55);
            backdrop-filter: blur(12px);
            -webkit-backdrop-filter: blur(12px);
            display: flex;
            justify-content: center;
            align-items: flex-start;
            padding-top: 15vh;
            opacity: 0;
            transition: opacity 0.15s ease-out;
            pointer-events: auto;
          }
          .backdrop.visible {
            opacity: 1;
          }
          .modal {
            width: min(640px, 92vw);
            background: #18181b;
            border: 1px solid #3f3f46;
            box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.75), 0 0 0 1px rgba(255, 255, 255, 0.08);
            border-radius: 14px;
            overflow: hidden;
            transform: translateY(-10px) scale(0.98);
            transition: transform 0.15s ease-out;
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
          }
          .backdrop.visible .modal {
            transform: translateY(0) scale(1);
          }
          .input-row {
            display: flex;
            align-items: center;
            padding: 0.9rem 1.1rem;
            gap: 0.75rem;
            background: #18181b;
          }
          .icon {
            color: #a1a1aa;
            display: flex;
            align-items: center;
            justify-content: center;
          }
          input {
            flex: 1;
            background: transparent;
            border: none;
            outline: none;
            color: #fafafa;
            font-size: 1.1rem;
            font-family: inherit;
          }
          input::placeholder {
            color: #71717a;
          }
          .footer {
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 0.6rem 1.1rem;
            background: #121214;
            border-top: 1px solid #27272a;
            font-size: 0.75rem;
            color: #71717a;
          }
          .hints {
            display: flex;
            gap: 0.8rem;
            align-items: center;
          }
          kbd {
            background: #27272a;
            border: 1px solid #3f3f46;
            border-radius: 4px;
            padding: 0.15rem 0.4rem;
            font-size: 0.7rem;
            color: #e4e4e7;
          }
        </style>
        <div class="backdrop" id="backdrop">
          <div class="modal">
            <div class="input-row">
              <div class="icon">
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <circle cx="11" cy="11" r="8"></circle>
                  <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
                </svg>
              </div>
              <input type="text" id="url-input" placeholder="Type URL (e.g. github.com) or search..." autocomplete="off" spellcheck="false" />
            </div>
            <div class="footer">
              <div class="hints">
                <span><kbd>Enter</kbd> Go</span>
                <span><kbd>Esc</kbd> Close</span>
                <span><kbd>F11</kbd> Fullscreen</span>
              </div>
              <span>Focus Browser</span>
            </div>
          </div>
        </div>
      `;

      backdrop = shadow.getElementById('backdrop');
      input = shadow.getElementById('url-input');

      backdrop.addEventListener('click', (e) => {
        if (e.target === backdrop) hide();
      });

      input.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') {
          e.preventDefault();
          const val = input.value.trim();
          if (val.length > 0) {
            hide();
            postIpc('navigate:' + val);
          }
        } else if (e.key === 'Escape') {
          e.preventDefault();
          hide();
        }
      });
    }

    const targetParent = document.body || document.documentElement;
    if (targetParent && !host.parentNode) {
      targetParent.appendChild(host);
    }
    return host;
  }

  function show() {
    setupOverlay();
    if (!host) return;
    const targetParent = document.body || document.documentElement;
    if (targetParent && !host.parentNode) {
      targetParent.appendChild(host);
    }
    host.style.display = 'block';
    host.style.pointerEvents = 'auto';
    requestAnimationFrame(() => {
      if (backdrop) backdrop.classList.add('visible');
      if (input) {
        const href = window.location ? window.location.href : '';
        if (href && !href.startsWith('about:') && !href.startsWith('focus:')) {
          input.value = href;
        } else {
          input.value = '';
        }
        input.focus();
        input.select();
      }
    });
  }

  function hide() {
    if (!host || !backdrop) return;
    backdrop.classList.remove('visible');
    if (input) input.blur();
    setTimeout(() => {
      host.style.display = 'none';
      host.style.pointerEvents = 'none';
    }, 150);
  }

  function toggle() {
    if (!host || host.style.display === 'none' || !backdrop || !backdrop.classList.contains('visible')) {
      show();
    } else {
      hide();
    }
  }

  window.__focusBrowserToggle = toggle;
  window.__focusBrowserHide = hide;
  window.__focusBrowserShow = show;

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', setupOverlay);
  } else {
    setupOverlay();
  }

  window.addEventListener('keydown', (e) => {
    const isG = e.key === 'g' || e.key === 'G' || e.code === 'KeyG' || e.keyCode === 71;
    if ((e.ctrlKey || e.metaKey) && isG) {
      e.preventDefault();
      e.stopPropagation();
      toggle();
    } else if (e.key === 'Escape') {
      if (host && host.style.display !== 'none') {
        e.preventDefault();
        e.stopPropagation();
        hide();
      }
    } else if (e.key === 'F11') {
      e.preventDefault();
      e.stopPropagation();
      postIpc('toggle_fullscreen');
    }
  }, true);
})();
"#;

pub fn get_start_page_html() -> String {
    START_PAGE_HTML.replace(
        "</body>",
        &format!("<script>{}</script></body>", INIT_SCRIPT),
    )
}
