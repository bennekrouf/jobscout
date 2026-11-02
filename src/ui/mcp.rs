//! How to use JobScout from Claude: what to paste, and where.
//!
//! The command is this app's own executable, read at runtime rather than
//! written down, so the snippet is right wherever it was installed — and in a
//! development build, which is what someone trying it from a checkout runs.

use dioxus::prelude::*;

use crate::state::{AppState, Modal};

#[component]
pub fn McpModal() -> Element {
    let mut st = use_context::<AppState>();
    rsx! {
        desktop_kit::ui::ConnectWindow {
            lead: "Ask Claude to search, read and score jobs, mark them Saved, Applied or Hidden, update your \
                   profile and manage your watches. It works on the same jobs as this window, and what it \
                   changes shows up here within a couple of seconds.",
            try_it: "\u{201c}What are my best matches this week?\u{201d}, or \u{201c}Search for data engineer jobs in \
                     Switzerland and score the new ones.\u{201d}",
            caution: "Scoring through Claude uses the model and key set in Settings, as it does here.",
            on_close: move |_| st.modal.set(Modal::None),
        }
    }
}
