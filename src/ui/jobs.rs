use dioxus::prelude::*;

use super::score_class;
use crate::model::{Job, Status};
use crate::services::store;
use crate::state::{open_job, score_jobs, AppState};

/// How many unscored jobs one "Score" click takes on — each is a model call.
const SCORE_BATCH: usize = 20;

/// Rows per page of the list.
const PAGE_SIZE: usize = 50;

#[derive(Clone, Copy, PartialEq)]
enum Sort {
    Found,
    Score,
    Posted,
}

#[component]
pub fn JobList() -> Element {
    let mut st = use_context::<AppState>();
    // None = every status but Hidden.
    let mut status = use_signal(|| Option::<Status>::None);
    let mut sort = use_signal(|| Sort::Found);
    let mut text = use_signal(String::new);
    let mut page = use_signal(|| 0usize);

    let jobs = st.jobs.read();
    let results = st.results.read().clone();
    let needle = text.read().to_lowercase();
    let mut visible: Vec<&Job> = jobs
        .iter()
        .filter(|j| results.as_ref().is_none_or(|r| r.ids.contains(&j.id)))
        .filter(|j| match *status.read() {
            None => j.status != Status::Hidden,
            Some(s) => j.status == s,
        })
        .filter(|j| {
            needle.is_empty()
                || j.title.to_lowercase().contains(&needle)
                || j.company.to_lowercase().contains(&needle)
                || j.location.to_lowercase().contains(&needle)
        })
        .collect();
    match *sort.read() {
        Sort::Found => {}
        Sort::Score => visible.sort_by_key(|j| {
            std::cmp::Reverse(j.score.as_ref().map(|s| s.score as i16).unwrap_or(-1))
        }),
        Sort::Posted => visible.sort_by(|a, b| b.posted.cmp(&a.posted)),
    }

    let unscored: Vec<String> = visible
        .iter()
        .filter(|j| j.score.is_none())
        .take(SCORE_BATCH)
        .map(|j| j.id.clone())
        .collect();
    let n_unscored = unscored.len();
    let count = visible.len();
    let pages = count.div_ceil(PAGE_SIZE).max(1);
    // A filter or a status change can leave the page past the end.
    let current = (*page.read()).min(pages - 1);
    let first = current * PAGE_SIZE;
    let shown = &visible[first..(first + PAGE_SIZE).min(count)];
    let mut go = move |to: usize| {
        page.set(to);
        document::eval("document.getElementById('rows')?.scrollTo(0, 0)");
    };
    let busy = st.busy.read().is_some();
    let selected = st.selected.read().clone();
    let csv = store::to_csv(&visible);

    let tabs: Vec<(Option<Status>, &str)> = std::iter::once((None, "Active"))
        .chain(Status::ALL.into_iter().map(|s| (Some(s), s.label())))
        .collect();

    rsx! {
        section { class: "list",
            div { class: "list-head",
                div { class: "tabs",
                    for (value, name) in tabs {
                        button {
                            class: if *status.read() == value { "tab on" } else { "tab" },
                            onclick: move |_| { status.set(value); page.set(0); },
                            "{name}"
                        }
                    }
                }
                div { class: "list-tools",
                    input {
                        class: "input input-small",
                        placeholder: "Filter…",
                        value: "{text}",
                        oninput: move |e| { text.set(e.value()); page.set(0); },
                    }
                    select {
                        class: "input input-small",
                        onchange: move |e| {
                            sort.set(match e.value().as_str() {
                                "score" => Sort::Score,
                                "posted" => Sort::Posted,
                                _ => Sort::Found,
                            });
                            page.set(0);
                        },
                        option { value: "found", "Newest found" }
                        option { value: "score", "Best score" }
                        option { value: "posted", "Newest posted" }
                    }
                }
                if let Some(r) = &results {
                    div { class: "list-tools",
                        span { class: "muted", "Found by “{r.keywords}”" }
                        button {
                            class: "btn btn-small btn-ghost",
                            onclick: move |_| { st.results.set(None); page.set(0); },
                            "Show all jobs"
                        }
                    }
                }
                div { class: "list-tools",
                    span { class: "muted", "{count} jobs" }
                    button {
                        class: "btn btn-small",
                        disabled: busy || n_unscored == 0,
                        title: "Fetch descriptions as needed and score against your profile",
                        onclick: move |_| { spawn(score_jobs(st, unscored.clone())); },
                        "Score {n_unscored} unscored"
                    }
                    button {
                        class: "btn btn-small btn-ghost",
                        disabled: count == 0,
                        onclick: move |_| {
                            let csv = csv.clone();
                            spawn(async move {
                                if let Some(f) = rfd::AsyncFileDialog::new()
                                    .set_file_name("jobs.csv")
                                    .save_file()
                                    .await
                                {
                                    match std::fs::write(f.path(), csv) {
                                        Ok(()) => st.log(format!("Exported to {}", f.path().display())),
                                        Err(e) => st.log(format!("Export failed: {e}")),
                                    }
                                }
                            });
                        },
                        "Export CSV"
                    }
                }
            }

            div { class: "rows", id: "rows",
                if count == 0 {
                    div { class: "empty",
                        if jobs.is_empty() { "Run a search to collect job posts." } else { "Nothing here." }
                    }
                }
                for job in shown.iter().copied() {
                    {
                        let id = job.id.clone();
                        let is_sel = selected.as_deref() == Some(job.id.as_str());
                        rsx! {
                            div {
                                key: "{job.id}",
                                class: if is_sel { "row sel" } else { "row" },
                                onclick: move |_| {
                                    st.selected.set(Some(id.clone()));
                                    spawn(open_job(st, id.clone()));
                                },
                                match &job.score {
                                    Some(s) => rsx! { span { class: "badge {score_class(s.score)}", "{s.score}" } },
                                    None => rsx! { span { class: "badge score-none", "–" } },
                                }
                                div { class: "row-main",
                                    div { class: "row-title", "{job.title}" }
                                    div { class: "row-sub", "{job.company} · {job.location}" }
                                }
                                div { class: "row-meta",
                                    span { class: "src {job.source.class()}", "{job.source.label()}" }
                                    if job.status != Status::New {
                                        span { class: "pill pill-{job.status.class()}", "{job.status.label()}" }
                                    }
                                    span { class: "muted", {job.posted.clone().unwrap_or_default()} }
                                }
                            }
                        }
                    }
                }
            }

            if pages > 1 {
                div { class: "pager",
                    button {
                        class: "btn btn-small",
                        disabled: current == 0,
                        onclick: move |_| go(0),
                        "«"
                    }
                    button {
                        class: "btn btn-small",
                        disabled: current == 0,
                        onclick: move |_| go(current.saturating_sub(1)),
                        "‹ Prev"
                    }
                    span { class: "pager-info",
                        "Page {current + 1} of {pages}"
                        span { class: "muted", " · {first + 1}–{first + shown.len()} of {count}" }
                    }
                    button {
                        class: "btn btn-small",
                        disabled: current + 1 >= pages,
                        onclick: move |_| go(current + 1),
                        "Next ›"
                    }
                    button {
                        class: "btn btn-small",
                        disabled: current + 1 >= pages,
                        onclick: move |_| go(pages - 1),
                        "»"
                    }
                }
            }
        }
    }
}
