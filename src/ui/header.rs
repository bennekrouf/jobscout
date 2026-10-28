//! The bar across the top (desktop-kit's header): the name, then what opens in
//! a window of its own: profile, watches, the scoring model, settings, MCP and
//! Pro. The sidebar keeps to the search.

use desktop_kit::ui::{use_theme, Header as KitHeader, ModelBadge};
use dioxus::prelude::*;

use crate::services::llm;
use crate::state::{AppState, Modal};

#[component]
pub fn Header() -> Element {
    let mut st = use_context::<AppState>();
    let theme = use_theme();
    let pro = use_context::<desktop_kit::ui::Pro>();
    let profile_len = st.profile.read().trim().chars().count();
    let n_watches = st.watches.read().len();
    let (label, region) = {
        let s = st.settings.read();
        let region = llm::is_api0(&s.llm_base_url)
            .then(|| desktop_kit::region::find(&s.llm_model))
            .flatten();
        match region {
            Some(r) => (r.label.to_string(), Some(r.id)),
            None => (s.llm_model.clone(), None),
        }
    };

    rsx! {
        KitHeader {
            theme,
            on_settings: move |_| st.modal.set(Modal::Settings),
            on_mcp: move |_| st.modal.set(Modal::Mcp),
            actions: rsx! {
                button {
                    class: "kit-btn",
                    onclick: move |_| st.modal.set(Modal::Profile),
                    if profile_len == 0 { "Set your profile…" } else { "Profile ({profile_len} chars)" }
                }
                button {
                    class: "kit-btn",
                    onclick: move |_| st.modal.set(Modal::Watches),
                    if n_watches == 0 { "Watches" } else { "Watches ({n_watches})" }
                }
            },
            model: rsx! {
                ModelBadge {
                    label,
                    region,
                    title: "The model jobs are scored with \u{2014} click to change it",
                    onclick: move |_| st.modal.set(Modal::Settings),
                }
            },
            pro: rsx! { desktop_kit::ui::ProButton { pro, hint: super::pro::hint() } },
        }
    }
}
