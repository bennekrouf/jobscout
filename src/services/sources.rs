//! One entry point per operation, routed to the source that owns the job.

use std::time::Duration;

use chrono::Utc;

use super::fetch::{self, FetchError};
use super::{jobcloud, jobstreet, linkedin, mcf};
use crate::model::{Backend, Job, JobDetail, Posted, SearchParams, Settings, Source};

/// JobStreet answers plain HTTP with a Cloudflare challenge, so it is read
/// in a browser even when the other sources use direct HTTP: Camoufox if
/// that is the backend, the built-in WebKit one otherwise.
fn in_browser(settings: &Settings) -> Settings {
    let backend = match settings.backend {
        Backend::Camoufox => Backend::Camoufox,
        Backend::Http | Backend::WebKit => Backend::WebKit,
    };
    Settings { backend, ..settings.clone() }
}

/// One search page, reused from the page cache for `ttl`. Returns the body
/// and whether it was cached. `page` is 1-based; `offset` is how many
/// postings earlier pages returned (LinkedIn pages by offset, the others by
/// page number).
pub async fn fetch_search(
    source: Source,
    p: &SearchParams,
    page: u32,
    offset: u32,
    ttl: Duration,
    settings: &Settings,
) -> Result<(String, bool), FetchError> {
    match source {
        Source::LinkedIn => {
            fetch::get_cached(&linkedin::search_url(p, offset), ttl, settings).await
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
    }
}

pub fn parse_search(source: Source, body: &str, query: &str) -> Vec<Job> {
    match source {
        Source::LinkedIn => linkedin::parse_search(body, query),
        Source::JobsCh | Source::Jobup => jobcloud::parse_search(source, body, query),
        Source::Mcf => mcf::parse_search(body, query),
        Source::JobStreet => jobstreet::parse_search(body, query),
    }
}

pub async fn fetch_detail(job: &Job, settings: &Settings) -> Result<String, FetchError> {
    match job.source {
        Source::LinkedIn => fetch::get(&linkedin::detail_url(&job.id), settings).await,
        Source::JobsCh | Source::Jobup => fetch::get(&job.url, settings).await,
        Source::Mcf => fetch::get_json(&mcf::detail_url(&job.id), settings).await,
        Source::JobStreet => fetch::get(&job.url, &in_browser(settings)).await,
    }
}

pub fn parse_detail(source: Source, body: &str) -> Option<JobDetail> {
    match source {
        Source::LinkedIn => linkedin::parse_detail(body),
        Source::JobsCh | Source::Jobup => jobcloud::parse_detail(body),
        Source::Mcf => mcf::parse_detail(body),
        Source::JobStreet => jobstreet::parse_detail(body),
    }
}

/// LinkedIn filters by date itself; the other sources are not asked to, so
/// their postings are filtered here. Undated postings are kept.
pub fn within_posted(source: Source, jobs: Vec<Job>, posted: Posted) -> Vec<Job> {
    let Some(days) = posted.days().filter(|_| source != Source::LinkedIn) else {
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
