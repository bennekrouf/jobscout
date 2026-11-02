//! The bar across the top (desktop-kit's header): the scoring model's flag,
//! the gear for Settings (where the profile and the watches are opened too),
//! MCP and Pro. The sidebar keeps to the search.

use desktop_kit::ui::{Header as KitHeader, ModelBadge};
use dioxus::prelude::*;

use crate::services::llm;
use crate::state::{AppState, Modal};

#[component]
pub fn Header() -> Element {
    let mut st = use_context::<AppState>();
    let pro = use_context::<desktop_kit::ui::Pro>();
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
            on_settings: move |_| st.modal.set(Modal::Settings),
            on_mcp: move |_| st.modal.set(Modal::Mcp),
            model: rsx! {
                ModelBadge {
                    label,
                    region,
                    title: "the model jobs are scored with \u{2014} click to change it",
                    onclick: move |_| st.modal.set(Modal::Settings),
                }
            },
            pro: rsx! { desktop_kit::ui::ProButton { pro, hint: super::pro::hint() } },
        }
    }
}
