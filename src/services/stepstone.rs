//! The StepStone Group's boards: StepStone (stepstone.de), Germany's largest
//! commercial board, and Totaljobs (totaljobs.com), the UK's. One platform,
//! so one reader.
//!
//! Both sit behind Akamai. Search pages come through plain HTTP; job pages
//! do not (the connection is reset), so those are read in a browser, like
//! JobStreet's (see `sources`).
//!
//!   * search: `/jobs/{keywords as a slug}?q=…`, the whole country, 25
//!     postings a page. Both robots.txt files disallow query strings on
//!     `/jobs/` except those starting with `?q=`, so `q` always comes first.
//!     StepStone's then allows `&ag=age_N&page=N`; Totaljobs' disallows
//!     anything after `q` (`/jobs/*?q=*&`), so a Totaljobs search is its first
//!     page only, with dates filtered on our side.
//!     The page ships its results as one JSON object,
//!     `window.__PRELOADED_STATE__["app-unifiedResultlist"] = {…};`, read
//!     rather than the markup. Past the last page it lists nothing.
//!   * detail: the posting's own URL, one schema.org `JobPosting` in JSON-LD,
//!     the same shape the JobCloud boards publish.

use chrono::{DateTime, Utc};
use serde_json::Value;

use super::jobcloud;
use crate::model::{Job, JobDetail, Posted, SearchParams, Source, Status};

fn site(source: Source) -> &'static str {
    match source {
        Source::Totaljobs => "https://www.totaljobs.com",
        _ => "https://www.stepstone.de",
    }
}

fn country_name(source: Source) -> &'static str {
    match source {
        Source::Totaljobs => "United Kingdom",
        _ => "Germany",
    }
}

/// `page` is 1-based, like the site's. `None` for a page robots.txt does not
/// allow: past the first, on Totaljobs.
pub fn search_url(source: Source, p: &SearchParams, page: u32) -> Option<String> {
    let paged = source != Source::Totaljobs;
    if page > 1 && !paged {
        return None;
    }
    let keywords = p.keywords.trim();
    let mut url = reqwest::Url::parse(&format!("{}/jobs/{}", site(source), slug(keywords)))
        .expect("site URL parses");
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("q", keywords);
        if let Some(age) = age(p.posted).filter(|_| paged) {
            q.append_pair("ag", age);
        }
        if page > 1 {
            q.append_pair("page", &page.to_string());
        }
    }
    Some(url.to_string())
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

/// StepStone only filters on the last day and the last week; a month is left
/// to the filter on our side.
fn age(posted: Posted) -> Option<&'static str> {
    match posted {
        Posted::Day => Some("age_1"),
        Posted::Week => Some("age_7"),
        Posted::Month | Posted::Any => None,
    }
}

pub fn parse_search(source: Source, html: &str, query: &str) -> Vec<Job> {
    let now = Utc::now();
    let Some(state) = preloaded_state(html) else {
        return vec![];
    };
    state["searchResults"]["items"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|i| to_job(source, i, query, now))
                .collect()
        })
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

fn to_job(source: Source, i: &Value, query: &str, now: DateTime<Utc>) -> Option<Job> {
    let id = match &i["id"] {
        Value::Number(n) => n.to_string(),
        v => str_of(v)?,
    };
    // The link carries a tracking query; the posting is the path.
    let url = str_of(&i["url"])?;
    let url = url.split('?').next().unwrap_or(&url).to_string();
    Some(Job {
        id,
        source,
        title: str_of(&i["title"])?,
        company: str_of(&i["companyName"]).unwrap_or_default(),
        location: str_of(&i["location"]).unwrap_or_else(|| country_name(source).into()),
        url,
        posted: str_of(&i["datePosted"]).map(|d| d.chars().take(10).collect()),
        // The site's figures when it has them, else the employer's own words.
        salary: salary(source, &i["unifiedSalary"]).or_else(|| str_of(&i["salary"])),
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

/// `EUR 58k–77k / year`. Most StepStone postings give no salary and the
/// site shows its own estimate instead; that one is marked as such.
fn salary(source: Source, v: &Value) -> Option<String> {
    if v["salaryAvailable"].as_bool() != Some(true) {
        return None;
    }
    let amount = |n: f64| {
        if n >= 1000.0 {
            format!("{}k", (n / 1000.0).round())
        } else if n.fract() == 0.0 {
            format!("{n}")
        } else {
            format!("{n:.2}")
        }
    };
    let positive = |key: &str| v[key].as_f64().filter(|n| *n > 0.0);
    let range = match (positive("min"), positive("max"), positive("exactValue")) {
        (_, _, Some(n)) => amount(n),
        (Some(lo), Some(hi), _) if hi > lo => format!("{}–{}", amount(lo), amount(hi)),
        (Some(n), _, _) | (_, Some(n), _) => amount(n),
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
        format!(" ({} estimate)", source.label())
    } else {
        String::new()
    };
    Some(format!("{currency} {range}{unit}{estimate}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ss(p: &SearchParams, page: u32) -> String {
        search_url(Source::StepStone, p, page).unwrap()
    }

    #[test]
    fn q_comes_first() {
        let mut p = SearchParams {
            keywords: " Rust Developer ".into(),
            ..SearchParams::default()
        };
        assert_eq!(
            ss(&p, 1),
            "https://www.stepstone.de/jobs/rust-developer?q=Rust+Developer&ag=age_7"
        );
        p.posted = Posted::Any;
        p.keywords = "C++".into();
        assert_eq!(
            ss(&p, 2),
            "https://www.stepstone.de/jobs/c?q=C%2B%2B&page=2"
        );
        p.keywords = "Softwareentwickler München".into();
        assert!(ss(&p, 1)
            .starts_with("https://www.stepstone.de/jobs/softwareentwickler-m%C3%BCnchen?q="));
    }

    /// Pages saved from the live site, cut down to the parts read.
    #[test]
    fn parses_saved_pages() {
        let jobs = parse_search(
            Source::StepStone,
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
        assert!(parse_search(Source::StepStone, "<html></html>", "q").is_empty());
        assert!(parse_detail("<html></html>").is_none());
    }

    #[test]
    fn totaljobs_is_one_page_with_q_alone() {
        let p = SearchParams {
            keywords: "Software Engineer".into(),
            ..SearchParams::default()
        };
        assert_eq!(
            search_url(Source::Totaljobs, &p, 1).as_deref(),
            Some("https://www.totaljobs.com/jobs/software-engineer?q=Software+Engineer")
        );
        assert_eq!(search_url(Source::Totaljobs, &p, 2), None);
    }

    #[test]
    fn parses_a_saved_totaljobs_page() {
        let jobs = parse_search(
            Source::Totaljobs,
            include_str!("../../tests/fixtures/totaljobs_search.html"),
            "q",
        );
        assert_eq!(jobs.len(), 25);
        assert_eq!(jobs[0].id, "108199323");
        assert_eq!(jobs[0].company, "National Grid");
        assert_eq!(jobs[0].location, "West Midlands");
        assert_eq!(
            jobs[0].url,
            "https://www.totaljobs.com/job/software-engineer/national-grid-job108199323"
        );
        assert_eq!(jobs[1].salary.as_deref(), Some("£39.71 - 53.51 per hour"));
        assert_eq!(jobs[8].salary, None, "an empty salary is none");
        assert!(jobs.iter().all(|j| j.source == Source::Totaljobs));
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
        let (url1, url2) = (ss(&p, 1), ss(&p, 2));
        let page1 = parse_search(
            Source::StepStone,
            &fetch::get(&url1, &settings).await.unwrap(),
            "q",
        );
        let page2 = parse_search(
            Source::StepStone,
            &fetch::get(&url2, &settings).await.unwrap(),
            "q",
        );
        assert_eq!(page1.len(), 25);
        assert!(
            page2.iter().any(|j| !page1.iter().any(|k| k.id == j.id)),
            "page 2 is new"
        );
        assert!(page1.iter().all(|j| !j.company.is_empty()));
    }

    /// Totaljobs' one allowed page, live: `cargo test live_totaljobs -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn live_totaljobs_search() {
        use crate::services::fetch;
        let settings = crate::model::Settings {
            delay_ms: 1500,
            ..Default::default()
        };
        let p = SearchParams {
            keywords: "software engineer".into(),
            ..SearchParams::default()
        };
        let url = search_url(Source::Totaljobs, &p, 1).unwrap();
        let jobs = parse_search(
            Source::Totaljobs,
            &fetch::get(&url, &settings).await.unwrap(),
            "q",
        );
        assert_eq!(jobs.len(), 25);
        assert!(jobs
            .iter()
            .all(|j| j.url.starts_with("https://www.totaljobs.com/job/")));
    }
}
