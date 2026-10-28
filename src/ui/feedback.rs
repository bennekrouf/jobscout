//! The way back to mayorana.ch from inside the app.
//!
//! People get JobScout as a download and may never see the site again: the
//! update banner only shows when there is an update. These links are always
//! in the sidebar, so anyone with an idea or a problem has somewhere to send it.

use dioxus::prelude::*;

use desktop_kit::feedback::contact_url;
use desktop_kit::feedback::site_url;

/// A quiet line of two links. Plain links rather than `open_url`: the webview
/// hands them to the system browser itself, `&` and all.
#[component]
pub fn FeedbackLine() -> Element {
    rsx! {
        div { class: "feedback-line",
            a {
                href: "{contact_url()}",
                target: "_blank",
                title: "Ideas, bugs, questions: send them to the person who makes JobScout",
                "Feedback"
            }
            span { " · " }
            a { href: "{site_url()}", target: "_blank", "mayorana.ch" }
        }
    }
}
