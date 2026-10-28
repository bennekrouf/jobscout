//! What JobScout Pro adds: more searches a month. The window, the badge and
//! the licence checks are desktop-kit's, the same in every mayorana app.

use dioxus::prelude::*;

use crate::services::licence::{plan_searches, Status, StatusExt, FREE_SEARCHES};

/// The badge's tooltip while JobScout isn't Pro.
pub fn hint() -> String {
    format!("The free version runs {FREE_SEARCHES} searches a month; Pro plans run more")
}

/// What the free version does and what Pro adds, in the Pro window.
pub fn pitch() -> Element {
    rsx! {
        p {
            "The free version runs {FREE_SEARCHES} searches a month, in any of JobScout's countries. "
            "Scoring and export are never limited. "
            "Pro comes with a monthly plan on mayorana.ch: Basic runs {plan_searches(\"basic\")} searches a month, "
            "Pro {plan_searches(\"pro\")} and Max {plan_searches(\"max\")}."
        }
    }
}

/// The searches used this month, under any licence.
pub fn details(status: &Status, used: u32) -> Element {
    let limit = status.monthly_searches();
    let used = used.min(limit);
    let class = if used >= limit { "pro-out" } else { "" };
    rsx! {
        p { class,
            "{used} of {limit} searches used this month. A search from the sidebar or from Claude "
            "counts as one, and a watch's run when it finds new jobs; the count starts again on the 1st."
        }
    }
}
