//! Reed (reed.co.uk), one of the UK's largest job boards, through its
//! official jobseeker API.
//!
//! The site's own search is off limits (its robots.txt disallows `/jobs/`),
//! so this source needs a free API key from reed.co.uk/developers. The key
//! is read from the environment variable Settings names, like the scoring
//! model's, and never saved. It goes as the Basic auth user name, with an
//! empty password.
//!
//!   * search: `GET /api/1.0/search?keywords=…&resultsToTake=100&resultsToSkip=N`,
//!     the whole UK. The API has no date filter, so dates are filtered on our
//!     side.
//!   * detail: `GET /api/1.0/jobs/{id}`, the full description and contract.

use base64::Engine;
use chrono::{DateTime, NaiveDate, Utc};
use serde_json::Value;

use super::fetch::{self, FetchError};
use super::llm;
use super::text::markup_to_text;
use crate::model::{Job, JobDetail, SearchParams, Settings, Source, Status};

const API: &str = "https://www.reed.co.uk/api/1.0";
/// The most the API returns in one answer.
pub const PAGE_SIZE: u32 = 100;

/// `page` is 1-based; each page skips the ones before it.
pub fn search_url(p: &SearchParams, page: u32) -> String {
    let mut url = reqwest::Url::parse(&format!("{API}/search")).expect("constant URL parses");
    url.query_pairs_mut()
        .append_pair("keywords", p.keywords.trim())
        .append_pair("resultsToTake", &PAGE_SIZE.to_string())
        .append_pair(
            "resultsToSkip",
            &((page.max(1) - 1) * PAGE_SIZE).to_string(),
        );
    url.to_string()
}

pub fn detail_url(id: &str) -> String {
    format!("{API}/jobs/{id}")
}

pub async fn fetch_search(
    p: &SearchParams,
    page: u32,
    ttl: std::time::Duration,
    settings: &Settings,
) -> Result<(String, bool), FetchError> {
    let auth = auth(settings)?;
    let headers = [("Authorization", auth.as_str())];
    unauthorised_is_setup(
        fetch::get_json_cached(&search_url(p, page), &headers, ttl, settings).await,
    )
}

pub async fn fetch_detail(id: &str, settings: &Settings) -> Result<String, FetchError> {
    let auth = auth(settings)?;
    let headers = [("Authorization", auth.as_str())];
    unauthorised_is_setup(fetch::get_json(&detail_url(id), &headers, settings).await)
}

/// `Basic base64("{key}:")`.
fn auth(settings: &Settings) -> Result<String, FetchError> {
    let key = llm::api_key(&settings.reed_key_env)
        .map_err(FetchError::Setup)?
        .ok_or_else(|| {
            FetchError::Setup(
                "Reed needs a free API key from reed.co.uk/developers: name its \
                 environment variable in Settings"
                    .into(),
            )
        })?;
    let token = base64::engine::general_purpose::STANDARD.encode(format!("{key}:"));
    Ok(format!("Basic {token}"))
}

/// A refused key is a setting to fix, not this job's fault.
fn unauthorised_is_setup<T>(r: Result<T, FetchError>) -> Result<T, FetchError> {
    match r {
        Err(FetchError::Status(401 | 403)) => Err(FetchError::Setup(
            "Reed refused the API key in Settings".into(),
        )),
        other => other,
    }
}

pub fn parse_search(body: &str, query: &str) -> Vec<Job> {
    let now = Utc::now();
    let v: Value = serde_json::from_str(body).unwrap_or_default();
    v["results"]
        .as_array()
        .map(|rs| rs.iter().filter_map(|r| to_job(r, query, now)).collect())
        .unwrap_or_default()
}

/// `None` when the answer has no description: a removed posting.
pub fn parse_detail(body: &str) -> Option<JobDetail> {
    let v: Value = serde_json::from_str(body).ok()?;
    let description = markup_to_text(v["jobDescription"].as_str()?);
    if description.is_empty() {
        return None;
    }
    let mut criteria = vec![];
    let mut push = |name: &str, value: Option<String>| {
        if let Some(v) = value.filter(|v| !v.trim().is_empty()) {
            criteria.push((name.to_string(), v));
        }
    };
    push("Contract", str_of(&v["contractType"]));
    push("Work hours", work_hours(&v));
    push("Salary", str_of(&v["salary"]).or_else(|| salary(&v)));
    push("Closes", str_of(&v["expirationDate"]));
    // Postings answered on the employer's own site.
    push(
        "Apply at",
        str_of(&v["externalUrl"]).and_then(|u| {
            reqwest::Url::parse(&u)
                .ok()
                .and_then(|u| u.host_str().map(str::to_string))
        }),
    );

    Some(JobDetail {
        description,
        criteria,
        applicants: v["applicationCount"]
            .as_u64()
            .map(|n| format!("{n} applicants")),
        posted_ago: None,
        fetched_at: Utc::now(),
    })
}

fn to_job(r: &Value, query: &str, now: DateTime<Utc>) -> Option<Job> {
    let id = match &r["jobId"] {
        Value::Number(n) => n.to_string(),
        v => str_of(v)?,
    };
    Some(Job {
        url: str_of(&r["jobUrl"]).unwrap_or_else(|| format!("https://www.reed.co.uk/jobs/{id}")),
        title: str_of(&r["jobTitle"])?,
        company: str_of(&r["employerName"]).unwrap_or_default(),
        location: str_of(&r["locationName"]).unwrap_or_else(|| "United Kingdom".into()),
        posted: str_of(&r["date"]).and_then(|d| iso_date(&d)),
        salary: salary(r),
        id,
        source: Source::Reed,
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

/// Reed writes dates the British way: `18/10/2026` → `2026-10-18`.
fn iso_date(d: &str) -> Option<String> {
    NaiveDate::parse_from_str(d.trim(), "%d/%m/%Y")
        .ok()
        .map(|d| d.format("%Y-%m-%d").to_string())
}

fn work_hours(v: &Value) -> Option<String> {
    match (
        v["fullTime"].as_bool() == Some(true),
        v["partTime"].as_bool() == Some(true),
    ) {
        (true, true) => Some("Full-time or part-time".into()),
        (true, false) => Some("Full-time".into()),
        (false, true) => Some("Part-time".into()),
        (false, false) => None,
    }
}

/// `GBP 40k–50k`, from the minimum and maximum. Reed does not say per what
/// in a search result, so no unit is guessed.
fn salary(v: &Value) -> Option<String> {
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
    let range = match (positive("minimumSalary"), positive("maximumSalary")) {
        (Some(lo), Some(hi)) if hi > lo => format!("{}–{}", amount(lo), amount(hi)),
        (Some(n), _) | (_, Some(n)) => amount(n),
        _ => return None,
    };
    let currency = str_of(&v["currency"]).unwrap_or_else(|| "GBP".into());
    Some(format!("{currency} {range}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_skip_whole_pages() {
        let p = SearchParams {
            keywords: " rust developer ".into(),
            ..SearchParams::default()
        };
        assert_eq!(
            search_url(&p, 1),
            "https://www.reed.co.uk/api/1.0/search?keywords=rust+developer&resultsToTake=100&resultsToSkip=0"
        );
        assert!(search_url(&p, 3).ends_with("&resultsToSkip=200"));
        assert_eq!(
            detail_url("51234567"),
            "https://www.reed.co.uk/api/1.0/jobs/51234567"
        );
    }

    /// Answers in the API's documented shape.
    #[test]
    fn parses_search_and_detail_answers() {
        let jobs = parse_search(include_str!("../../tests/fixtures/reed_search.json"), "q");
        assert_eq!(jobs.len(), 2);
        let j = &jobs[0];
        assert_eq!(j.id, "51234567");
        assert_eq!(j.title, "Rust Developer");
        assert_eq!(j.company, "Example Systems Ltd");
        assert_eq!(j.location, "Manchester");
        assert_eq!(j.posted.as_deref(), Some("2026-10-16"));
        assert_eq!(j.salary.as_deref(), Some("GBP 55k–70k"));
        assert_eq!(j.url, "https://www.reed.co.uk/jobs/rust-developer/51234567");
        assert_eq!(jobs[1].salary, None);
        assert!(jobs.iter().all(|j| j.source == Source::Reed));

        let d = parse_detail(include_str!("../../tests/fixtures/reed_detail.json")).unwrap();
        assert!(d.description.contains("• Build services in Rust"));
        assert!(d
            .criteria
            .contains(&("Contract".into(), "Permanent".into())));
        assert!(d
            .criteria
            .contains(&("Work hours".into(), "Full-time".into())));
        assert!(d
            .criteria
            .contains(&("Salary".into(), "£55,000 - £70,000 per annum".into())));
        assert!(d
            .criteria
            .contains(&("Apply at".into(), "careers.example.co.uk".into())));
        assert_eq!(d.applicants.as_deref(), Some("12 applicants"));
        assert!(parse_detail("{}").is_none());
        assert!(parse_search("not json", "q").is_empty());
    }

    #[test]
    fn auth_is_the_key_as_user_name() {
        std::env::set_var("JOBSCOUT_TEST_REED_KEY", "k3y");
        let settings = Settings {
            reed_key_env: "JOBSCOUT_TEST_REED_KEY".into(),
            ..Settings::default()
        };
        // base64("k3y:")
        assert_eq!(auth(&settings).unwrap(), "Basic azN5Og==");
    }

    /// Against the live API: `REED_API_KEY=… cargo test live_reed -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn live_reed_search_and_detail() {
        let settings = Settings {
            delay_ms: 1000,
            ..Default::default()
        };
        let p = SearchParams {
            keywords: "software engineer".into(),
            ..SearchParams::default()
        };
        let ttl = std::time::Duration::ZERO;
        let page1 = parse_search(&fetch_search(&p, 1, ttl, &settings).await.unwrap().0, "q");
        let page2 = parse_search(&fetch_search(&p, 2, ttl, &settings).await.unwrap().0, "q");
        assert_eq!(page1.len(), PAGE_SIZE as usize);
        assert!(
            page2.iter().any(|j| !page1.iter().any(|k| k.id == j.id)),
            "page 2 is new"
        );
        let d = parse_detail(&fetch_detail(&page1[0].id, &settings).await.unwrap());
        assert!(d.is_some_and(|d| !d.description.is_empty()));
    }
}
