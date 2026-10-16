//! JobStreet Singapore (sg.jobstreet.com), SEEK's board for South-East Asia.
//!
//! The site is behind a Cloudflare challenge that plain HTTP never gets
//! past, so every request goes through a browser: the built-in WebKit one,
//! or Camoufox when that is the backend in Settings (see `sources`). Both
//! wait out the challenge.
//!
//!   * search: `/jobs?keywords=…&where=…&daterange=N&page=N`, 30 postings a
//!     page. robots.txt disallows query strings except those starting with
//!     `?keywords`, so `keywords` always comes first.
//!   * detail: `/job/{id}`.
//!
//! Both pages are a SEEK app that ships its state as one JSON object,
//! `window.SEEK_REDUX_DATA = {…};`. That object is read rather than the
//! markup: it holds the same data, in a steadier shape.

use chrono::{DateTime, Utc};
use serde_json::Value;

use super::text::markup_to_text;
use crate::model::{Job, JobDetail, Posted, SearchParams, Source, Status};

const SITE: &str = "https://sg.jobstreet.com";

pub fn search_url(p: &SearchParams, page: u32) -> String {
    let mut url = reqwest::Url::parse(&format!("{SITE}/jobs")).expect("constant URL parses");
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("keywords", p.keywords.trim());
        if let Some(loc) = location(&p.location) {
            q.append_pair("where", loc);
        }
        if let Some(days) = daterange(p.posted) {
            q.append_pair("daterange", days);
        }
        if page > 1 {
            q.append_pair("page", &page.to_string());
        }
    }
    url.to_string()
}

pub fn detail_url(id: &str) -> String {
    format!("{SITE}/job/{id}")
}

/// The board is Singapore only: the country as a location adds nothing.
fn location(loc: &str) -> Option<&str> {
    let l = loc.trim();
    (!["", "singapore", "sg"].contains(&l.to_lowercase().as_str())).then_some(l)
}

/// JobStreet's own `daterange` values, in days.
fn daterange(posted: Posted) -> Option<&'static str> {
    match posted {
        Posted::Any => None,
        Posted::Day => Some("1"),
        Posted::Week => Some("7"),
        Posted::Month => Some("31"),
    }
}

pub fn parse_search(html: &str, query: &str) -> Vec<Job> {
    let now = Utc::now();
    let Some(state) = redux_state(html) else { return vec![] };
    state["results"]["results"]["jobs"]
        .as_array()
        .map(|jobs| jobs.iter().filter_map(|j| to_job(j, query, now)).collect())
        .unwrap_or_default()
}

/// `None` when the page holds no posting with a description: a removed job,
/// or a page that is not a job page at all.
pub fn parse_detail(html: &str) -> Option<JobDetail> {
    let state = redux_state(html)?;
    let result = &state["jobdetails"]["result"];
    let job = &result["job"];
    let description = markup_to_text(job["content"].as_str()?);
    if description.is_empty() {
        return None;
    }
    let mut criteria = vec![];
    let mut push = |name: &str, value: Option<String>| {
        if let Some(v) = value.filter(|v| !v.trim().is_empty()) {
            criteria.push((name.to_string(), v));
        }
    };
    push("Work type", str_of(&job["workTypes"]["label"]));
    push("Work arrangement", labels(&result["workArrangements"]["arrangements"]));
    push("Classification", labels(&job["classifications"]));
    push("Salary", str_of(&job["salary"]["label"]));
    push("Advertiser", str_of(&job["advertiser"]["name"]));
    if job["isExpired"].as_bool() == Some(true) {
        push("Status", Some("Expired".into()));
    }

    Some(JobDetail {
        description,
        criteria,
        applicants: None,
        posted_ago: str_of(&job["listedAt"]["label"]),
        fetched_at: Utc::now(),
    })
}

fn to_job(j: &Value, query: &str, now: DateTime<Utc>) -> Option<Job> {
    let id = str_of(&j["id"])?;
    let location: Vec<String> = j["locations"]
        .as_array()
        .map(|ls| ls.iter().filter_map(|l| str_of(&l["label"])).collect())
        .unwrap_or_default();
    Some(Job {
        url: detail_url(&id),
        id,
        source: Source::JobStreet,
        title: str_of(&j["title"])?,
        // A private advertiser has no company name, only "Private Advertiser".
        company: str_of(&j["companyName"])
            .or_else(|| str_of(&j["advertiser"]["description"]))
            .unwrap_or_default(),
        location: if location.is_empty() { "Singapore".into() } else { location.join(", ") },
        posted: str_of(&j["listingDate"]).map(|d| d.chars().take(10).collect()),
        salary: str_of(&j["salaryLabel"]),
        found_at: now,
        query: query.to_string(),
        status: Status::New,
        detail: None,
        detail_failed_at: None,
        score: None,
        notified: false,
    })
}

/// The object assigned to `window.SEEK_REDUX_DATA`. It is JavaScript, not
/// JSON: read the first value and stop, and treat `undefined` as `null`.
fn redux_state(html: &str) -> Option<Value> {
    const MARK: &str = "window.SEEK_REDUX_DATA = ";
    let rest = &html[html.find(MARK)? + MARK.len()..];
    let parse = |s: &str| serde_json::Deserializer::from_str(s).into_iter::<Value>().next()?.ok();
    parse(rest).or_else(|| {
        let end = rest.find(";\n").unwrap_or(rest.len());
        parse(&rest[..end].replace(":undefined", ":null"))
    })
}

fn str_of(v: &Value) -> Option<String> {
    v.as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

/// `[{"label": "On-site"}, {"label": "Hybrid"}]` → `On-site, Hybrid`.
fn labels(list: &Value) -> Option<String> {
    let labels: Vec<String> = list.as_array()?.iter().filter_map(|x| str_of(&x["label"])).collect();
    (!labels.is_empty()).then(|| labels.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keywords_come_first() {
        let mut p = SearchParams { keywords: "rust dev".into(), location: "Singapore".into(), ..SearchParams::default() };
        assert_eq!(search_url(&p, 1), "https://sg.jobstreet.com/jobs?keywords=rust+dev&daterange=7");
        p.location = "Central Region".into();
        p.posted = Posted::Any;
        assert_eq!(
            search_url(&p, 3),
            "https://sg.jobstreet.com/jobs?keywords=rust+dev&where=Central+Region&page=3"
        );
    }

    /// Trimmed from real pages: the SEEK state with only what is read here.
    #[test]
    fn parses_saved_pages() {
        let jobs = parse_search(include_str!("../../tests/fixtures/jobstreet_search.html"), "q");
        assert_eq!(jobs.len(), 10);
        let j = &jobs[0];
        assert_eq!(j.id, "95305071");
        assert_eq!(j.title, "Innovation Software Engineer");
        assert_eq!(j.company, "Private Advertiser");
        assert_eq!(j.location, "Central Region");
        assert_eq!(j.posted.as_deref(), Some("2026-10-16"));
        assert_eq!(j.salary.as_deref(), Some("$4,500 – $6,000 per month"));
        assert_eq!(j.url, "https://sg.jobstreet.com/job/95305071");
        assert!(jobs.iter().all(|j| j.source == Source::JobStreet && !j.company.is_empty()));

        let d = parse_detail(include_str!("../../tests/fixtures/jobstreet_detail.html")).unwrap();
        assert!(d.description.contains("Responsibilities"));
        assert!(d.criteria.contains(&("Work type".into(), "Full time".into())));
        assert!(d.criteria.contains(&("Work arrangement".into(), "On-site".into())));
        assert!(d.criteria.contains(&("Salary".into(), "Up to $6k + AWS + VB".into())));
        assert!(parse_detail("<html>Just a moment...</html>").is_none());
        assert!(parse_search("<html></html>", "q").is_empty());
    }

    #[test]
    fn reads_state_with_undefined() {
        let html = "<script>window.SEEK_REDUX_DATA = {\"a\":undefined,\"b\":1};\n</script>";
        assert_eq!(redux_state(html).unwrap()["b"], 1);
    }

    /// Against the live site, through Camoufox: `CAMOUFOX_PYTHON=/path/to/python
    /// cargo test live_jobstreet -- --ignored` (a Python with camoufox installed).
    /// Camoufox, because the built-in browser needs the main thread, which a
    /// test does not run on.
    #[tokio::test]
    #[ignore]
    async fn live_jobstreet_search_paging_and_detail() {
        use crate::services::sources;
        let settings = crate::model::Settings {
            delay_ms: 1500,
            backend: crate::model::Backend::Camoufox,
            python: std::env::var("CAMOUFOX_PYTHON").unwrap_or_else(|_| "python3".into()),
            ..Default::default()
        };
        let p = SearchParams {
            keywords: "software engineer".into(),
            location: "Singapore".into(),
            ..SearchParams::default()
        };
        let ttl = std::time::Duration::ZERO;
        let page = |n| sources::fetch_search(Source::JobStreet, &p, n, 0, ttl, &settings);
        let page1 = parse_search(&page(1).await.unwrap().0, "q");
        let page2 = parse_search(&page(2).await.unwrap().0, "q");
        assert!(page1.len() >= 20, "page 1: {}", page1.len());
        assert!(page2.iter().any(|j| !page1.iter().any(|k| k.id == j.id)), "page 2 is new");
        let far = parse_search(&page(999).await.unwrap().0, "q");
        assert!(far.is_empty(), "past the end: {}", far.len());
        let d = parse_detail(&sources::fetch_detail(&page1[0], &settings).await.unwrap()).expect("detail parses");
        eprintln!("{} — {} chars, {:?}", page1[0].title, d.description.len(), d.criteria);
    }
}
