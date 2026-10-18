//! StepStone (stepstone.de), Germany's largest commercial job board.
//!
//! The site sits behind Akamai. Search pages come through plain HTTP; job
//! pages do not (the connection is reset), so those are read in a browser,
//! like JobStreet's (see `sources`).
//!
//!   * search: `/jobs/{keywords as a slug}?q=…&ag=age_N&page=N`, all of
//!     Germany, 25 postings a page. robots.txt disallows query strings on
//!     `/jobs/` except those starting with `?q=`, so `q` always comes first.
//!     The page ships its results as one JSON object,
//!     `window.__PRELOADED_STATE__["app-unifiedResultlist"] = {…};`, read
//!     rather than the markup. Past the last page it lists nothing.
//!   * detail: the posting's own URL, one schema.org `JobPosting` in JSON-LD,
//!     the same shape the JobCloud boards publish.

use chrono::{DateTime, Utc};
use serde_json::Value;

use super::jobcloud;
use crate::model::{Job, JobDetail, Posted, SearchParams, Source, Status};

const SITE: &str = "https://www.stepstone.de";

/// `page` is 1-based, like the site's.
pub fn search_url(p: &SearchParams, page: u32) -> String {
    let keywords = p.keywords.trim();
    let mut url =
        reqwest::Url::parse(&format!("{SITE}/jobs/{}", slug(keywords))).expect("site URL parses");
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("q", keywords);
        if let Some(age) = age(p.posted) {
            q.append_pair("ag", age);
        }
        if page > 1 {
            q.append_pair("page", &page.to_string());
        }
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
    if words.is_empty() {
        "jobs".into()
    } else {
        words.join("-")
    }
}

/// The site only filters on the last day and the last week; a month is left
/// to the filter on our side.
fn age(posted: Posted) -> Option<&'static str> {
    match posted {
        Posted::Day => Some("age_1"),
        Posted::Week => Some("age_7"),
        Posted::Month | Posted::Any => None,
    }
}

pub fn parse_search(html: &str, query: &str) -> Vec<Job> {
    let now = Utc::now();
    let Some(state) = preloaded_state(html) else {
        return vec![];
    };
    state["searchResults"]["items"]
        .as_array()
        .map(|items| items.iter().filter_map(|i| to_job(i, query, now)).collect())
        .unwrap_or_default()
}

/// `None` when the page has no `JobPosting` with a description: a removed
/// posting, or a block page.
pub fn parse_detail(html: &str) -> Option<JobDetail> {
    jobcloud::parse_detail(html)
}

fn preloaded_state(html: &str) -> Option<Value> {
    const MARK: &str = r#"window.__PRELOADED_STATE__["app-unifiedResultlist"] = "#;
    let rest = &html[html.find(MARK)? + MARK.len()..];
    serde_json::Deserializer::from_str(rest)
        .into_iter::<Value>()
        .next()?
        .ok()
}

fn to_job(i: &Value, query: &str, now: DateTime<Utc>) -> Option<Job> {
    let id = match &i["id"] {
        Value::Number(n) => n.to_string(),
        v => str_of(v)?,
    };
    // The link carries a tracking query; the posting is the path.
    let url = str_of(&i["url"])?;
    let url = url.split('?').next().unwrap_or(&url).to_string();
    Some(Job {
        id,
        source: Source::StepStone,
        title: str_of(&i["title"])?,
        company: str_of(&i["companyName"]).unwrap_or_default(),
        location: str_of(&i["location"]).unwrap_or_else(|| "Germany".into()),
        url,
        posted: str_of(&i["datePosted"]).map(|d| d.chars().take(10).collect()),
        salary: salary(&i["unifiedSalary"]),
        found_at: now,
        query: query.to_string(),
        status: Status::New,
        detail: None,
        detail_failed_at: None,
        score: None,
        notified: false,
    })
}

fn str_of(v: &Value) -> Option<String> {
    v.as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// `EUR 58k–77k / year`. Most postings give no salary and StepStone shows
/// its own estimate instead; that one is marked as such.
fn salary(v: &Value) -> Option<String> {
    if v["salaryAvailable"].as_bool() != Some(true) {
        return None;
    }
    let k = |n: f64| format!("{}k", (n / 1000.0).round());
    let positive = |key: &str| v[key].as_f64().filter(|n| *n > 0.0);
    let range = match (positive("min"), positive("max"), positive("exactValue")) {
        (_, _, Some(n)) => k(n),
        (Some(lo), Some(hi), _) if hi > lo => format!("{}–{}", k(lo), k(hi)),
        (Some(n), _, _) | (_, Some(n), _) => k(n),
        _ => return None,
    };
    let currency = str_of(&v["currencyIso"]).unwrap_or_else(|| "EUR".into());
    let unit = match v["period"].as_str() {
        Some("Annually" | "Yearly") => " / year",
        Some("Monthly") => " / month",
        Some("Hourly") => " / hour",
        _ => "",
    };
    let estimate = if v["salaryType"].as_str() == Some("Predicted") {
        " (StepStone estimate)"
    } else {
        ""
    };
    Some(format!("{currency} {range}{unit}{estimate}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn q_comes_first() {
        let mut p = SearchParams {
            keywords: " Rust Developer ".into(),
            ..SearchParams::default()
        };
        assert_eq!(
            search_url(&p, 1),
            "https://www.stepstone.de/jobs/rust-developer?q=Rust+Developer&ag=age_7"
        );
        p.posted = Posted::Any;
        p.keywords = "C++".into();
        assert_eq!(
            search_url(&p, 2),
            "https://www.stepstone.de/jobs/c?q=C%2B%2B&page=2"
        );
        p.keywords = "Softwareentwickler München".into();
        assert!(search_url(&p, 1)
            .starts_with("https://www.stepstone.de/jobs/softwareentwickler-m%C3%BCnchen?q="));
    }

    /// Pages saved from the live site, cut down to the parts read.
    #[test]
    fn parses_saved_pages() {
        let jobs = parse_search(
            include_str!("../../tests/fixtures/stepstone_search.html"),
            "q",
        );
        assert_eq!(jobs.len(), 25);
        let j = &jobs[0];
        assert_eq!(j.id, "14624450");
        assert_eq!(j.title, "Software Engineer Azure Cloud Operations");
        assert_eq!(j.company, "Hannover Rück SE");
        assert_eq!(j.location, "Hannover");
        assert_eq!(j.posted.as_deref(), Some("2026-10-16"));
        assert_eq!(
            j.salary.as_deref(),
            Some("EUR 58k–77k / year (StepStone estimate)")
        );
        assert_eq!(
            j.url,
            "https://www.stepstone.de/stellenangebote--Software-Engineer-Azure-Cloud-Operations-Hannover-Hannover-Rueck-SE--14624450-inline.html"
        );
        assert!(jobs.iter().any(|j| j.salary.is_none()));
        assert!(jobs
            .iter()
            .all(|j| j.source == Source::StepStone && !j.url.contains('?')));

        let d = parse_detail(include_str!("../../tests/fixtures/stepstone_detail.html")).unwrap();
        assert!(d.description.contains("Hannover Rück"));
        assert!(d
            .criteria
            .contains(&("Employment type".into(), "FULL_TIME".into())));
        assert!(parse_search("<html></html>", "q").is_empty());
        assert!(parse_detail("<html></html>").is_none());
    }

    /// Search against the live site, over plain HTTP: `cargo test live_ --
    /// --ignored`. Job pages need a browser, which a test does not have.
    #[tokio::test]
    #[ignore]
    async fn live_stepstone_search_paging() {
        use crate::services::fetch;
        let settings = crate::model::Settings {
            delay_ms: 1500,
            ..Default::default()
        };
        let p = SearchParams {
            keywords: "software engineer".into(),
            ..SearchParams::default()
        };
        let (url1, url2) = (search_url(&p, 1), search_url(&p, 2));
        let page1 = parse_search(&fetch::get(&url1, &settings).await.unwrap(), "q");
        let page2 = parse_search(&fetch::get(&url2, &settings).await.unwrap(), "q");
        assert_eq!(page1.len(), 25);
        assert!(
            page2.iter().any(|j| !page1.iter().any(|k| k.id == j.id)),
            "page 2 is new"
        );
        assert!(page1.iter().all(|j| !j.company.is_empty()));
    }
}
