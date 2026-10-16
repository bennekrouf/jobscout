use dioxus::prelude::*;

use crate::model::{Posted, Source, Watch, EXPERIENCE};
use crate::services::store;
use crate::state::{run_search, AppState, Modal};

#[component]
pub fn SearchPanel() -> Element {
    let mut st = use_context::<AppState>();
    let mut p = use_signal(|| st.settings.peek().search.clone());
    let busy = st.busy.read().is_some();
    let profile_len = st.profile.read().trim().chars().count();
    let n_watches = st.watches.read().len();

    let mut submit = move || {
        let params = p.read().clone();
        st.settings.write().search = params.clone();
        store::save_settings(&st.settings.read());
        spawn(run_search(st, params));
    };

    let current = p.read().clone();

    rsx! {
        aside { class: "sidebar",
            div { class: "brand",
                span { class: "brand-mark", "◎" }
                span { "JobScout" }
            }

            form {
                class: "search-form",
                onsubmit: move |e| { e.prevent_default(); submit(); },

                label { class: "field-label", "Keywords" }
                input {
                    class: "input",
                    placeholder: "rust developer",
                    value: "{current.keywords}",
                    oninput: move |e| p.write().keywords = e.value(),
                }

                label { class: "field-label", "Location" }
                input {
                    class: "input",
                    placeholder: "Switzerland, Zurich, Remote…",
                    value: "{current.location}",
                    oninput: move |e| p.write().location = e.value(),
                }

                label { class: "field-label", "Date posted" }
                select {
                    class: "input",
                    onchange: move |e| {
                        let v = e.value();
                        if let Some(posted) = Posted::ALL.into_iter().find(|x| x.label() == v) {
                            p.write().posted = posted;
                        }
                    },
                    for opt in Posted::ALL {
                        option { value: "{opt.label()}", selected: opt == current.posted, "{opt.label()}" }
                    }
                }

                label { class: "field-label", "Sources" }
                div { class: "checks",
                    for source in Source::ALL {
                        Check {
                            key: "{source.label()}",
                            label: source.label(),
                            checked: current.sources.contains(&source),
                            on_change: move |v: bool| {
                                let mut w = p.write();
                                w.sources.retain(|s| *s != source);
                                if v {
                                    w.sources.push(source);
                                    w.sources.sort_by_key(|s| Source::ALL.iter().position(|x| x == s));
                                }
                            },
                        }
                    }
                }

                label { class: "field-label", "Workplace · LinkedIn only" }
                div { class: "checks",
                    Check { label: "On-site", checked: current.onsite, on_change: move |v| p.write().onsite = v }
                    Check { label: "Hybrid", checked: current.hybrid, on_change: move |v| p.write().hybrid = v }
                    Check { label: "Remote", checked: current.remote, on_change: move |v| p.write().remote = v }
                }

                label { class: "field-label", "Experience · LinkedIn only" }
                div { class: "checks",
                    for (code, name) in EXPERIENCE {
                        Check {
                            key: "{code}",
                            label: name,
                            checked: current.experience.contains(&code),
                            on_change: move |v: bool| {
                                let mut w = p.write();
                                w.experience.retain(|c| *c != code);
                                if v { w.experience.push(code); w.experience.sort(); }
                            },
                        }
                    }
                }

                label { class: "field-label", "Pages per source" }
                input {
                    class: "input",
                    r#type: "number",
                    min: "1",
                    max: "20",
                    value: "{current.pages}",
                    oninput: move |e| {
                        if let Ok(n) = e.value().parse::<u32>() {
                            p.write().pages = n.clamp(1, 20);
                        }
                    },
                }

                button {
                    class: "btn btn-primary btn-wide",
                    r#type: "submit",
                    disabled: busy || current.sources.is_empty(),
                    "Search"
                }
                button {
                    class: "btn btn-wide btn-ghost watch-btn",
                    r#type: "button",
                    disabled: current.keywords.trim().is_empty(),
                    title: "Run this search on a schedule and notify about high scores",
                    onclick: move |_| {
                        let w = Watch::new(p.read().clone());
                        st.watches.write().push(w);
                        store::save_watches(&st.watches.read());
                        st.modal.set(Modal::Watches);
                    },
                    "Watch this search…"
                }
            }

            div { class: "sidebar-foot",
                button {
                    class: "btn btn-wide",
                    onclick: move |_| st.modal.set(Modal::Profile),
                    if profile_len == 0 { "Set your profile…" } else { "Profile ({profile_len} chars)" }
                }
                button {
                    class: "btn btn-wide",
                    onclick: move |_| st.modal.set(Modal::Watches),
                    if n_watches == 0 { "Watches" } else { "Watches ({n_watches})" }
                }
                button {
                    class: "btn btn-wide btn-ghost",
                    onclick: move |_| st.modal.set(Modal::Settings),
                    "Settings"
                }
            }
        }
    }
}

#[component]
fn Check(label: String, checked: bool, on_change: EventHandler<bool>) -> Element {
    rsx! {
        label { class: if checked { "check on" } else { "check" },
            input {
                r#type: "checkbox",
                checked,
                onchange: move |e| on_change.call(e.checked()),
            }
            "{label}"
        }
    }
}
