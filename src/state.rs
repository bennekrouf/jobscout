//! Window state shared through context, and the long-running actions on it.
//!
//! One action runs at a time (`busy`); each checks `stop` between requests,
//! so Stop takes effect after the request in flight, never mid-write.

use chrono::{DateTime, Duration, Local, Utc};
use dioxus::prelude::*;

use crate::model::{Country, Job, SearchParams, Settings, Source, Watch};

/// A page that could not be read is not asked for again before this.
const DETAIL_RETRY_AFTER: Duration = Duration::hours(1);
/// An explicit Refresh within this of the last fetch reuses it — a
/// double-click is not a reason to ask LinkedIn twice.
const REFRESH_COOLDOWN: Duration = Duration::minutes(2);
/// A watch scores at most this many new jobs per run — each is a model call
/// and a LinkedIn request. The rest stay listed, unscored.
const WATCH_SCORE_CAP: usize = 15;
/// How often the scheduler looks for a watch that is due.
const SCHEDULER_TICK: std::time::Duration = std::time::Duration::from_secs(60);

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Freshness {
    /// Never fetched; or failed, and the retry window has passed.
    Missing,
    /// Failed recently: wait before asking again.
    Failed,
    /// Fetched, but older than the TTL: show it, refresh it in the background.
    Stale,
    Fresh,
}

pub fn freshness(job: &Job, settings: &Settings, now: DateTime<Utc>) -> Freshness {
    match &job.detail {
        Some(d) if now - d.fetched_at < Duration::hours(settings.detail_ttl_hours as i64) => {
            Freshness::Fresh
        }
        Some(_) => Freshness::Stale,
        None if job
            .detail_failed_at
            .is_some_and(|at| now - at < DETAIL_RETRY_AFTER) =>
        {
            Freshness::Failed
        }
        None => Freshness::Missing,
    }
}

/// "just now", "5 min ago", "3 h ago", "2 d ago".
pub fn age(at: DateTime<Utc>) -> String {
    let d = Utc::now() - at;
    match d.num_minutes() {
        m if m < 1 => "just now".into(),
        m if m < 60 => format!("{m} min ago"),
        m if m < 60 * 48 => format!("{} h ago", m / 60),
        m => format!("{} d ago", m / (60 * 24)),
    }
}
use crate::services::sources::SearchEvent;
use crate::services::{licence, llm, mail, notify, sources, store};

#[derive(Clone, Copy, PartialEq)]
pub enum Modal {
    None,
    Profile,
    Settings,
    Watches,
    /// How to use JobScout from Claude.
    Mcp,
}

/// The jobs one search found, new or already listed.
#[derive(Clone, PartialEq)]
pub struct Results {
    pub keywords: String,
    pub ids: std::collections::HashSet<String>,
}

#[derive(Clone, Copy)]
pub struct AppState {
    /// The country whose context is loaded: `jobs`, `profile`, `watches` and
    /// `search` all belong to it. See `switch_country`.
    pub country: Signal<Country>,
    /// The last search run in this country; the sidebar starts from it.
    pub search: Signal<SearchParams>,
    pub jobs: Signal<Vec<Job>>,
    /// What the last search from the sidebar found, which the list shows
    /// instead of every job until it is cleared.
    pub results: Signal<Option<Results>>,
    pub selected: Signal<Option<String>>,
    pub settings: Signal<Settings>,
    pub profile: Signal<String>,
    /// What is running, if anything.
    pub busy: Signal<Option<String>>,
    pub stop: Signal<bool>,
    /// Jobs opened while something else was running, most recently opened
    /// first; their descriptions are fetched as soon as the app is idle.
    pub detail_queue: Signal<Vec<String>>,
    /// The loaded country's activity log. The others wait in `other_logs`
    /// until their country is loaded again, so each country shows only what
    /// happened in it.
    pub log: Signal<Vec<String>>,
    other_logs: Signal<std::collections::HashMap<Country, Vec<String>>>,
    pub modal: Signal<Modal>,
    pub watches: Signal<Vec<Watch>>,
    /// Searches run this month, in every country and from Claude too: the
    /// plan sets how many there may be. See `licence::take_search`.
    pub searches_used: Signal<u32>,
}

impl AppState {
    pub fn new() -> Self {
        let settings = store::load_settings();
        let country = settings.country;
        Self {
            country: Signal::new(country),
            search: Signal::new(store::load_search(country)),
            jobs: Signal::new(store::load_jobs(country)),
            results: Signal::new(None),
            selected: Signal::new(None),
            settings: Signal::new(settings),
            profile: Signal::new(store::load_profile(country)),
            busy: Signal::new(None),
            stop: Signal::new(false),
            detail_queue: Signal::new(vec![]),
            log: Signal::new(vec![]),
            other_logs: Signal::new(Default::default()),
            modal: Signal::new(Modal::None),
            watches: Signal::new(store::load_watches(country)),
            searches_used: Signal::new(licence::searches_used()),
        }
    }

    /// Unloads the current country and loads `to`: its jobs, watches,
    /// profile, last search and activity log. Refused while anything runs, since a search
    /// or watch in flight would write into the country just loaded.
    pub fn switch_country(mut self, to: Country) {
        let from = *self.country.peek();
        if from == to || self.busy.peek().is_some() {
            return;
        }
        let back = self.other_logs.write().remove(&to).unwrap_or_default();
        let left = std::mem::replace(&mut *self.log.write(), back);
        self.other_logs.write().insert(from, left);
        self.country.set(to);
        self.search.set(store::load_search(to));
        self.jobs.set(store::load_jobs(to));
        self.profile.set(store::load_profile(to));
        self.watches.set(store::load_watches(to));
        self.results.set(None);
        self.selected.set(None);
        self.detail_queue.set(vec![]);
        self.settings.write().country = to;
        store::save_settings(&self.settings.peek());
        self.log(format!(
            "{}: {} jobs, {} watches.",
            to.label(),
            self.jobs.peek().len(),
            self.watches.peek().len()
        ));
    }

    /// Loads what another process changed on disk in the loaded country:
    /// `jobscout mcp`, driven by Claude, writes the same files as this
    /// window. `ui::App` calls this every couple of seconds, so a job Claude
    /// scored or marked shows up here, and this window's next save keeps it.
    pub fn reload_changed(mut self) {
        let country = *self.country.peek();
        let mut what = vec![];
        if store::changed_elsewhere(country, store::JOBS) {
            self.jobs.set(store::load_jobs(country));
            what.push("jobs");
        }
        if store::changed_elsewhere(country, store::WATCHES) {
            self.watches.set(store::load_watches(country));
            what.push("watches");
        }
        if store::changed_elsewhere(country, store::PROFILE) {
            self.profile.set(store::load_profile(country));
            what.push("profile");
        }
        if store::settings_changed_elsewhere() {
            // The country stays this window's: switching it reloads everything.
            let mut loaded = store::load_settings();
            loaded.country = country;
            self.settings.set(loaded);
            what.push("settings");
        }
        let used = licence::searches_used();
        if *self.searches_used.peek() != used {
            self.searches_used.set(used);
        }
        if !what.is_empty() {
            self.log(format!(
                "Loaded {} changed outside this window.",
                what.join(", ")
            ));
        }
    }

    pub fn save_jobs(self) {
        store::save_jobs(*self.country.peek(), &self.jobs.peek());
    }

    pub fn save_watches(self) {
        store::save_watches(*self.country.peek(), &self.watches.peek());
    }

    /// Counts one search against the plan, or logs why there is none left.
    fn take_search(mut self) -> Result<(), String> {
        let taken = licence::take_search(&licence::current());
        self.searches_used.set(licence::searches_used());
        if let Err(why) = &taken {
            self.log(why);
        }
        taken
    }

    pub fn log(mut self, msg: impl AsRef<str>) {
        let mut log = self.log.write();
        log.push(format!(
            "{}  {}",
            Local::now().format("%H:%M:%S"),
            msg.as_ref()
        ));
        let excess = log.len().saturating_sub(300);
        log.drain(..excess);
    }

    pub fn job(self, id: &str) -> Option<Job> {
        self.jobs.read().iter().find(|j| j.id == id).cloned()
    }

    pub fn update(mut self, id: &str, f: impl FnOnce(&mut Job)) {
        if let Some(job) = self.jobs.write().iter_mut().find(|j| j.id == id) {
            f(job);
        }
        self.save_jobs();
    }

    pub fn update_watch(mut self, id: &str, f: impl FnOnce(&mut Watch)) {
        if let Some(w) = self.watches.write().iter_mut().find(|w| w.id == id) {
            f(w);
        }
        self.save_watches();
    }

    /// Adds jobs not seen before, newest first; returns their ids. See
    /// `sources::merge_new` for what counts as seen.
    fn merge(mut self, found: Vec<Job>) -> Vec<String> {
        let ids = sources::merge_new(&mut self.jobs.write(), found);
        self.save_jobs();
        ids
    }

    /// Adds what a page of the search found to `results`, new or already
    /// listed. A posting already listed from another source shows as that
    /// copy, which is the one `merge` kept.
    fn show_found(mut self, found: &[Job]) {
        let jobs = self.jobs.peek();
        let mut results = self.results.write();
        let Some(r) = results.as_mut() else {
            return;
        };
        for f in found {
            let key = sources::same_posting_key(f);
            let kept = jobs.iter().find(|j| j.id == f.id).or_else(|| {
                jobs.iter()
                    .find(|j| j.source != f.source && sources::same_posting_key(j) == key)
            });
            if let Some(j) = kept {
                r.ids.insert(j.id.clone());
            }
        }
    }

    fn begin(mut self, what: impl Into<String>) -> bool {
        if self.busy.peek().is_some() {
            return false;
        }
        self.stop.set(false);
        self.busy.set(Some(what.into()));
        true
    }

    fn enqueue_detail(mut self, id: String) {
        let mut q = self.detail_queue.write();
        q.retain(|x| *x != id);
        q.insert(0, id);
    }

    pub fn is_queued(self, id: &str) -> bool {
        self.detail_queue.read().iter().any(|x| x == id)
    }

    fn set_busy(mut self, what: String) {
        self.busy.set(Some(what));
    }

    fn end(mut self) {
        self.busy.set(None);
    }

    fn stopped(self) -> bool {
        *self.stop.peek()
    }
}

pub async fn run_search(mut st: AppState, p: SearchParams) {
    if p.keywords.trim().is_empty() {
        st.log("Enter keywords to search.");
        return;
    }
    if st.busy.peek().is_some() || st.take_search().is_err() {
        return;
    }
    if !st.begin(format!("Searching “{}”", p.keywords.trim())) {
        return;
    }
    st.results.set(Some(Results {
        keywords: p.keywords.trim().to_string(),
        ids: Default::default(),
    }));
    let settings = st.settings.peek().clone();
    search_pages(st, &p, &settings, true).await;
    st.end();
}

/// Runs one search on every selected source and returns the ids of the jobs
/// it added. `manual` is off for watches: a scheduled run is there to see
/// what is new, so it never reuses a page from an earlier search, and it
/// leaves what the list shows alone.
async fn search_pages(
    st: AppState,
    p: &SearchParams,
    settings: &Settings,
    manual: bool,
) -> Vec<String> {
    // The jobs go into the loaded country, so only its sources may run, and
    // it is where they search.
    let country = *st.country.peek();
    let query = format!("{} · {}", p.keywords.trim(), country.label());
    let mut added = vec![];
    let sources: Vec<Source> = p
        .sources
        .iter()
        .copied()
        .filter(|s| country.sources().contains(s))
        .collect();
    if sources.is_empty() {
        st.log("No source selected.");
    }
    for source in sources {
        if st.stopped() {
            break;
        }
        added.extend(search_source(st, source, country, p, settings, manual, &query).await);
    }
    st.log(format!("Search “{query}” done: {} new jobs.", added.len()));
    added
}

/// One source's pages, logged as they go. A source that fails or rate-limits
/// ends its own part of the search only; the other sources still run.
async fn search_source(
    st: AppState,
    source: Source,
    country: Country,
    p: &SearchParams,
    settings: &Settings,
    manual: bool,
    query: &str,
) -> Vec<String> {
    let name = source.label();
    let minutes = if manual {
        settings.search_cache_minutes
    } else {
        0
    };
    let ttl = std::time::Duration::from_secs(minutes * 60);
    let report = |event| {
        st.log(match event {
            SearchEvent::Page {
                page,
                found,
                new,
                cached,
            } => {
                let from = if cached { " (cached)" } else { "" };
                format!("{name} page {page}{from}: {found} jobs, {new} new.")
            }
            SearchEvent::End { page } => format!("{name} page {page}: no more results."),
            SearchEvent::EndStatus { page, code } => {
                format!("{name} page {page}: HTTP {code}, treated as the end of results.")
            }
            SearchEvent::RateLimited(code) => format!(
                "{name} is rate-limiting (HTTP {code}). Skipped the rest of {name} — wait a few minutes or raise the delay in Settings."
            ),
            SearchEvent::Failed(e) => format!("{name} search failed: {e}"),
            SearchEvent::Stopped => "Search stopped.".into(),
        })
    };
    sources::search_source(
        source,
        country,
        p,
        ttl,
        settings,
        query,
        || st.stopped(),
        |found| {
            let shown = manual.then(|| found.clone());
            let new = st.merge(found);
            if let Some(found) = shown {
                st.show_found(&found);
            }
            new
        },
        report,
    )
    .await
}

async fn fetch_detail_inner(st: AppState, id: &str) -> Result<(), String> {
    let settings = st.settings.peek().clone();
    let job = st.job(id).ok_or("job no longer listed")?;
    sources::fetch_job_detail(&job, &settings, |detail| {
        st.update(id, |j| sources::record_detail(j, detail));
        Ok(())
    })
    .await
}

/// Fetches the description only when the cache says so — what opening a job
/// does. A stale description stays on screen while the new one loads.
pub async fn open_job(st: AppState, id: String) {
    let Some(job) = st.job(&id) else { return };
    let settings = st.settings.peek().clone();
    let label = match freshness(&job, &settings, Utc::now()) {
        Freshness::Fresh | Freshness::Failed => return,
        Freshness::Stale => "Refreshing description",
        Freshness::Missing => "Fetching description",
    };
    if !st.begin(label) {
        st.enqueue_detail(id);
        return;
    }
    if let Err(e) = fetch_detail_inner(st, &id).await {
        st.log(format!("Description: {e}"));
    }
    st.end();
}

/// Opens the next queued job, if the app is idle. One job per call: whatever
/// it changes (busy, or the queue itself) makes the watcher in `ui::App`
/// call again, so the queue drains one fetch at a time and still yields to
/// anything the user starts in between.
pub async fn drain_detail_queue(mut st: AppState) {
    if st.busy.peek().is_some() {
        return;
    }
    let next = {
        let mut q = st.detail_queue.write();
        (!q.is_empty()).then(|| q.remove(0))
    };
    if let Some(id) = next {
        open_job(st, id).await;
    }
}

/// The Refresh button: always goes to LinkedIn, unless the description was
/// fetched moments ago.
pub async fn refresh_detail(st: AppState, id: String) {
    let recent = st
        .job(&id)
        .and_then(|j| j.detail.map(|d| d.fetched_at))
        .is_some_and(|at| Utc::now() - at < REFRESH_COOLDOWN);
    if recent {
        st.log("Description was fetched less than 2 minutes ago — kept as is.");
        return;
    }
    if !st.begin("Refreshing description") {
        return;
    }
    if let Err(e) = fetch_detail_inner(st, &id).await {
        st.log(format!("Description: {e}"));
    }
    st.end();
}

async fn score_inner(st: AppState, id: &str) -> Result<u8, String> {
    let profile = st.profile.peek().clone();
    if profile.trim().is_empty() {
        return Err("set your profile first (Profile button)".into());
    }
    let settings = st.settings.peek().clone();
    let job = st.job(id).ok_or("job no longer listed")?;
    match freshness(&job, &settings, Utc::now()) {
        Freshness::Fresh => {}
        // Score what is there rather than fail on a page that just failed.
        Freshness::Failed => return Err("no description: the job page failed recently".into()),
        Freshness::Missing | Freshness::Stale => fetch_detail_inner(st, id).await?,
    }
    let job = st.job(id).ok_or("job no longer listed")?;
    let score = llm::score(&settings, &profile, &job).await?;
    let n = score.score;
    st.update(id, |j| j.score = Some(score));
    Ok(n)
}

/// Scores each job in turn, fetching its description first when needed.
pub async fn score_jobs(st: AppState, ids: Vec<String>) {
    if ids.is_empty() || !st.begin(format!("Scoring {} job(s)", ids.len())) {
        return;
    }
    score_loop(st, &ids).await;
    st.end();
}

async fn score_loop(st: AppState, ids: &[String]) {
    let total = ids.len();
    for (i, id) in ids.iter().enumerate() {
        if st.stopped() {
            st.log("Scoring stopped.");
            break;
        }
        st.set_busy(format!("Scoring {}/{total}", i + 1));
        let title = st
            .job(id)
            .map(|j| format!("{} @ {}", j.title, j.company))
            .unwrap_or_default();
        match score_inner(st, id).await {
            Ok(n) => st.log(format!("{n:>3}  {title}")),
            Err(e) => {
                st.log(format!("Could not score {title}: {e}"));
                // A missing or blocked page is this job's problem; anything else
                // (profile, model, rate limit) would repeat for every job.
                if !e.starts_with("HTTP") && !e.contains("no description") {
                    break;
                }
            }
        }
    }
}

/// One scheduled run: search without the page cache, score what is new (up
/// to `WATCH_SCORE_CAP`), and notify about new jobs at or above the watch's
/// threshold. A job is notified about once, ever.
pub async fn run_watch(st: AppState, watch_id: String) {
    let Some(w) = st.watches.peek().iter().find(|w| w.id == watch_id).cloned() else {
        return;
    };
    if st.busy.peek().is_some() {
        return;
    }
    // A run counts as a search only when it finds something new, but needs
    // one left to start.
    if let Err(why) = licence::search_left(&licence::current()) {
        st.log(&why);
        // Not tried again before its next run is due.
        st.update_watch(&watch_id, |w| {
            w.last_run = Some(Utc::now());
            w.last_result = Some(why);
        });
        return;
    }
    if !st.begin(format!("Watch “{}”", w.name())) {
        return;
    }
    let settings = st.settings.peek().clone();
    let new_ids = search_pages(st, &w.search, &settings, false).await;
    if !new_ids.is_empty() {
        // Should another search have taken the last one meanwhile, this one
        // goes uncounted: the jobs are found either way.
        let _ = licence::take_search(&licence::current());
        let mut used = st.searches_used;
        used.set(licence::searches_used());
    }

    let has_profile = !st.profile.peek().trim().is_empty();
    let to_score: Vec<String> = new_ids.iter().take(WATCH_SCORE_CAP).cloned().collect();
    if has_profile && !to_score.is_empty() && !st.stopped() {
        score_loop(st, &to_score).await;
    }

    let mut matches: Vec<Job> = new_ids
        .iter()
        .filter_map(|id| st.job(id))
        .filter(|j| !j.notified && j.score.as_ref().is_some_and(|s| s.score >= w.min_score))
        .collect();
    matches.sort_by_key(|j| std::cmp::Reverse(j.score.as_ref().map(|s| s.score)));
    if !matches.is_empty() {
        let (title, body) = notify::matches_message(&w.name(), &matches);
        notify::send(title, body);
        for j in &matches {
            st.update(&j.id, |j| j.notified = true);
        }
    }

    let mut result = if has_profile {
        format!(
            "{} new, {} at {}+",
            new_ids.len(),
            matches.len(),
            w.min_score
        )
    } else {
        format!("{} new, not scored (no profile)", new_ids.len())
    };

    // By email too: the matches, or every new job when nothing is scored.
    if w.email {
        let jobs: Vec<Job> = if has_profile {
            matches.clone()
        } else {
            new_ids.iter().filter_map(|id| st.job(id)).collect()
        };
        if !jobs.is_empty() {
            let email = mail::watch_email(&w.name(), *st.country.peek(), &jobs, has_profile);
            match mail::send(&email).await {
                Ok(to) => result.push_str(&format!(", emailed to {to}")),
                Err(why) => result.push_str(&format!(", {why}")),
            }
        }
    }
    st.update_watch(&watch_id, |w| {
        w.last_run = Some(Utc::now());
        w.last_result = Some(result.clone());
    });
    st.log(format!("Watch “{}”: {result}", w.name()));
    st.end();
}

/// Runs whichever watch is due, one at a time, whenever the app is idle.
/// Due-ness is judged by wall-clock time each tick, so a tick that comes late
/// (a sleeping Mac, App Nap) catches up instead of drifting.
pub async fn scheduler(st: AppState) {
    loop {
        tokio::time::sleep(SCHEDULER_TICK).await;
        if st.busy.peek().is_some() {
            continue;
        }
        let now = Utc::now();
        let due = st
            .watches
            .peek()
            .iter()
            .find(|w| w.is_due(now))
            .map(|w| w.id.clone());
        if let Some(id) = due {
            run_watch(st, id).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{JobDetail, Status};

    fn job(detail_age: Option<Duration>, failed_age: Option<Duration>) -> Job {
        let now = Utc::now();
        Job {
            id: "1".into(),
            source: Source::LinkedIn,
            title: "t".into(),
            company: "c".into(),
            location: "l".into(),
            url: "u".into(),
            posted: None,
            salary: None,
            found_at: now,
            query: String::new(),
            status: Status::New,
            detail: detail_age.map(|a| JobDetail {
                description: "d".into(),
                criteria: vec![],
                applicants: None,
                posted_ago: None,
                fetched_at: now - a,
            }),
            detail_failed_at: failed_age.map(|a| now - a),
            score: None,
            notified: false,
        }
    }

    #[test]
    fn watch_is_due_after_its_interval() {
        let now = Utc::now();
        let mut w = Watch::new(SearchParams::default());
        assert!(w.is_due(now), "a watch that never ran is due");
        w.last_run = Some(now - Duration::hours(5));
        assert!(!w.is_due(now));
        w.last_run = Some(now - Duration::hours(6));
        assert!(w.is_due(now));
        w.enabled = false;
        assert!(!w.is_due(now));
    }

    #[test]
    fn freshness_follows_ttl_and_retry_window() {
        let s = Settings::default(); // 12 h TTL
        let now = Utc::now();
        assert_eq!(freshness(&job(None, None), &s, now), Freshness::Missing);
        assert_eq!(
            freshness(&job(Some(Duration::hours(1)), None), &s, now),
            Freshness::Fresh
        );
        assert_eq!(
            freshness(&job(Some(Duration::hours(13)), None), &s, now),
            Freshness::Stale
        );
        assert_eq!(
            freshness(&job(None, Some(Duration::minutes(10))), &s, now),
            Freshness::Failed
        );
        assert_eq!(
            freshness(&job(None, Some(Duration::hours(2))), &s, now),
            Freshness::Missing
        );
        // A description in hand wins over an older failure.
        assert_eq!(
            freshness(
                &job(Some(Duration::hours(1)), Some(Duration::minutes(5))),
                &s,
                now
            ),
            Freshness::Fresh
        );
    }
}
