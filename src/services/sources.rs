//! One entry point per operation, routed to the source that owns the job.

use chrono::Utc;

use super::{jobcloud, linkedin};
use crate::model::{Job, JobDetail, Posted, SearchParams, Source};

/// `page` is 1-based; `offset` is how many postings earlier pages returned
/// (LinkedIn pages by offset, JobCloud by page number).
pub fn search_url(source: Source, p: &SearchParams, page: u32, offset: u32) -> String {
    match source {
        Source::LinkedIn => linkedin::search_url(p, offset),
        Source::JobsCh | Source::Jobup => jobcloud::search_url(source, p, page),
    }
}

pub fn parse_search(source: Source, html: &str, query: &str) -> Vec<Job> {
    match source {
        Source::LinkedIn => linkedin::parse_search(html, query),
        Source::JobsCh | Source::Jobup => jobcloud::parse_search(source, html, query),
    }
}

pub fn detail_url(job: &Job) -> String {
    match job.source {
        Source::LinkedIn => linkedin::detail_url(&job.id),
        Source::JobsCh | Source::Jobup => job.url.clone(),
    }
}

pub fn parse_detail(source: Source, html: &str) -> Option<JobDetail> {
    match source {
        Source::LinkedIn => linkedin::parse_detail(html),
        Source::JobsCh | Source::Jobup => jobcloud::parse_detail(html),
    }
}

/// LinkedIn filters by date itself; JobCloud's pages do not take a date
/// filter, so their postings are filtered here. Undated postings are kept.
pub fn within_posted(source: Source, jobs: Vec<Job>, posted: Posted) -> Vec<Job> {
    let Some(days) = posted.days().filter(|_| source != Source::LinkedIn) else {
        return jobs;
    };
    let oldest = (Utc::now() - chrono::Duration::days(days)).format("%Y-%m-%d").to_string();
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
