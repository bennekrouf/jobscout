//! Bayt (bayt.com), the largest job board in the Middle East, for the UAE.
//!
//! The site is behind a Cloudflare challenge that plain HTTP never gets
//! past, so every request goes through a browser, like JobStreet's (see
//! `sources`).
//!
//!   * search: `/en/uae/jobs/{keywords as a slug}-jobs/?page=N`, all of the
//!     UAE, 30 postings a page. robots.txt disallows the country-less
//!     `/en/jobs/…-jobs/` pages, not the `/en/uae/` ones. The postings are
//!     read from their cards (`li[data-job-id]`): the page's JSON-LD only
//!     lists their links.
//!   * detail: the posting's own URL, one schema.org `JobPosting` in JSON-LD,
//!     the same shape the JobCloud boards publish.

use chrono::{DateTime, Utc};
use scraper::{ElementRef, Html, Selector};

use super::jobcloud;
use super::text::squash;
use crate::model::{Job, JobDetail, SearchParams, Source, Status};

const SITE: &str = "https://www.bayt.com";

/// `page` is 1-based, like the site's.
pub fn search_url(p: &SearchParams, page: u32) -> String {
    let mut url = reqwest::Url::parse(&format!("{SITE}/en/uae/jobs/{}-jobs/", slug(&p.keywords)))
        .expect("site URL parses");
    if page > 1 {
        url.query_pairs_mut().append_pair("page", &page.to_string());
    }
    url.to_string()
}

/// `Rust Developer` → `rust-developer`, the path the site itself links to.
fn slug(keywords: &str) -> String {
    let words: Vec<String> = keywords
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect();
    words.join("-")
}

/// Every card on one search page, unfiltered: the caller tells the end of the
/// results from a page that repeats what it has already seen.
pub fn parse_search(html: &str, query: &str) -> Vec<Job> {
    let now = Utc::now();
    let doc = Html::parse_document(html);
    let card = Selector::parse("li[data-js-job][data-job-id]").expect("constant selector");
    doc.select(&card)
        .filter_map(|c| to_job(c, query, now))
        .collect()
}

/// `None` when the page has no `JobPosting` with a description: a removed
/// posting, or a block page.
pub fn parse_detail(html: &str) -> Option<JobDetail> {
    jobcloud::parse_detail(html)
}

fn to_job(card: ElementRef, query: &str, now: DateTime<Utc>) -> Option<Job> {
    let sel = |s: &str| Selector::parse(s).expect("constant selector");
    let first = |s: &str| card.select(&sel(s)).next();
    let text = |e: ElementRef| squash(&e.text().collect::<String>());

    let id = card.value().attr("data-job-id")?.trim().to_string();
    let link = first("h2 a[href]")?;
    let href = link.value().attr("href")?;
    let url = reqwest::Url::parse(SITE).ok()?.join(href).ok()?;
    Some(Job {
        id,
        source: Source::Bayt,
        title: Some(text(link)).filter(|t| !t.is_empty())?,
        // A confidential employer has no company link.
        company: first("div.job-company-location-wrapper a")
            .map(text)
            .unwrap_or_default(),
        location: first("dt.jb-label-location")
            .map(text)
            .filter(|l| !l.is_empty())
            .unwrap_or_else(|| "United Arab Emirates".into()),
        url: url.to_string(),
        // When the posting went live, as a Unix time.
        posted: first("[data-automation-jobactivedate]")
            .and_then(|e| e.value().attr("data-automation-jobactivedate"))
            .and_then(|t| t.trim().parse::<i64>().ok())
            .and_then(|t| DateTime::from_timestamp(t, 0))
            .map(|t| t.format("%Y-%m-%d").to_string()),
        salary: None,
        found_at: now,
        query: query.to_string(),
        status: Status::New,
        detail: None,
        detail_failed_at: None,
        score: None,
        notified: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keywords_become_the_path() {
        let p = SearchParams {
            keywords: " Rust Developer ".into(),
            ..SearchParams::default()
        };
        assert_eq!(
            search_url(&p, 1),
            "https://www.bayt.com/en/uae/jobs/rust-developer-jobs/"
        );
        assert_eq!(
            search_url(&p, 3),
            "https://www.bayt.com/en/uae/jobs/rust-developer-jobs/?page=3"
        );
    }

    /// Pages saved from the live site, cut down to the parts read.
    #[test]
    fn parses_saved_pages() {
        let jobs = parse_search(include_str!("../../tests/fixtures/bayt_search.html"), "q");
        assert_eq!(jobs.len(), 30);
        let j = &jobs[0];
        assert_eq!(j.id, "5495651");
        assert_eq!(j.title, "Software Test Engineer");
        assert_eq!(j.company, "InnovaziT (A Happiest Minds company)");
        assert_eq!(j.location, "Abu Dhabi, UAE");
        assert_eq!(j.posted.as_deref(), Some("2026-10-16"));
        assert_eq!(
            j.url,
            "https://www.bayt.com/en/uae/jobs/software-test-engineer-5495651/"
        );
        assert!(jobs.iter().all(|j| j.source == Source::Bayt
            && j.url.starts_with("https://www.bayt.com/en/uae/jobs/")
            && j.posted.is_some()));

        let d = parse_detail(include_str!("../../tests/fixtures/bayt_detail.html")).unwrap();
        assert!(d.description.contains("UAT Tester"));
        assert!(d
            .criteria
            .contains(&("Employment type".into(), "FULL_TIME".into())));
        assert!(parse_search("<html></html>", "q").is_empty());
    }
}
