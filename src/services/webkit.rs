//! The built-in browser backend: pages loaded in an offscreen WKWebView, the
//! same WebKit the app's window renders in. It ships with macOS, so it needs
//! nothing installed, unlike Camoufox.
//!
//! A real browser engine gets through Cloudflare's "Just a moment…"
//! interstitial on its own: the page runs its check and reloads with the
//! content. The loader waits for that, then hands back the page's HTML.
//!
//! WebKit has to be driven from the main thread. Dioxus polls the app's
//! tasks there, so a fetch started from the UI is fine; anywhere else (a
//! test, a spawned thread) gets an error instead of a crash.

use std::time::Duration;

use super::fetch::FetchError;

/// Longest a page may take, Cloudflare check included.
const LOAD_LIMIT: Duration = Duration::from_secs(45);
const POLL: Duration = Duration::from_millis(500);

/// Cloudflare's interstitial, by its title or its script.
fn is_challenge(title: &str, html: &str) -> bool {
    let t = title.trim().to_lowercase();
    ["just a moment", "un instant", "einen moment"]
        .iter()
        .any(|p| t.starts_with(p))
        || html.contains("_cf_chl_opt")
}

#[cfg(target_os = "macos")]
pub async fn get(url: &str) -> Result<String, FetchError> {
    mac::get(url).await
}

#[cfg(not(target_os = "macos"))]
pub async fn get(_url: &str) -> Result<String, FetchError> {
    Err(FetchError::Browser(
        "the built-in browser is macOS only; pick Camoufox in Settings".into(),
    ))
}

#[cfg(target_os = "macos")]
mod mac {
    use std::cell::{Cell, RefCell};
    use std::time::Instant;

    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2::{MainThreadMarker, MainThreadOnly};
    use objc2_foundation::{NSError, NSPoint, NSRect, NSSize, NSString, NSURLRequest, NSURL};
    use objc2_web_kit::{WKWebView, WKWebViewConfiguration, WKWebsiteDataStore};
    use tokio::sync::{oneshot, Mutex};

    use super::{is_challenge, FetchError, LOAD_LIMIT, POLL};

    thread_local! {
        /// One view for the app's lifetime. It keeps the default data store,
        /// so a passed Cloudflare check (a cookie) holds for later pages.
        static VIEW: RefCell<Option<Retained<WKWebView>>> = const { RefCell::new(None) };
    }

    /// Set on the page being left, so the new page can be told from it.
    const MARK_OLD: &str = "window.__jobscoutOld = true; ''";
    const STATE: &str =
        "JSON.stringify([!window.__jobscoutOld, document.readyState, document.title])";
    const HTML: &str = "document.documentElement.outerHTML";

    fn view(mtm: MainThreadMarker) -> Retained<WKWebView> {
        VIEW.with(|v| {
            v.borrow_mut()
                .get_or_insert_with(|| unsafe {
                    let config = WKWebViewConfiguration::new(mtm);
                    config.setWebsiteDataStore(&WKWebsiteDataStore::defaultDataStore(mtm));
                    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1280.0, 900.0));
                    WKWebView::initWithFrame_configuration(WKWebView::alloc(mtm), frame, &config)
                })
                .clone()
        })
    }

    pub async fn get(url: &str) -> Result<String, FetchError> {
        let mtm = MainThreadMarker::new().ok_or_else(|| {
            FetchError::Browser("the built-in browser only runs on the main thread".into())
        })?;
        // One view, so one page at a time.
        static ONE_AT_A_TIME: Mutex<()> = Mutex::const_new(());
        let _turn = ONE_AT_A_TIME.lock().await;

        let view = view(mtm);
        let target = NSURL::URLWithString(&NSString::from_str(url))
            .ok_or_else(|| FetchError::Browser(format!("not a URL: {url}")))?;
        let _ = eval(&view, MARK_OLD).await;
        unsafe { view.loadRequest(&NSURLRequest::requestWithURL(&target)) };

        let start = Instant::now();
        let mut challenged = false;
        while start.elapsed() < LOAD_LIMIT {
            tokio::time::sleep(POLL).await;
            let Ok(state) = eval(&view, STATE).await else {
                continue;
            };
            let Ok((new, ready, title)) = serde_json::from_str::<(bool, String, String)>(&state)
            else {
                continue;
            };
            if !new || ready == "loading" {
                continue;
            }
            let html = eval(&view, HTML).await.map_err(FetchError::Browser)?;
            if is_challenge(&title, &html) {
                challenged = true;
                continue;
            }
            return Ok(html);
        }
        unsafe { view.stopLoading() };
        Err(if challenged {
            // Never got through: a block, the same as an HTTP 403.
            FetchError::Status(403)
        } else {
            FetchError::Network(format!(
                "page did not load within {} s",
                LOAD_LIMIT.as_secs()
            ))
        })
    }

    /// Runs `js` in the page; its value must be a string.
    async fn eval(view: &WKWebView, js: &str) -> Result<String, String> {
        let (tx, rx) = oneshot::channel();
        let tx = Cell::new(Some(tx));
        let done = block2::RcBlock::new(move |value: *mut AnyObject, error: *mut NSError| {
            let out = match unsafe { error.as_ref() } {
                Some(e) => Err(e.localizedDescription().to_string()),
                None => unsafe { value.as_ref() }
                    .and_then(|v| v.downcast_ref::<NSString>())
                    .map(|s| s.to_string())
                    .ok_or_else(|| "script returned no string".to_string()),
            };
            if let Some(tx) = tx.take() {
                let _ = tx.send(out);
            }
        });
        unsafe { view.evaluateJavaScript_completionHandler(&NSString::from_str(js), Some(&done)) };
        rx.await.map_err(|_| "page went away".to_string())?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tells_the_interstitial_from_a_page() {
        assert!(is_challenge("Just a moment...", ""));
        assert!(is_challenge("", "<script>window._cf_chl_opt = {}</script>"));
        assert!(!is_challenge(
            "Rust Jobs in Singapore | Jobstreet",
            "<html>…</html>"
        ));
    }

    #[cfg(not(target_os = "macos"))]
    #[tokio::test]
    async fn other_platforms_get_a_clear_error() {
        assert!(matches!(
            get("https://example.com").await,
            Err(FetchError::Browser(_))
        ));
    }

    /// Off the main thread, as every test is: an error, never a crash.
    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn refuses_off_the_main_thread() {
        let r = get("https://example.com").await;
        assert!(matches!(r, Err(FetchError::Browser(e)) if e.contains("main thread")));
    }
}
