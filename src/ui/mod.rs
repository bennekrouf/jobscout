mod detail;
mod feedback;
mod flag;
mod header;
mod jobs;
mod mcp;
mod modals;
pub mod pro;
mod search;
mod theme;

use dioxus::prelude::*;

use crate::model::Settings;
use crate::services::store;
use crate::state::{drain_detail_queue, scheduler, AppState, Modal};

/// Which splitter is being dragged.
#[derive(Clone, Copy, PartialEq)]
enum Drag {
    Sidebar,
    List,
}

const SPLITTER: f64 = 5.0;

const CSS: &str = include_str!("../../assets/main.css");

#[component]
pub fn App() -> Element {
    let mut st = use_context_provider(AppState::new);
    // JobScout Pro: the plan sets how many searches a month JobScout runs.
    use_context_provider(|| pro::Pro {
        open: Signal::new(false),
        status: Signal::new(crate::services::licence::current()),
    });
    // The licence is checked with api0 at start, then once a day.
    let mut pro_status = use_context::<pro::Pro>().status;
    // The title bar follows the licence, however it changes.
    use_effect(move || {
        dioxus::desktop::window().set_title(&crate::window_title(&pro_status.read()));
    });
    use_future(move || async move {
        pro_status.set(crate::services::licence::refresh(true).await);
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
            pro_status.set(crate::services::licence::refresh(false).await);
        }
    });
    let modal = *st.modal.read();
    let mut drag = use_signal(|| Option::<Drag>::None);

    // Watches run on their own for as long as the app is open.
    use_future(move || scheduler(st));

    // What Claude changes through `jobscout mcp` shows up here.
    use_future(move || async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            st.reload_changed();
        }
    });

    // A newer release on mayorana.ch, offered in a banner. After the window
    // is up, never in its way; DISABLE_UPDATE_CHECK turns it off.
    let mut update = use_signal(|| Option::<crate::services::update_check::UpdateInfo>::None);
    let mut update_dismissed = use_signal(|| false);
    use_future(move || async move {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        if let Some(info) = crate::services::update_check::check().await {
            update.set(Some(info));
        }
    });

    // Fetch descriptions queued while busy, as soon as nothing else runs.
    use_effect(move || {
        let idle = st.busy.read().is_none();
        let pending = !st.detail_queue.read().is_empty();
        if idle && pending {
            spawn(drain_detail_queue(st));
        }
    });

    let (sidebar_w, list_w) = {
        let s = st.settings.read();
        (s.sidebar_width, s.list_width)
    };

    rsx! {
        style { {CSS} }
        theme::ThemeToggle {}
        if let (Some(info), false) = (update(), update_dismissed()) {
            div { class: "update-banner", role: "status",
                span { class: "update-text",
                    "JobScout "
                    strong { "{info.latest_version}" }
                    " is available (you have {env!(\"CARGO_PKG_VERSION\")})."
                }
                {
                    // A button, not a link: the URL must open in the browser,
                    // never in the app's own webview.
                    let url = info.release_url.clone();
                    rsx! {
                        button {
                            class: "btn btn-primary btn-small",
                            onclick: move |_| pro::open_url(&url),
                            "Download"
                        }
                    }
                }
                button {
                    class: "btn btn-ghost btn-small",
                    title: "Hide until the next start",
                    onclick: move |_| update_dismissed.set(true),
                    "Not now"
                }
            }
        }
        header::Header {}
        // Each column has a minimum, and the job pane gets what is left: on a
        // narrow window the list gives way before the job pane disappears.
        div {
            class: if drag.read().is_some() { "shell dragging" } else { "shell" },
            style: "grid-template-columns: minmax(200px, {sidebar_w}px) {SPLITTER}px minmax(280px, {list_w}px) {SPLITTER}px minmax(320px, 1fr);",
            // A key only takes effect in a list: in a list of one, a new
            // country mounts a fresh form instead of keeping the last
            // country's search in it.
            for country in [*st.country.read()] {
                search::SearchPanel { key: "{country.code()}" }
            }
            Splitter { which: Drag::Sidebar, drag }
            jobs::JobList {}
            Splitter { which: Drag::List, drag }
            detail::JobDetailPane {}
        }
        // While dragging, a full-window layer takes the mouse, so the drag
        // keeps going over the panes and stops wherever the button is let go.
        if drag.read().is_some() {
            div {
                class: "drag-layer",
                onmousemove: move |e| {
                    let x = e.client_coordinates().x;
                    let mut s = st.settings.write();
                    match *drag.peek() {
                        Some(Drag::Sidebar) => s.sidebar_width = x.clamp(200.0, 480.0),
                        Some(Drag::List) => {
                            s.list_width = (x - s.sidebar_width - SPLITTER).clamp(280.0, 1400.0)
                        }
                        None => {}
                    }
                },
                onmouseup: move |_| {
                    drag.set(None);
                    store::save_settings(&st.settings.read());
                },
                onmouseleave: move |_| {
                    drag.set(None);
                    store::save_settings(&st.settings.read());
                },
            }
        }
        StatusBar {}
        match modal {
            Modal::Profile => rsx! { modals::ProfileModal {} },
            Modal::Settings => rsx! { modals::SettingsModal {} },
            Modal::Watches => rsx! { modals::WatchesModal {} },
            Modal::Mcp => rsx! { mcp::McpModal {} },
            Modal::None => rsx! {},
        }
        pro::ProWindow {}
    }
}

#[component]
fn StatusBar() -> Element {
    let mut st = use_context::<AppState>();
    let mut open = use_signal(|| false);
    let busy = st.busy.read().clone();
    let log = st.log.read();
    let last = log.last().cloned().unwrap_or_else(|| "Ready.".into());

    rsx! {
        if *open.read() {
            div { class: "log-panel",
                for line in log.iter().rev() {
                    div { class: "log-line", "{line}" }
                }
            }
        }
        div { class: "statusbar",
            if let Some(what) = busy {
                span { class: "spinner" }
                span { class: "status-busy", "{what}" }
                button {
                    class: "btn btn-small btn-danger",
                    onclick: move |_| {
                        st.stop.set(true);
                        // Stop means stop: drop the queued fetches too.
                        st.detail_queue.set(vec![]);
                    },
                    "Stop"
                }
            }
            span { class: "status-last", "{last}" }
            button {
                class: "btn btn-small btn-ghost",
                onclick: move |_| { let v = *open.read(); open.set(!v); },
                if *open.read() { "Hide log" } else { "Log" }
            }
        }
    }
}

/// Score badge colour band.
pub fn score_class(score: u8) -> &'static str {
    match score {
        80.. => "score-high",
        50..=79 => "score-mid",
        _ => "score-low",
    }
}

/// A draggable column border. Double-click puts that column back to its
/// default width.
#[component]
fn Splitter(which: Drag, drag: Signal<Option<Drag>>) -> Element {
    let mut st = use_context::<AppState>();
    let active = *drag.read() == Some(which);
    rsx! {
        div {
            class: if active { "splitter on" } else { "splitter" },
            title: "Drag to resize · double-click to reset",
            onmousedown: move |e| {
                e.prevent_default();
                drag.set(Some(which));
            },
            ondoubleclick: move |_| {
                let d = Settings::default();
                {
                    let mut s = st.settings.write();
                    match which {
                        Drag::Sidebar => s.sidebar_width = d.sidebar_width,
                        Drag::List => s.list_width = d.list_width,
                    }
                }
                store::save_settings(&st.settings.read());
            },
        }
    }
}
