//! Window state shared through context, and the long-running actions on it.
//!
//! One action runs at a time (`busy`); each checks `stop` between requests,
//! so Stop takes effect after the request in flight, never mid-write.

use chrono::{DateTime, Duration, Local, Utc};
use dioxus::prelude::*;

use crate::model::{Job, SearchParams, Settings, Source, Watch};

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
use crate::services::fetch::{self, FetchError};
use crate::services::{llm, notify, sources, store};

#[derive(Clone, Copy, PartialEq)]
pub enum Modal {
    None,
    Profile,
    Settings,
    Watches,
}

#[derive(Clone, Copy)]
pub struct AppState {
    pub jobs: Signal<Vec<Job>>,
    pub selected: Signal<Option<String>>,
    pub settings: Signal<Settings>,
    pub profile: Signal<String>,
    /// What is running, if anything.
    pub busy: Signal<Option<String>>,
    pub stop: Signal<bool>,
    /// Jobs opened while something else was running, most recently opened
    /// first; their descriptions are fetched as soon as the app is idle.
    pub detail_queue: Signal<Vec<String>>,
    pub log: Signal<Vec<String>>,
    pub modal: Signal<Modal>,
    pub watches: Signal<Vec<Watch>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            jobs: Signal::new(store::load_jobs()),
            selected: Signal::new(None),
            settings: Signal::new(store::load_settings()),
            profile: Signal::new(store::load_profile()),
            busy: Signal::new(None),
            stop: Signal::new(false),
            detail_queue: Signal::new(vec![]),
            log: Signal::new(vec![]),
            modal: Signal::new(Modal::None),
            watches: Signal::new(store::load_watches()),
        }
    }

    pub fn log(mut self, msg: impl AsRef<str>) {
        let mut log = self.log.write();
        log.push(format!("{}  {}", Local::now().format("%H:%M:%S"), msg.as_ref()));
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
        store::save_jobs(&self.jobs.read());
    }

    pub fn update_watch(mut self, id: &str, f: impl FnOnce(&mut Watch)) {
        if let Some(w) = self.watches.write().iter_mut().find(|w| w.id == id) {
            f(w);
        }
        store::save_watches(&self.watches.read());
    }

    /// Adds jobs not seen before, newest first; returns their ids.
    /// A job seen again keeps its status, description and score. So does one
    /// already found on another source under the same title and company:
    /// the copy found later is dropped.
    fn merge(mut self, found: Vec<Job>) -> Vec<String> {
        let mut jobs = self.jobs.write();
        let mut ids: std::collections::HashSet<String> = jobs.iter().map(|j| j.id.clone()).collect();
        let mut keys: std::collections::HashMap<String, Source> = jobs
            .iter()
            .map(|j| (sources::same_posting_key(j), j.source))
            .collect();
        let mut fresh = vec![];
        for f in found {
            let key = sources::same_posting_key(&f);
            let elsewhere = keys.get(&key).is_some_and(|s| *s != f.source);
            if elsewhere || !ids.insert(f.id.clone()) {
                continue;
            }
            keys.insert(key, f.source);
            fresh.push(f);
        }
        let ids = fresh.iter().map(|j| j.id.clone()).collect();
        jobs.splice(0..0, fresh);
        drop(jobs);
        store::save_jobs(&self.jobs.read());
        ids
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

pub async fn run_search(st: AppState, p: SearchParams) {
    if p.keywords.trim().is_empty() {
        st.log("Enter keywords to search.");
        return;
    }
    if !st.begin(format!("Searching “{}”", p.keywords.trim())) {
        return;
    }
    let settings = st.settings.peek().clone();
    search_pages(st, &p, &settings, true).await;
    st.end();
}

/// Runs one search on every selected source and returns the ids of the jobs
/// it added. `use_cache` is off for watches: a scheduled run is there to see
/// what is new, so it never reuses a page from an earlier search.
async fn search_pages(
    st: AppState,
    p: &SearchParams,
    settings: &Settings,
    use_cache: bool,
) -> Vec<String> {
    let query = format!("{} · {}", p.keywords.trim(), p.location.trim());
    let mut added = vec![];
    if p.sources.is_empty() {
        st.log("No source selected.");
    }
    for &source in &p.sources {
        if st.stopped() {
            break;
        }
        added.extend(search_source(st, source, p, settings, use_cache, &query).await);
    }
    st.log(format!("Search “{query}” done: {} new jobs.", added.len()));
    added
}

/// One source's pages. A source that fails or rate-limits ends its own part
/// of the search only; the other sources still run.
async fn search_source(
    st: AppState,
    source: Source,
    p: &SearchParams,
    settings: &Settings,
    use_cache: bool,
    query: &str,
) -> Vec<String> {
    let name = source.label();
    let minutes = if use_cache { settings.search_cache_minutes } else { 0 };
    let ttl = std::time::Duration::from_secs(minutes * 60);
    let mut added = vec![];
    let mut seen = std::collections::HashSet::new();
    let mut offset = 0;

    for page in 1..=p.pages.max(1) {
        if st.stopped() {
            st.log("Search stopped.");
            break;
        }
        let url = sources::search_url(source, p, page, offset);
        match fetch::get_cached(&url, ttl, settings).await {
            Ok((html, cached)) => {
                let found = sources::parse_search(source, &html, query);
                offset += found.len() as u32;
                let before = seen.len();
                seen.extend(found.iter().map(|j| j.id.clone()));
                // An empty page ends the results; so does one that only
                // repeats postings this search has already seen.
                if found.is_empty() || seen.len() == before {
                    st.log(format!("{name} page {page}: no more results."));
                    break;
                }
                let n = found.len();
                let new = st.merge(sources::within_posted(source, found, p.posted));
                let from = if cached { " (cached)" } else { "" };
                st.log(format!("{name} page {page}{from}: {n} jobs, {} new.", new.len()));
                added.extend(new);
            }
            Err(FetchError::RateLimited(code)) => {
                st.log(format!(
                    "{name} is rate-limiting (HTTP {code}). Skipped the rest of {name} — wait a few minutes or raise the delay in Settings."
                ));
                break;
            }
            // Past the first page, LinkedIn answers the end of results with an error status.
            Err(FetchError::Status(code)) if page > 1 => {
                st.log(format!("{name} page {page}: HTTP {code}, treated as the end of results."));
                break;
            }
            Err(e) => {
                st.log(format!("{name} search failed: {e}"));
                break;
            }
        }
    }
    added
}

async fn fetch_detail_inner(st: AppState, id: &str) -> Result<(), String> {
    let settings = st.settings.peek().clone();
    let job = st.job(id).ok_or("job no longer listed")?;
    let html = match fetch::get(&sources::detail_url(&job), &settings).await {
        Ok(html) => html,
        // The page itself answered with an error: remember it, so opening the
        // job again does not ask again. Network trouble and rate limits are
        // not this job's fault and are not remembered.
        Err(e @ FetchError::Status(_)) => {
            st.update(id, |j| j.detail_failed_at = Some(Utc::now()));
            return Err(e.to_string());
        }
        Err(e) => return Err(e.to_string()),
    };
    match sources::parse_detail(job.source, &html) {
        Some(detail) => {
            st.update(id, |j| {
                j.detail = Some(detail);
                j.detail_failed_at = None;
            });
            Ok(())
        }
        None => {
            st.update(id, |j| j.detail_failed_at = Some(Utc::now()));
            Err("no description on the page (removed, or a block page)".into())
        }
    }
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
        let title = st.job(id).map(|j| format!("{} @ {}", j.title, j.company)).unwrap_or_default();
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
    if !st.begin(format!("Watch “{}”", w.name())) {
        return;
    }
    let settings = st.settings.peek().clone();
    let new_ids = search_pages(st, &w.search, &settings, false).await;

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

    let result = if has_profile {
        format!("{} new, {} at {}+", new_ids.len(), matches.len(), w.min_score)
    } else {
        format!("{} new, not scored (no profile)", new_ids.len())
    };
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
        let due = st.watches.peek().iter().find(|w| w.is_due(now)).map(|w| w.id.clone());
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
        assert_eq!(freshness(&job(Some(Duration::hours(1)), None), &s, now), Freshness::Fresh);
        assert_eq!(freshness(&job(Some(Duration::hours(13)), None), &s, now), Freshness::Stale);
        assert_eq!(freshness(&job(None, Some(Duration::minutes(10))), &s, now), Freshness::Failed);
        assert_eq!(freshness(&job(None, Some(Duration::hours(2))), &s, now), Freshness::Missing);
        // A description in hand wins over an older failure.
        assert_eq!(
            freshness(&job(Some(Duration::hours(1)), Some(Duration::minutes(5))), &s, now),
            Freshness::Fresh
        );
    }
}
