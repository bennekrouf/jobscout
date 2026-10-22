use dioxus::prelude::*;

use crate::model::{Backend, Watch};
use crate::services::{llm, notify, store};
use crate::state::{age, run_watch, AppState, Modal};
use crate::ui::flag::{Flag, RegionFlag};

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
    // What each api0 region costs, asked once when the window opens.
    let mut offers = use_signal(|| Option::<Result<Vec<llm::RegionOffer>, String>>::None);
    use_future(move || async move {
        offers.set(Some(llm::api0_regions().await));
    });
    let has_licence = llm::api0_key().is_some();
    let region_notes: Vec<(&'static llm::Region, String)> = llm::REGIONS
        .iter()
        .map(|r| {
            let note = match offers.read().as_ref() {
                _ if !has_licence => "needs a licence".to_string(),
                None => "\u{2026}".to_string(),
                Some(Err(_)) => "unavailable".to_string(),
                Some(Ok(list)) => match list.iter().find(|o| o.id == r.id) {
                    Some(o) => {
                        let n = o.credits_per_score();
                        format!(
                            "\u{2248} {n} {} a score",
                            if n == 1 { "credit" } else { "credits" }
                        )
                    }
                    None => "not offered".to_string(),
                },
            };
            (r, note)
        })
        .collect();
    let region = cur.region();
    let offers_error = match offers.read().as_ref() {
        Some(Err(e)) if has_licence => Some(e.clone()),
        _ => None,
    };

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

                h3 { "Scoring model" }
                div { class: "region-row",
                    for (r, note) in region_notes {
                        button {
                            key: "{r.id}",
                            class: if region == Some(r) { "region on" } else { "region" },
                            title: "api0, in {r.label}",
                            onclick: move |_| s.write().use_region(r.id),
                            RegionFlag { id: r.id }
                            span { class: "region-label", "{r.label}" }
                            span { class: "region-note", "{note}" }
                        }
                    }
                    button {
                        class: if cur.is_ollama() { "region on" } else { "region" },
                        title: "A model running on this computer",
                        onclick: move |_| s.write().use_ollama(),
                        span { class: "region-icon", "\u{1F4BB}" }
                        span { class: "region-label", "Ollama" }
                        span { class: "region-note", "on this computer, free" }
                    }
                }
                div { class: "presets",
                    button {
                        class: if cur.llm_base_url.contains("api.deepseek.com") { "btn btn-small on" } else { "btn btn-small" },
                        onclick: move |_| {
                            let mut w = s.write();
                            w.llm_base_url = "https://api.deepseek.com".into();
                            w.llm_model = "deepseek-chat".into();
                            w.llm_key_env = "DEEPSEEK_API_KEY".into();
                        },
                        "DeepSeek, your own key"
                    }
                }
                if let Some(r) = region {
                    if has_licence {
                        p { class: "muted small",
                            "api0 runs the model in {r.label} and takes AI credits from the licence \
                             in the Pro window, with no key to set up. Which model runs there is \
                             api0's choice. A score is one model call of about \
                             {llm::TYPICAL_SCORE_TOKENS} tokens; a long profile costs more."
                        }
                    } else {
                        p { class: "muted small",
                            "api0 needs a licence: get free AI credits or paste your licence number \
                             in the Pro window."
                        }
                    }
                    if let Some(e) = offers_error {
                        p { class: "muted small", "{e}" }
                    }
                } else {
                    label { class: "field-label", "Base URL (OpenAI-compatible)" }
                    input { class: "input", value: "{cur.llm_base_url}", oninput: move |e| s.write().llm_base_url = e.value() }
                    label { class: "field-label", "Model" }
                    input { class: "input", value: "{cur.llm_model}", oninput: move |e| s.write().llm_model = e.value() }
                    label { class: "field-label", "API key environment variable (empty for none)" }
                    input { class: "input", value: "{cur.llm_key_env}", oninput: move |e| s.write().llm_key_env = e.value() }
                    p { class: "muted small", "The key itself is read from your environment and never saved." }
                    if llm::is_api0(&cur.llm_base_url) {
                        p { class: "muted small",
                            "api0 needs no key: it uses the licence from the Pro window, and each \
                             score takes from its AI credits."
                        }
                    }
                }

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
                     notification for every new job at or above its threshold. \
                     Tick Email to get the same jobs by email, at your licence's email."
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
    let (id_every, id_min, id_on, id_run, id_del, id_mail) = (
        id.clone(),
        id.clone(),
        id.clone(),
        id.clone(),
        id.clone(),
        id.clone(),
    );
    // The licence is what api0 sends the email with, and to whose email.
    let pro = use_context::<super::pro::Pro>();
    let has_licence = pro.status.read().licence().is_some();
    let mail_title = if has_licence {
        "Also email what it finds to your licence's email"
    } else {
        "Emails go to your licence's email: paste a licence in the Pro window, or get free AI credits there"
    };
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
                label { class: "check", title: mail_title,
                    input {
                        r#type: "checkbox",
                        checked: watch.email && has_licence,
                        disabled: !has_licence,
                        onchange: move |e| st.update_watch(&id_mail, |w| w.email = e.checked()),
                    }
                    "Email"
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
