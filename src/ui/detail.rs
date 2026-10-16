use dioxus::prelude::*;

use super::score_class;
use crate::model::Status;
use crate::state::{age, freshness, refresh_detail, score_jobs, AppState, Freshness};

#[component]
pub fn JobDetailPane() -> Element {
    let st = use_context::<AppState>();
    // Which job was last copied, for the button's "Copied" state.
    let mut copied = use_signal(|| Option::<String>::None);
    let Some(id) = st.selected.read().clone() else {
        return rsx! { section { class: "detail empty", "Select a job to see it here." } };
    };
    let Some(job) = st.job(&id) else {
        return rsx! { section { class: "detail empty", "Select a job to see it here." } };
    };
    let busy = st.busy.read().is_some();
    let queued = st.is_queued(&id);
    let fresh = freshness(&job, &st.settings.read(), chrono::Utc::now());
    let fetched = match (&job.detail, job.detail_failed_at) {
        (Some(d), _) => {
            let stale = if fresh == Freshness::Stale {
                " · stale"
            } else {
                ""
            };
            format!("Fetched {}{stale}", age(d.fetched_at))
        }
        (None, Some(at)) => format!("Page failed {}", age(at)),
        (None, None) => String::new(),
    };
    let (id_fetch, id_score, id_copy) = (id.clone(), id.clone(), id.clone());
    let is_copied = copied.read().as_deref() == Some(id.as_str());
    let text = job.as_text();
    let has_detail = job.detail.is_some();

    rsx! {
        section { class: "detail",
            div { class: "detail-head",
                h1 { "{job.title}" }
                div { class: "detail-sub",
                    strong { "{job.company}" }
                    " · {job.location}"
                    if let Some(p) = &job.posted { " · posted {p}" }
                }
                if let Some(s) = &job.salary { div { class: "detail-sub", "{s}" } }
                if let Some(d) = &job.detail {
                    if let Some(a) = &d.applicants { div { class: "muted", "{a}" } }
                }
            }

            div { class: "actions",
                a { class: "btn btn-primary", href: "{job.url}", target: "_blank", "Open on {job.source.label()} ↗" }
                button {
                    class: if is_copied { "btn btn-small on" } else { "btn btn-small" },
                    title: "Copy title, company, link, criteria and description",
                    onclick: move |_| {
                        match arboard::Clipboard::new().and_then(|mut c| c.set_text(text.clone())) {
                            Ok(()) => {
                                copied.set(Some(id_copy.clone()));
                                if !has_detail {
                                    st.log("Copied the header only — the description is not fetched yet.");
                                }
                            }
                            Err(e) => st.log(format!("Copy failed: {e}")),
                        }
                    },
                    if is_copied { "Copied ✓" } else { "Copy" }
                }
                for s in Status::ALL {
                    {
                        let id = id.clone();
                        rsx! {
                            button {
                                class: if job.status == s { "btn btn-small on" } else { "btn btn-small" },
                                onclick: move |_| st.update(&id, |j| j.status = s),
                                "{s.label()}"
                            }
                        }
                    }
                }
            }

            div { class: "actions",
                button {
                    class: "btn btn-small",
                    disabled: busy,
                    onclick: move |_| { spawn(refresh_detail(st, id_fetch.clone())); },
                    if job.detail.is_some() { "Refresh description" } else { "Fetch description" }
                }
                button {
                    class: "btn btn-small",
                    disabled: busy,
                    onclick: move |_| { spawn(score_jobs(st, vec![id_score.clone()])); },
                    if job.score.is_some() { "Re-score" } else { "Score against profile" }
                }
                span { class: "muted small fetched", "{fetched}" }
                if queued && job.detail.is_some() {
                    span { class: "muted small", "· refresh queued" }
                }
            }

            if let Some(s) = &job.score {
                div { class: "score-card",
                    div { class: "score-big {score_class(s.score)}", "{s.score}" }
                    div { class: "score-body",
                        p { class: "verdict", "{s.verdict}" }
                        div { class: "score-cols",
                            if !s.strengths.is_empty() {
                                ul { class: "good",
                                    for x in s.strengths.iter() { li { "{x}" } }
                                }
                            }
                            if !s.gaps.is_empty() {
                                ul { class: "bad",
                                    for x in s.gaps.iter() { li { "{x}" } }
                                }
                            }
                        }
                    }
                }
            }

            match &job.detail {
                Some(d) => rsx! {
                    if !d.criteria.is_empty() {
                        div { class: "criteria",
                            for (k, v) in d.criteria.iter() {
                                div { class: "crit", span { class: "muted", "{k}" } span { "{v}" } }
                            }
                        }
                    }
                    div { class: "description", "{d.description}" }
                },
                None => rsx! {
                    div { class: "muted pad",
                        if queued {
                            "Queued — the description loads when the current run finishes."
                        } else if busy {
                            "Loading description…"
                        } else {
                            "Description not fetched yet."
                        }
                    }
                },
            }
        }
    }
}
