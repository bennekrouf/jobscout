use dioxus::prelude::*;

use crate::model::{Backend, Settings, Watch};
use crate::services::{notify, store};
use crate::state::{age, run_watch, AppState, Modal};
use crate::ui::flag::Flag;

#[component]
pub fn ProfileModal() -> Element {
    let mut st = use_context::<AppState>();
    let mut text = use_signal(|| st.profile.peek().clone());

    rsx! {
        div { class: "modal-bg",
            div { class: "modal wide",
                h2 { "Your profile · " Flag { country: *st.country.read() } " {st.country.read().label()}" }
                p { class: "muted",
                    "Paste your CV and what you are looking for: target roles, seniority, \
                     locations, remote or not, salary floor, languages, deal-breakers. \
                     Every score compares a job with this text. Each country keeps its \
                     own profile: say here what this market means for you (permits, visa \
                     sponsorship, relocation)."
                }
                textarea {
                    class: "input profile-text",
                    value: "{text}",
                    oninput: move |e| text.set(e.value()),
                }
                div { class: "modal-actions",
                    button {
                        class: "btn btn-ghost",
                        onclick: move |_| {
                            spawn(async move {
                                if let Some(f) = rfd::AsyncFileDialog::new()
                                    .add_filter("Text", &["md", "txt", "yaml", "yml", "json"])
                                    .pick_file()
                                    .await
                                {
                                    match std::fs::read_to_string(f.path()) {
                                        Ok(s) => text.set(s),
                                        Err(e) => st.log(format!("Could not read {}: {e}", f.path().display())),
                                    }
                                }
                            });
                        },
                        "Load from file…"
                    }
                    span { class: "spacer" }
                    button { class: "btn", onclick: move |_| st.modal.set(Modal::None), "Cancel" }
                    button {
                        class: "btn btn-primary",
                        onclick: move |_| {
                            let t = text.read().clone();
                            store::save_profile(*st.country.peek(), &t);
                            st.profile.set(t);
                            st.modal.set(Modal::None);
                        },
                        "Save"
                    }
                }
            }
        }
    }
}

#[component]
pub fn SettingsModal() -> Element {
    let mut st = use_context::<AppState>();
    let mut s = use_signal(|| st.settings.peek().clone());
    let cur = s.read().clone();

    rsx! {
        div { class: "modal-bg",
            div { class: "modal",
                h2 { "Settings" }

                h3 { "Fetching" }
                label { class: "field-label", "Backend" }
                select {
                    class: "input",
                    onchange: move |e| {
                        s.write().backend = match e.value().as_str() {
                            "camoufox" => Backend::Camoufox,
                            "webkit" => Backend::WebKit,
                            _ => Backend::Http,
                        }
                    },
                    option { value: "http", selected: cur.backend == Backend::Http, "Direct HTTP (guest endpoints)" }
                    option { value: "webkit", selected: cur.backend == Backend::WebKit, "Built-in browser (WebKit)" }
                    option { value: "camoufox", selected: cur.backend == Backend::Camoufox, "Camoufox browser (needs Python)" }
                }
                label { class: "field-label", "Delay between requests (ms)" }
                input {
                    class: "input",
                    r#type: "number",
                    min: "500",
                    value: "{cur.delay_ms}",
                    oninput: move |e| if let Ok(v) = e.value().parse::<u64>() { s.write().delay_ms = v.max(500) },
                }
                label { class: "field-label", "Descriptions stay fresh for (hours)" }
                input {
                    class: "input",
                    r#type: "number",
                    min: "1",
                    value: "{cur.detail_ttl_hours}",
                    oninput: move |e| if let Ok(v) = e.value().parse::<u64>() { s.write().detail_ttl_hours = v.max(1) },
                }
                label { class: "field-label", "Reuse search pages for (minutes, 0 = never)" }
                input {
                    class: "input",
                    r#type: "number",
                    min: "0",
                    value: "{cur.search_cache_minutes}",
                    oninput: move |e| if let Ok(v) = e.value().parse::<u64>() { s.write().search_cache_minutes = v },
                }
                if cur.backend == Backend::Camoufox {
                    label { class: "field-label", "Python with camoufox installed" }
                    input {
                        class: "input",
                        value: "{cur.python}",
                        oninput: move |e| s.write().python = e.value(),
                    }
                    label { class: "check",
                        input {
                            r#type: "checkbox",
                            checked: cur.headed,
                            onchange: move |e| s.write().headed = e.checked(),
                        }
                        "Show the browser window"
                    }
                    p { class: "muted small",
                        "Setup: pip install -U camoufox && python -m camoufox fetch"
                    }
                }

                h3 { "Scoring model (OpenAI-compatible)" }
                div { class: "presets",
                    button {
                        class: "btn btn-small",
                        onclick: move |_| {
                            let mut w = s.write();
                            w.llm_base_url = Settings::default().llm_base_url;
                            w.llm_model = Settings::default().llm_model;
                            w.llm_key_env = Settings::default().llm_key_env;
                        },
                        "DeepSeek"
                    }
                    button {
                        class: "btn btn-small",
                        onclick: move |_| {
                            let mut w = s.write();
                            w.llm_base_url = "http://localhost:11434/v1".into();
                            w.llm_key_env = String::new();
                        },
                        "Local ollama"
                    }
                }
                label { class: "field-label", "Base URL" }
                input { class: "input", value: "{cur.llm_base_url}", oninput: move |e| s.write().llm_base_url = e.value() }
                label { class: "field-label", "Model" }
                input { class: "input", value: "{cur.llm_model}", oninput: move |e| s.write().llm_model = e.value() }
                label { class: "field-label", "API key environment variable (empty for none)" }
                input { class: "input", value: "{cur.llm_key_env}", oninput: move |e| s.write().llm_key_env = e.value() }
                p { class: "muted small", "The key itself is read from your environment and never saved." }

                h3 { "France Travail API" }
                p { class: "muted small",
                    "France's search needs an application registered at francetravail.io, \
                     subscribed to the \"Offres d'emploi v2\" API. Free; without it, \
                     France searches LinkedIn only."
                }
                label { class: "field-label", "Client id" }
                input { class: "input", value: "{cur.france_travail_client_id}", oninput: move |e| s.write().france_travail_client_id = e.value() }
                label { class: "field-label", "Client secret environment variable" }
                input { class: "input", value: "{cur.france_travail_secret_env}", oninput: move |e| s.write().france_travail_secret_env = e.value() }

                h3 { "Reed API" }
                p { class: "muted small",
                    "The United Kingdom's Reed search needs a free API key from \
                     reed.co.uk/developers. Without it, Reed is skipped."
                }
                label { class: "field-label", "API key environment variable" }
                input { class: "input", value: "{cur.reed_key_env}", oninput: move |e| s.write().reed_key_env = e.value() }

                div { class: "modal-actions",
                    span { class: "muted small", "Data: {store::data_dir().display()}" }
                    span { class: "spacer" }
                    button { class: "btn", onclick: move |_| st.modal.set(Modal::None), "Cancel" }
                    button {
                        class: "btn btn-primary",
                        onclick: move |_| {
                            let mut next = s.read().clone();
                            // The flags own the country, the splitters the
                            // column widths; keep their latest values.
                            next.country = st.settings.peek().country;
                            next.sidebar_width = st.settings.peek().sidebar_width;
                            next.list_width = st.settings.peek().list_width;
                            store::save_settings(&next);
                            st.settings.set(next);
                            st.modal.set(Modal::None);
                        },
                        "Save"
                    }
                }
            }
        }
    }
}

#[component]
pub fn WatchesModal() -> Element {
    let mut st = use_context::<AppState>();
    let watches = st.watches.read().clone();
    let busy = st.busy.read().is_some();
    let no_profile = st.profile.read().trim().is_empty();

    rsx! {
        div { class: "modal-bg",
            div { class: "modal wide",
                h2 { "Watches" }
                p { class: "muted",
                    "Each watch re-runs its search on a schedule while JobScout is open, \
                     scores what is new against your profile, and sends a desktop \
                     notification for every new job at or above its threshold."
                }
                if no_profile {
                    p { class: "warn", "No profile set: watches will list new jobs but cannot score or notify." }
                }
                if watches.is_empty() {
                    div { class: "empty", "No watches yet. Fill in a search, then “Watch this search…”." }
                }
                for w in watches {
                    WatchRow { key: "{w.id}", watch: w, busy }
                }
                div { class: "modal-actions",
                    button {
                        class: "btn btn-ghost",
                        onclick: move |_| notify::send(
                            "JobScout".into(),
                            "Notifications work. Matches will look like this.".into(),
                        ),
                        "Send test notification"
                    }
                    span { class: "spacer" }
                    button { class: "btn btn-primary", onclick: move |_| st.modal.set(Modal::None), "Done" }
                }
            }
        }
    }
}

#[component]
fn WatchRow(watch: Watch, busy: bool) -> Element {
    let mut st = use_context::<AppState>();
    let id = watch.id.clone();
    let (id_every, id_min, id_on, id_run, id_del) =
        (id.clone(), id.clone(), id.clone(), id.clone(), id.clone());
    let last = match (&watch.last_run, &watch.last_result) {
        (Some(at), Some(r)) => format!("Last run {}: {r}", age(*at)),
        (Some(at), None) => format!("Last run {}", age(*at)),
        _ => "Not run yet — runs within a minute".into(),
    };
    let p = &watch.search;
    let mut filters: Vec<String> = p.sources.iter().map(|s| s.label().to_string()).collect();
    filters.push(p.posted.label().to_string());
    filters.push(format!("{} pages each", p.pages));
    for (on, name) in [
        (p.onsite, "on-site"),
        (p.hybrid, "hybrid"),
        (p.remote, "remote"),
    ] {
        if on {
            filters.push(name.into());
        }
    }
    filters.extend(p.contract.iter().map(|c| c.label().to_string()));
    let filters = filters.join(" · ");

    rsx! {
        div { class: if watch.enabled { "watch" } else { "watch off" },
            div { class: "watch-main",
                div { class: "watch-name", "{watch.name()}" }
                div { class: "muted small", "{filters}" }
                div { class: "muted small", "{last}" }
            }
            div { class: "watch-controls",
                label { class: "small muted", "every" }
                select {
                    class: "input input-small",
                    onchange: move |e| {
                        if let Ok(h) = e.value().parse::<u32>() {
                            st.update_watch(&id_every, |w| w.every_hours = h);
                        }
                    },
                    for h in Watch::INTERVALS {
                        option { value: "{h}", selected: h == watch.every_hours, "{h} h" }
                    }
                }
                label { class: "small muted", "notify ≥" }
                input {
                    class: "input input-small score-input",
                    r#type: "number",
                    min: "0",
                    max: "100",
                    value: "{watch.min_score}",
                    oninput: move |e| {
                        if let Ok(v) = e.value().parse::<u8>() {
                            st.update_watch(&id_min, |w| w.min_score = v.min(100));
                        }
                    },
                }
                label { class: "check",
                    input {
                        r#type: "checkbox",
                        checked: watch.enabled,
                        onchange: move |e| st.update_watch(&id_on, |w| w.enabled = e.checked()),
                    }
                    "On"
                }
                button {
                    class: "btn btn-small",
                    disabled: busy,
                    onclick: move |_| { spawn(run_watch(st, id_run.clone())); },
                    "Run now"
                }
                button {
                    class: "btn btn-small btn-danger",
                    onclick: move |_| {
                        st.watches.write().retain(|w| w.id != id_del);
                        st.save_watches();
                    },
                    "Delete"
                }
            }
        }
    }
}
