//! The bar across the top: the name, and what opens in a window of its own —
//! profile, watches, Pro, MCP and settings — so the sidebar keeps to the
//! search.

use dioxus::prelude::*;

use crate::state::{AppState, Modal};

#[component]
pub fn Header() -> Element {
    let mut st = use_context::<AppState>();
    let profile_len = st.profile.read().trim().chars().count();
    let n_watches = st.watches.read().len();

    rsx! {
        header { class: "header",
            div { class: "brand",
                span { class: "brand-mark", "◎" }
                span { "JobScout" }
            }
            div { class: "spacer" }
            button {
                class: "btn",
                onclick: move |_| st.modal.set(Modal::Profile),
                if profile_len == 0 { "Set your profile…" } else { "Profile ({profile_len} chars)" }
            }
            button {
                class: "btn",
                onclick: move |_| st.modal.set(Modal::Watches),
                if n_watches == 0 { "Watches" } else { "Watches ({n_watches})" }
            }
            super::pro::ProButton {}
            button {
                class: "btn btn-ghost",
                title: "Use JobScout from Claude Desktop or Claude Code",
                onclick: move |_| st.modal.set(Modal::Mcp),
                "MCP"
            }
            button {
                class: "btn btn-ghost",
                onclick: move |_| st.modal.set(Modal::Settings),
                "Settings"
            }
        }
    }
}
