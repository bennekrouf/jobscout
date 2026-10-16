//! MyCareersFuture (mycareersfuture.gov.sg), Singapore's government job board.
//!
//! Read from the JSON API its own site uses; robots.txt on the site allows
//! everything, and the API needs no key.
//!
//!   * search: `POST /v2/search?limit=20&page=N` (0-based) with
//!     `{"search": …, "sortBy": ["new_posting_date"]}`.
//!   * detail: `GET /v2/jobs/{uuid}`, the full posting.
//!
//! The board is Singapore only, so there is no location to pass.

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use super::text::markup_to_text;
use crate::model::{Job, JobDetail, SearchParams, Source, Status};

const API: &str = "https://api.mycareersfuture.gov.sg/v2";
pub const PAGE_SIZE: u32 = 20;

/// `page` is 1-based, like the other sources.
pub fn search_url(page: u32) -> String {
    format!("{API}/search?limit={PAGE_SIZE}&page={}", page.saturating_sub(1))
}

pub fn search_body(p: &SearchParams) -> Value {
    json!({ "search": p.keywords.trim(), "sortBy": ["new_posting_date"] })
}

pub fn detail_url(id: &str) -> String {
    format!("{API}/jobs/{id}")
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
    let description = markup_to_text(v["description"].as_str()?);
    if description.is_empty() {
        return None;
    }
    let mut criteria = vec![];
    let mut push = |name: &str, value: Option<String>| {
        if let Some(v) = value.filter(|v| !v.trim().is_empty()) {
            criteria.push((name.to_string(), v));
        }
    };
    push("Employment type", names(&v["employmentTypes"], "employmentType"));
    push("Seniority", names(&v["positionLevels"], "position"));
    push(
        "Experience",
        v["minimumYearsExperience"].as_u64().map(|n| format!("{n}+ years")),
    );
    push("Category", names(&v["categories"], "category"));
    push("Salary", salary(&v["salary"]));
    push("Skills", names(&v["skills"], "skill"));

    Some(JobDetail {
        description,
        criteria,
        applicants: v["metadata"]["totalNumberJobApplication"]
            .as_u64()
            .map(|n| format!("{n} applicants")),
        posted_ago: None,
        fetched_at: Utc::now(),
    })
}

fn to_job(r: &Value, query: &str, now: DateTime<Utc>) -> Option<Job> {
    let id = str_of(&r["uuid"])?;
    let meta = &r["metadata"];
    // An agency posting names the agency; the client, when it is shown, is
    // the employer.
    let company = str_of(&r["hiringCompany"]["name"])
        .or_else(|| str_of(&r["postedCompany"]["name"]))
        .unwrap_or_default();
    Some(Job {
        url: str_of(&meta["jobDetailsUrl"])
            .unwrap_or_else(|| format!("https://www.mycareersfuture.gov.sg/job/{id}")),
        id,
        source: Source::Mcf,
        title: str_of(&r["title"])?,
        company,
        location: names(&r["address"]["districts"], "location").unwrap_or_else(|| "Singapore".into()),
        posted: str_of(&meta["newPostingDate"]).map(|d| d.chars().take(10).collect()),
        salary: salary(&r["salary"]),
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
    v.as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

/// `[{"skill": "Rust"}, {"skill": "SQL"}]` → `Rust, SQL`.
fn names(list: &Value, field: &str) -> Option<String> {
    let names: Vec<String> = list.as_array()?.iter().filter_map(|x| str_of(&x[field])).collect();
    (!names.is_empty()).then(|| names.join(", "))
}

/// `SGD 7.5k–10k / month`.
fn salary(v: &Value) -> Option<String> {
    let k = |n: f64| {
        let k = n / 1000.0;
        if k.fract() == 0.0 { format!("{k}k") } else { format!("{k:.1}k") }
    };
    let range = match (v["minimum"].as_f64(), v["maximum"].as_f64()) {
        (Some(lo), Some(hi)) if lo > 0.0 && hi > lo => format!("{}–{}", k(lo), k(hi)),
        (Some(n), _) | (_, Some(n)) if n > 0.0 => k(n),
        _ => return None,
    };
    let unit = match v["type"]["salaryType"].as_str() {
        Some("Monthly") => " / month",
        Some("Annually" | "Yearly") => " / year",
        Some("Hourly") => " / hour",
        _ => "",
    };
    Some(format!("SGD {range}{unit}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_are_zero_based_pages() {
        assert_eq!(search_url(1), "https://api.mycareersfuture.gov.sg/v2/search?limit=20&page=0");
        assert_eq!(search_url(3), "https://api.mycareersfuture.gov.sg/v2/search?limit=20&page=2");
        let p = SearchParams { keywords: " rust ".into(), ..SearchParams::default() };
        assert_eq!(search_body(&p)["search"], "rust");
    }

    /// Answers saved from the live API.
    #[test]
    fn parses_saved_answers() {
        let jobs = parse_search(include_str!("../../tests/fixtures/mcf_search.json"), "q");
        assert_eq!(jobs.len(), 20);
        let j = &jobs[0];
        assert_eq!(j.id, "2ecc85f964536dbe4fdee2b32185b357");
        assert_eq!(j.title, "Software Developer");
        assert_eq!(j.company, "ES TECH CONSULTANCY PTE. LTD.");
        assert_eq!(j.location, "D26 Springleaf, Upper Thomson");
        assert_eq!(j.posted.as_deref(), Some("2026-10-16"));
        assert_eq!(j.salary.as_deref(), Some("SGD 8k–9k / month"));
        assert!(j.url.starts_with("https://www.mycareersfuture.gov.sg/job/"));
        assert!(jobs.iter().all(|j| j.source == Source::Mcf && !j.company.is_empty()));

        let d = parse_detail(include_str!("../../tests/fixtures/mcf_detail.json")).unwrap();
        assert!(d.description.contains("About the Opportunity"));
        assert!(d.criteria.contains(&("Salary".into(), "SGD 7.5k–10k / month".into())));
        assert!(d.criteria.contains(&("Experience".into(), "3+ years".into())));
        assert!(d.criteria.iter().any(|(k, v)| k == "Skills" && v.contains("Rust")));
        assert!(parse_detail("{}").is_none());
        assert!(parse_search("not json", "q").is_empty());
    }

    /// Against the live API: `cargo test live_ -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn live_mcf_search_and_detail() {
        use crate::services::fetch;
        let settings = crate::model::Settings { delay_ms: 1000, ..Default::default() };
        let p = SearchParams { keywords: "software engineer".into(), ..SearchParams::default() };
        let ttl = std::time::Duration::ZERO;
        let (b1, _) = fetch::post_json_cached(&search_url(1), &search_body(&p), ttl, &settings).await.unwrap();
        let (b2, _) = fetch::post_json_cached(&search_url(2), &search_body(&p), ttl, &settings).await.unwrap();
        let (page1, page2) = (parse_search(&b1, "q"), parse_search(&b2, "q"));
        assert_eq!(page1.len(), 20);
        assert!(page2.iter().all(|j| !page1.iter().any(|k| k.id == j.id)), "page 2 is new");
        let d = parse_detail(&fetch::get_json(&detail_url(&page1[0].id), &settings).await.unwrap());
        assert!(d.is_some_and(|d| !d.description.is_empty()));
    }
}
