//! One entry point per operation, routed to the source that owns the job.

use std::time::Duration;

use chrono::Utc;

use super::fetch::{self, FetchError};
use super::{
    arbeitsagentur, bayt, francetravail, jobcloud, jobstreet, linkedin, mcf, reed, stepstone,
};
use crate::model::{Backend, Country, Job, JobDetail, Posted, SearchParams, Settings, Source};

/// JobStreet and Bayt answer plain HTTP with a Cloudflare challenge,
/// IrishJobs and stepstone.at with a 403, and StepStone (stepstone.de and .be)
/// and Totaljobs reset the connection on their job pages, so those are read in a browser
/// even when the other sources use direct HTTP: Camoufox if that is the
/// backend, the built-in WebKit one otherwise.
fn in_browser(settings: &Settings) -> Settings {
    let backend = match settings.backend {
        Backend::Camoufox => Backend::Camoufox,
        Backend::Http | Backend::WebKit => Backend::WebKit,
    };
    Settings {
        backend,
        ..settings.clone()
    }
}

/// One search page, reused from the page cache for `ttl`. Returns the body
/// and whether it was cached. `page` is 1-based; `offset` is how many
/// postings earlier pages returned (LinkedIn pages by offset, the others by
/// page number).
pub async fn fetch_search(
    source: Source,
    country: Country,
    p: &SearchParams,
    page: u32,
    offset: u32,
    ttl: Duration,
    settings: &Settings,
) -> Result<(String, bool), FetchError> {
    match source {
        Source::LinkedIn => {
            fetch::get_cached(&linkedin::search_url(p, country, offset), ttl, settings).await
        }
        Source::JobsCh | Source::Jobup => {
            fetch::get_cached(&jobcloud::search_url(source, p, page), ttl, settings).await
        }
        Source::Mcf => {
            fetch::post_json_cached(&mcf::search_url(page), &mcf::search_body(p), ttl, settings)
                .await
        }
        Source::JobStreet => {
            fetch::get_cached(&jobstreet::search_url(p, page), ttl, &in_browser(settings)).await
        }
        Source::Arbeitsagentur => {
            let url = arbeitsagentur::search_url(p, page);
            fetch::get_json_cached(&url, &arbeitsagentur::HEADERS, ttl, settings).await
        }
        // Totaljobs has no page robots.txt allows past the first: an empty
        // page ends its results.
        Source::StepStone
        | Source::Totaljobs
        | Source::IrishJobs
        | Source::StepStoneBelgium
        | Source::StepStoneAustria => {
            // IrishJobs and stepstone.at refuse plain HTTP, search pages
            // included.
            let settings = if matches!(source, Source::IrishJobs | Source::StepStoneAustria) {
                &in_browser(settings)
            } else {
                settings
            };
            match stepstone::search_url(source, p, page) {
                Some(url) => fetch::get_cached(&url, ttl, settings).await,
                None => Ok((String::new(), false)),
            }
        }
        Source::Reed => reed::fetch_search(p, page, ttl, settings).await,
        Source::Bayt => {
            fetch::get_cached(&bayt::search_url(p, page), ttl, &in_browser(settings)).await
        }
        Source::FranceTravail => francetravail::fetch_search(p, page, ttl, settings).await,
    }
}

pub fn parse_search(source: Source, body: &str, query: &str) -> Vec<Job> {
    match source {
        Source::LinkedIn => linkedin::parse_search(body, query),
        Source::JobsCh | Source::Jobup => jobcloud::parse_search(source, body, query),
        Source::Mcf => mcf::parse_search(body, query),
        Source::JobStreet => jobstreet::parse_search(body, query),
        Source::Arbeitsagentur => arbeitsagentur::parse_search(body, query),
        Source::StepStone
        | Source::Totaljobs
        | Source::IrishJobs
        | Source::StepStoneBelgium
        | Source::StepStoneAustria => stepstone::parse_search(source, body, query),
        Source::Reed => reed::parse_search(body, query),
        Source::Bayt => bayt::parse_search(body, query),
        Source::FranceTravail => francetravail::parse_search(body, query),
    }
}

pub async fn fetch_detail(job: &Job, settings: &Settings) -> Result<String, FetchError> {
    match job.source {
        Source::LinkedIn => fetch::get(&linkedin::detail_url(&job.id), settings).await,
        Source::JobsCh | Source::Jobup => fetch::get(&job.url, settings).await,
        Source::Mcf => fetch::get_json(&mcf::detail_url(&job.id), &[], settings).await,
        Source::JobStreet => fetch::get(&job.url, &in_browser(settings)).await,
        Source::Arbeitsagentur => {
            let url = arbeitsagentur::detail_url(&job.id);
            fetch::get_json(&url, &arbeitsagentur::HEADERS, settings).await
        }
        Source::StepStone
        | Source::Totaljobs
        | Source::IrishJobs
        | Source::StepStoneBelgium
        | Source::StepStoneAustria => fetch::get(&job.url, &in_browser(settings)).await,
        Source::Reed => reed::fetch_detail(&job.id, settings).await,
        Source::Bayt => fetch::get(&job.url, &in_browser(settings)).await,
        Source::FranceTravail => francetravail::fetch_detail(&job.id, settings).await,
    }
}

pub fn parse_detail(source: Source, body: &str) -> Option<JobDetail> {
    match source {
        Source::LinkedIn => linkedin::parse_detail(body),
        Source::JobsCh | Source::Jobup => jobcloud::parse_detail(body),
        Source::Mcf => mcf::parse_detail(body),
        Source::JobStreet => jobstreet::parse_detail(body),
        Source::Arbeitsagentur => arbeitsagentur::parse_detail(body),
        Source::StepStone
        | Source::Totaljobs
        | Source::IrishJobs
        | Source::StepStoneBelgium
        | Source::StepStoneAustria => stepstone::parse_detail(body),
        Source::Reed => reed::parse_detail(body),
        Source::Bayt => bayt::parse_detail(body),
        Source::FranceTravail => francetravail::parse_detail(body),
    }
}

/// LinkedIn and France Travail filter by date themselves. The other sources'
/// postings are filtered here (the Arbeitsagentur also filters, by the same
/// date, so nothing it returns is dropped). Undated postings are kept.
pub fn within_posted(source: Source, jobs: Vec<Job>, posted: Posted) -> Vec<Job> {
    let filtered_there = matches!(source, Source::LinkedIn | Source::FranceTravail);
    let Some(days) = posted.days().filter(|_| !filtered_there) else {
        return jobs;
    };
    let oldest = (Utc::now() - chrono::Duration::days(days))
        .format("%Y-%m-%d")
        .to_string();
    jobs.into_iter()
        .filter(|j| j.posted.as_deref().is_none_or(|d| d >= oldest.as_str()))
        .collect()
}

/// Lowercase letters and digits of title and company: how the same posting
/// is recognised across sources, which do not share ids.
pub fn same_posting_key(job: &Job) -> String {
    format!("{}|{}", job.title, job.company)
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '|')
        .flat_map(char::to_lowercase)
        .collect()
}

/// Adds the jobs in `found` that `jobs` does not have yet, newest first, and
/// returns their ids. A job seen again keeps its status, description and
/// score. So does one already found on another source under the same title
/// and company: the copy found later is dropped.
///
/// The window and the MCP server both add jobs through this, so a posting
/// is recognised the same way whichever of them found it.
pub fn merge_new(jobs: &mut Vec<Job>, found: Vec<Job>) -> Vec<String> {
    let mut ids: std::collections::HashSet<String> = jobs.iter().map(|j| j.id.clone()).collect();
    let mut keys: std::collections::HashMap<String, Source> = jobs
        .iter()
        .map(|j| (same_posting_key(j), j.source))
        .collect();
    let mut fresh = vec![];
    for f in found {
        let key = same_posting_key(&f);
        let elsewhere = keys.get(&key).is_some_and(|s| *s != f.source);
        if elsewhere || !ids.insert(f.id.clone()) {
            continue;
        }
        keys.insert(key, f.source);
        fresh.push(f);
    }
    let ids = fresh.iter().map(|j| j.id.clone()).collect();
    jobs.splice(0..0, fresh);
    ids
}
