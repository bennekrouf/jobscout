//! The JobScout Pro window: what the free version does, the licence on this
//! computer, and where to paste a key.

use dioxus::prelude::*;

use crate::model::Country;
use crate::services::licence::{self, FreeError, Status, FREE_WATCHES};
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
#[component]
pub fn ProButton() -> Element {
    let mut pro = use_context::<Pro>();
    let status = pro.status.read().clone();
    let (label, class, title) = match &status {
        Status::Pro(a) => (
            "✦ PRO",
            "btn pro-btn active",
            format!("JobScout Pro, {} plan for {}", a.plan.name, a.email),
        ),
        Status::NotPro(a) if a.is_free() => (
            "Get Pro…",
            "btn pro-btn",
            format!(
                "Free: {} AI credits left this month. The free version keeps {FREE_WATCHES} watch; Pro keeps any number",
                a.credits.left
            ),
        ),
        Status::NotPro(_) | Status::Offline(_) | Status::Unknown(_) => (
            "✦ RENEW PRO",
            "btn pro-btn renew",
            "Your licence doesn't make this JobScout Pro right now".to_owned(),
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
    let mut busy = use_signal(|| false);
    let mut free_email = use_signal(String::new);
    let mut free_note = use_signal(|| Option::<String>::None);
    let mut free_hidden = use_signal(|| false);
    if !(pro.open)() {
        return rsx! {};
    }
    let status = pro.status.read().clone();
    let licensed = status.licence().is_some();
    let on_free_plan = status.free_credits().is_some();
    let asks_number = !licensed || on_free_plan || matches!(status, Status::Unknown(_));
    let offers_free = matches!(status, Status::Free | Status::Unknown(_)) && !free_hidden();
    let mut close = move || {
        pro.open.set(false);
        error.set(None);
    };
    let mut activate = move || {
        let pasted = key.read().clone();
        busy.set(true);
        spawn(async move {
            match licence::activate(&pasted).await {
                Ok(s) => {
                    pro.status.set(s);
                    key.set(String::new());
                    error.set(None);
                }
                Err(e) => error.set(Some(e)),
            }
            busy.set(false);
        });
    };
    let mut check_again = move || {
        busy.set(true);
        spawn(async move {
            pro.status.set(licence::refresh(true).await);
            busy.set(false);
        });
    };
    let mut ask_free = move || {
        let email = free_email.read().clone();
        busy.set(true);
        spawn(async move {
            match licence::request_free_credits(&email).await {
                Ok(message) => {
                    free_note.set(Some(format!(
                        "{message} Paste the licence number above when it arrives."
                    )));
                    error.set(None);
                }
                Err(FreeError::Unavailable) => free_hidden.set(true),
                Err(FreeError::Refused(e)) => error.set(Some(e)),
            }
            busy.set(false);
        });
    };
    let mut manage = move |number: String| {
        busy.set(true);
        spawn(async move {
            match licence::portal_url(&number).await {
                Ok(url) => {
                    open_url(&url);
                    error.set(None);
                }
                Err(e) => error.set(Some(e)),
            }
            busy.set(false);
        });
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
                            "Pro keeps any number of watches, in every country. It comes with a monthly plan on mayorana.ch."
                        }
                    },
                    Status::Pro(a) => rsx! {
                        p { class: "pro-ok",
                            "Pro is active on this computer: "
                            strong { "{a.plan.name}" }
                            " plan for "
                            strong { "{a.email}" }
                            "."
                        }
                        p { class: "small",
                            "{a.credits.left} AI credits left this month. {a.renewal()}."
                        }
                    },
                    Status::NotPro(a) if a.is_free() => rsx! {
                        p {
                            "Free — "
                            strong { "{a.credits.left} AI credits" }
                            " left this month, for "
                            strong { "{a.email}" }
                            ". {a.renewal()}."
                        }
                        p {
                            "The free version keeps one watch. Pro keeps any number of watches, in every country. "
                            "It comes with a monthly plan on mayorana.ch."
                        }
                    },
                    Status::NotPro(a) if !a.active => rsx! {
                        p {
                            "The subscription for "
                            strong { "{a.email}" }
                            " isn't active, so JobScout runs as the free version. Renew it to have Pro again."
                        }
                    },
                    Status::NotPro(a) => rsx! {
                        p {
                            "The "
                            strong { "{a.plan.name}" }
                            " plan doesn't include JobScout, so it runs as the free version. A plan with JobScout makes it Pro."
                        }
                    },
                    Status::Offline(a) => rsx! {
                        p {
                            "JobScout hasn't been able to check the licence for "
                            strong { "{a.email}" }
                            " for a week, so it runs as the free version until it can. Check the connection, then check again."
                        }
                    },
                    Status::Unknown(_) => rsx! {
                        p { "The licence number on this computer isn't known. Check it against the email, or paste it again." }
                    },
                }
                if !matches!(status, Status::Pro(_)) {
                    button {
                        class: "btn btn-primary pro-buy",
                        onclick: move |_| open_url(licence::BUY_URL),
                        if licensed && !on_free_plan { "Renew JobScout Pro" } else { "Buy JobScout Pro" }
                    }
                }
                if asks_number {
                    p { class: "muted small", "Already subscribed? Paste the licence number from your email:" }
                    input {
                        class: "licence-key",
                        placeholder: "ABCDE-FGHJK-LMNPQ-RSTUV",
                        value: "{key}",
                        oninput: move |e| key.set(e.value()),
                    }
                    p { class: "muted small",
                        "The number is checked with api0 now and once a day. Pro keeps working for a week without a connection."
                    }
                }
                if offers_free {
                    div { class: "free-credits",
                        p { class: "muted small", "No plan? Get free AI credits every month, without Pro. The licence number comes by email." }
                        div { class: "free-row",
                            input {
                                class: "licence-key",
                                r#type: "email",
                                placeholder: "you@example.com",
                                value: "{free_email}",
                                oninput: move |e| free_email.set(e.value()),
                            }
                            button {
                                class: "btn",
                                disabled: free_email.read().trim().is_empty() || busy(),
                                onclick: move |_| ask_free(),
                                "Get free AI credits"
                            }
                        }
                        if let Some(n) = free_note.read().as_ref() {
                            p { class: "pro-ok small", "{n}" }
                        }
                    }
                }
                if let Some(e) = error.read().as_ref() {
                    p { class: "pro-error small", "{e}" }
                }
                div { class: "modal-actions",
                    if asks_number {
                        button {
                            class: "btn",
                            disabled: key.read().trim().is_empty() || busy(),
                            onclick: move |_| activate(),
                            "Activate"
                        }
                    }
                    if let Some(number) = status.licence().map(str::to_owned) {
                        if !matches!(status, Status::Unknown(_)) && !on_free_plan {
                            button {
                                class: "btn",
                                disabled: busy(),
                                title: "Change or cancel the plan on mayorana.ch",
                                onclick: move |_| manage(number.clone()),
                                "Manage subscription"
                            }
                        }
                        if !matches!(status, Status::Unknown(_)) {
                            button {
                                class: "btn btn-ghost",
                                disabled: busy(),
                                onclick: move |_| check_again(),
                                "Check again"
                            }
                        }
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
