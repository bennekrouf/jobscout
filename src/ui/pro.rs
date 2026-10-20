//! The JobScout Pro window: what the free version does, the licence on this
//! computer, and where to paste a key.

use dioxus::prelude::*;

use crate::model::Country;
use crate::services::licence::{self, Status, FREE_WATCHES};
use crate::services::store;
use crate::state::AppState;

/// Shared by every part of the UI that can open the window or needs to know
/// whether the app is licensed. Provided once, by `App`.
#[derive(Clone, Copy)]
pub struct Pro {
    pub open: Signal<bool>,
    pub status: Signal<Status>,
}

/// Watches saved in every country: the loaded one from memory, the others
/// from disk.
pub fn watch_count(st: AppState) -> usize {
    let here = *st.country.read();
    Country::ALL
        .iter()
        .map(|&c| {
            if c == here {
                st.watches.read().len()
            } else {
                store::load_watches(c).len()
            }
        })
        .sum()
}

/// Opens `url` in the default browser.
pub fn open_url(url: &str) {
    use std::process::Command;
    let _ = if cfg!(target_os = "macos") {
        Command::new("open").arg(url).spawn()
    } else if cfg!(windows) {
        Command::new("cmd").args(["/c", "start", "", url]).spawn()
    } else {
        Command::new("xdg-open").arg(url).spawn()
    };
}

/// "Get Pro…" while free, "✦ PRO" once licensed; opens the window either way.
/// Nothing at all in a build that can't check licences.
#[component]
pub fn ProButton() -> Element {
    let mut pro = use_context::<Pro>();
    let status = pro.status.read().clone();
    let (label, class, title) = match &status {
        Status::Unavailable => return rsx! {},
        Status::Pro(l) => (
            "✦ PRO",
            "btn pro-btn active",
            format!("JobScout Pro, licensed to {}", l.email),
        ),
        Status::Renew(_) => (
            "✦ RENEW PRO",
            "btn pro-btn renew",
            "Your Pro licence doesn't cover this version".to_owned(),
        ),
        Status::Free => (
            "Get Pro…",
            "btn pro-btn",
            format!("The free version keeps {FREE_WATCHES} watch; Pro keeps any number"),
        ),
    };
    rsx! {
        button { class, title, onclick: move |_| pro.open.set(true), "{label}" }
    }
}

#[component]
pub fn ProWindow() -> Element {
    let mut pro = use_context::<Pro>();
    let mut key = use_signal(String::new);
    let mut error = use_signal(|| Option::<String>::None);
    if !(pro.open)() {
        return rsx! {};
    }
    let status = pro.status.read().clone();
    let mut close = move || {
        pro.open.set(false);
        error.set(None);
    };
    let mut activate = move || {
        let pasted = key.read().clone();
        match licence::activate(&pasted) {
            Ok(s) => {
                pro.status.set(s);
                key.set(String::new());
                error.set(None);
            }
            Err(e) => error.set(Some(e)),
        }
    };

    rsx! {
        div { class: "modal-bg",
            div { class: "modal",
                h2 { "JobScout Pro" }
                match &status {
                    Status::Free => rsx! {
                        p {
                            "The free version keeps one watch: one search that runs on its own and notifies you about good matches. "
                            "Searching, scoring and export are never limited. "
                            "Pro keeps any number of watches, in every country."
                        }
                    },
                    Status::Pro(l) => rsx! {
                        p { class: "pro-ok",
                            "Pro is active on this computer, licensed to "
                            strong { "{l.email}" }
                            ". Updates included until {l.updates_until}."
                        }
                    },
                    Status::Renew(l) => rsx! {
                        p {
                            "Your licence ("
                            strong { "{l.email}" }
                            ") covers versions released until {l.updates_until}. This version is newer, so it runs as the free version: renew Pro to use it here, or keep using a version from before that date."
                        }
                    },
                    Status::Unavailable => rsx! {
                        p { "This build of JobScout can't check licences, so nothing is limited. Builds from mayorana.ch can." }
                    },
                }
                if matches!(status, Status::Free | Status::Renew(_)) {
                    button {
                        class: "btn btn-primary pro-buy",
                        onclick: move |_| open_url(licence::BUY_URL),
                        if matches!(status, Status::Renew(_)) { "Renew JobScout Pro" } else { "Buy JobScout Pro" }
                    }
                    p { class: "muted small", "Already bought it? Paste the licence key from your email:" }
                    textarea {
                        class: "licence-key",
                        rows: 4,
                        placeholder: "Licence key",
                        value: "{key}",
                        oninput: move |e| key.set(e.value()),
                    }
                    if let Some(e) = error.read().as_ref() {
                        p { class: "pro-error small", "{e}" }
                    }
                    p { class: "muted small",
                        "The key is checked on this computer: no account, nothing sent anywhere."
                    }
                }
                div { class: "modal-actions",
                    if matches!(status, Status::Free | Status::Renew(_)) {
                        button {
                            class: "btn",
                            disabled: key.read().trim().is_empty(),
                            onclick: move |_| activate(),
                            "Activate"
                        }
                    }
                    if matches!(status, Status::Pro(_) | Status::Renew(_)) {
                        button {
                            class: "btn btn-ghost",
                            title: "To move the licence to another computer",
                            onclick: move |_| {
                                licence::deactivate();
                                pro.status.set(licence::current());
                            },
                            "Remove the licence from this computer"
                        }
                    }
                    span { class: "spacer" }
                    button { class: "btn btn-primary", onclick: move |_| close(), "Close" }
                }
            }
        }
    }
}
