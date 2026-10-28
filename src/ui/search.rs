use dioxus::prelude::*;

use crate::model::{Contract, Country, Posted, Watch, EXPERIENCE};
use crate::services::licence::StatusExt;
use crate::services::store;
use crate::state::{run_search, AppState, Modal};
use crate::ui::flag::Flag;

/// Keyed by country in `ui::App`: switching country mounts a fresh form,
/// started from that country's last search.
#[component]
pub fn SearchPanel() -> Element {
    let mut st = use_context::<AppState>();
    let mut pro = use_context::<super::pro::Pro>();
    let country = *st.country.read();
    let mut p = use_signal(|| st.search.peek().clone());
    let busy = st.busy.read().is_some();

    let status = pro.status.read().clone();
    let limit = status.monthly_searches();
    let left = limit.saturating_sub(*st.searches_used.read());

    let mut submit = move || {
        // None left this month: the Pro window says why and what has more.
        if left == 0 {
            pro.open.set(true);
            return;
        }
        let params = p.read().clone();
        st.search.set(params.clone());
        store::save_search(country, &params);
        spawn(run_search(st, params));
    };

    let current = p.read().clone();
    // The boards here that can filter by contract; the others ignore it.
    let contract_sources = country
        .sources()
        .iter()
        .filter(|s| s.filters_contract())
        .map(|s| s.label())
        .collect::<Vec<_>>()
        .join(", ");

    rsx! {
        aside { class: "sidebar",

            div { class: "countries",
                for c in Country::ALL {
                    button {
                        key: "{c.code()}",
                        class: if c == country { "country on" } else { "country" },
                        r#type: "button",
                        disabled: busy && c != country,
                        title: if busy && c != country { "Wait for the current task to finish, or stop it".to_string() } else { format!("Load the {} context", c.label()) },
                        onclick: move |_| st.switch_country(c),
                        "aria-label": c.label(),
                        Flag { country: c }
                    }
                }
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
                    for &source in country.sources() {
                        Check {
                            key: "{source.label()}",
                            label: source.label(),
                            checked: current.sources.contains(&source),
                            on_change: move |v: bool| {
                                let mut w = p.write();
                                w.sources.retain(|s| *s != source);
                                if v {
                                    w.sources.push(source);
                                    w.sources.sort_by_key(|s| country.sources().iter().position(|x| x == s));
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

                if !contract_sources.is_empty() {
                    label { class: "field-label", "Contract · {contract_sources}" }
                    div { class: "checks",
                        for c in Contract::ALL {
                            Check {
                                key: "{c.label()}",
                                label: c.label(),
                                checked: current.contract.contains(&c),
                                on_change: move |v: bool| {
                                    let mut w = p.write();
                                    w.contract.retain(|x| *x != c);
                                    if v {
                                        w.contract.push(c);
                                        w.contract.sort_by_key(|x| Contract::ALL.iter().position(|y| y == x));
                                    }
                                },
                            }
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
                    title: "Each search counts as one of the {limit} a month",
                    "Search"
                }
                button {
                    class: if left == 0 { "searches-left out" } else { "searches-left" },
                    r#type: "button",
                    title: "Searches left this month, in every country. See the plans",
                    onclick: move |_| pro.open.set(true),
                    "{left} of {limit} searches left this month"
                }
                button {
                    class: "btn btn-wide btn-ghost watch-btn",
                    r#type: "button",
                    disabled: current.keywords.trim().is_empty(),
                    title: "Run this search on a schedule and notify about high scores. Each run that finds new jobs counts as a search",
                    onclick: move |_| {
                        let w = Watch::new(p.read().clone());
                        st.watches.write().push(w);
                        st.save_watches();
                        st.modal.set(Modal::Watches);
                    },
                    "Watch this search…"
                }
            }

            div { class: "sidebar-foot",
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
