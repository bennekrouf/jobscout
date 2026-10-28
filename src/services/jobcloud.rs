//! jobs.ch and jobup.ch — both run on JobCloud: same pages, same markup.
//!
//! Read from the public pages, never from `/api/`: robots.txt on both sites
//! disallows the API. The pages carry the same data as schema.org
//! `JobPosting` JSON-LD — the structured data job boards publish for search
//! engines — which is a far steadier contract than CSS class names.
//!
//!   * search: `/en/vacancies/?term=…&page=N` (jobup: `/en/jobs/`), all of
//!     Switzerland,
//!     about 20 postings in an `ItemList`, each a short `JobPosting`.
//!   * detail: the posting's own URL, one full `JobPosting`.

use chrono::{DateTime, Utc};
use scraper::Html;
use serde_json::Value;

use super::text::markup_to_text;
use crate::model::{Job, JobDetail, SearchParams, Source, Status};

pub fn search_url(source: Source, p: &SearchParams, page: u32) -> String {
    let base = match source {
        Source::Jobup => "https://www.jobup.ch/en/jobs/",
        _ => "https://www.jobs.ch/en/vacancies/",
    };
    let mut url = super::fetch::site_url(base);
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("term", p.keywords.trim());
        if page > 1 {
            q.append_pair("page", &page.to_string());
        }
    }
    url.to_string()
}

/// Every posting on one search page, unfiltered: the caller tells the end of
/// the results from a page that repeats what it has already seen.
pub fn parse_search(source: Source, html: &str, query: &str) -> Vec<Job> {
    let now = Utc::now();
    json_ld(html)
        .iter()
        .filter(|v| is_type(v, "ItemList"))
        .flat_map(|list| {
            list["itemListElement"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .filter_map(|el| {
            let item = &el["item"];
            is_type(item, "JobPosting").then(|| to_job(source, item, query, now))?
        })
        .collect()
}

/// The full posting; `None` when the page has no `JobPosting` with a
/// description — a removed posting lands on a search page instead.
pub fn parse_detail(html: &str) -> Option<JobDetail> {
    let posting = json_ld(html)
        .into_iter()
        .find(|v| is_type(v, "JobPosting"))?;
    let description = markup_to_text(posting["description"].as_str()?);
    if description.is_empty() {
        return None;
    }
    let mut criteria = vec![];
    let mut push = |name: &str, value: Option<String>| {
        if let Some(v) = value.filter(|v| !v.trim().is_empty()) {
            criteria.push((name.to_string(), v));
        }
    };
    push("Employment type", str_of(&posting["employmentType"]));
    push("Work hours", str_of(&posting["workHours"]));
    push("Industry", str_of(&posting["industry"]));
    push("Category", str_of(&posting["occupationalCategory"]["name"]));
    push("Salary", salary(&posting["baseSalary"]));

    Some(JobDetail {
        description,
        criteria,
        applicants: None,
        posted_ago: None,
        fetched_at: Utc::now(),
    })
}

fn to_job(source: Source, item: &Value, query: &str, now: DateTime<Utc>) -> Option<Job> {
    let url = str_of(&item["url"])?;
    let id = str_of(&item["identifier"]["value"])
        .or_else(|| str_of(&item["identifier"]))
        .or_else(|| id_from_url(&url))?;
    Some(Job {
        id,
        source,
        title: str_of(&item["title"])?,
        company: str_of(&item["hiringOrganization"]["name"]).unwrap_or_default(),
        location: place(&item["jobLocation"]),
        url,
        posted: str_of(&item["datePosted"]).map(|d| d.chars().take(10).collect()),
        salary: salary(&item["baseSalary"]),
        found_at: now,
        query: query.to_string(),
        status: Status::New,
        detail: None,
        detail_failed_at: None,
        score: None,
        notified: false,
    })
}

fn json_ld(html: &str) -> Vec<Value> {
    let doc = Html::parse_document(html);
    let sel = super::fetch::selector(r#"script[type="application/ld+json"]"#);
    doc.select(&sel)
        .filter_map(|s| serde_json::from_str::<Value>(&s.text().collect::<String>()).ok())
        .flat_map(|v| match v {
            Value::Array(items) => items,
            v => vec![v],
        })
        .collect()
}

fn is_type(v: &Value, t: &str) -> bool {
    v["@type"].as_str() == Some(t)
}

fn str_of(v: &Value) -> Option<String> {
    v.as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// `jobLocation` is one place or a list of them.
fn place(v: &Value) -> String {
    let places = match v {
        Value::Array(a) => a.iter().collect(),
        v => vec![v],
    };
    let mut names: Vec<String> = vec![];
    for p in places {
        let a = &p["address"];
        if let Some(n) = str_of(&a["addressLocality"]).or_else(|| str_of(&a["addressRegion"])) {
            if !names.contains(&n) {
                names.push(n);
            }
        }
    }
    names.join(", ")
}

/// `CHF 77k–117k / year`.
fn salary(v: &Value) -> Option<String> {
    let value = &v["value"];
    let k = |n: f64| format!("{}k", (n / 1000.0).round());
    let range = match (value["minValue"].as_f64(), value["maxValue"].as_f64()) {
        (Some(lo), Some(hi)) if (hi - lo).abs() >= 1.0 => format!("{}–{}", k(lo), k(hi)),
        (Some(n), _) | (_, Some(n)) => k(n),
        _ => return None,
    };
    let currency = str_of(&v["currency"]).unwrap_or_default();
    let unit = str_of(&value["unitText"])
        .map(|u| format!(" / {}", u.to_lowercase()))
        .unwrap_or_default();
    Some(format!("{currency} {range}{unit}").trim().to_string())
}

/// `…/detail/0a5885a1-…/` → `0a5885a1-…`.
fn id_from_url(url: &str) -> Option<String> {
    let mut parts = url.trim_end_matches('/').rsplit('/');
    parts.next().filter(|s| s.len() >= 32).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_covers_the_whole_country() {
        let p = SearchParams {
            keywords: "rust".into(),
            ..SearchParams::default()
        };
        assert_eq!(
            search_url(Source::JobsCh, &p, 1),
            "https://www.jobs.ch/en/vacancies/?term=rust"
        );
        assert_eq!(
            search_url(Source::Jobup, &p, 2),
            "https://www.jobup.ch/en/jobs/?term=rust&page=2"
        );
    }

    /// JSON-LD saved from real jobs.ch and jobup.ch pages.
    #[test]
    fn parses_saved_pages() {
        let jobs = parse_search(
            Source::JobsCh,
            include_str!("../../tests/fixtures/jobsch_search.html"),
            "q",
        );
        assert_eq!(jobs.len(), 20);
        let j = &jobs[0];
        assert_eq!(j.id, "a20d74c5-df85-4b13-9300-158fe2f456a0");
        assert_eq!(j.company, "VZ VermögensZentrum");
        assert_eq!(j.location, "Zürich");
        assert_eq!(j.posted.as_deref(), Some("2026-10-02"));
        assert!(j
            .url
            .starts_with("https://www.jobs.ch/en/vacancies/detail/"));

        let jobup = parse_search(
            Source::Jobup,
            include_str!("../../tests/fixtures/jobup_search.html"),
            "q",
        );
        assert_eq!(jobup.len(), 20);
        assert!(jobup
            .iter()
            .all(|j| j.source == Source::Jobup && !j.title.is_empty()));

        let d = parse_detail(include_str!("../../tests/fixtures/jobsch_detail.html")).unwrap();
        assert!(d.description.contains("What you bring"));
        assert!(d.description.contains("• Master's degree"));
        assert!(d
            .criteria
            .contains(&("Salary".into(), "CHF 77k–117k / year".into())));
        assert!(d
            .criteria
            .contains(&("Employment type".into(), "Permanent position".into())));
        assert!(parse_detail("<html><body>Search results</body></html>").is_none());
    }

    /// Against the live sites: `cargo test live_ -- --ignored`. Catches the
    /// sites changing their markup, which the saved fixtures cannot.
    #[tokio::test]
    #[ignore]
    async fn live_search_paging_and_detail() {
        let settings = crate::model::Settings {
            delay_ms: 1000,
            ..Default::default()
        };
        for source in [Source::JobsCh, Source::Jobup] {
            let p = SearchParams {
                keywords: "software engineer".into(),
                ..SearchParams::default()
            };
            let get = |url: String| {
                let settings = settings.clone();
                async move { crate::services::fetch::get(&url, &settings).await.unwrap() }
            };
            let page1 = parse_search(source, &get(search_url(source, &p, 1)).await, "q");
            let page2 = parse_search(source, &get(search_url(source, &p, 2)).await, "q");
            assert!(page1.len() >= 10, "{source:?} page 1: {}", page1.len());
            assert!(
                page2.iter().any(|j| !page1.iter().any(|k| k.id == j.id)),
                "{source:?} page 2 is new"
            );
            let far = parse_search(source, &get(search_url(source, &p, 400)).await, "q");
            eprintln!(
                "{source:?}: page1 {} page2 {} page400 {}",
                page1.len(),
                page2.len(),
                far.len()
            );
            let d = parse_detail(&get(page1[0].url.clone()).await).expect("detail parses");
            eprintln!(
                "  {} — {} chars, {:?}",
                page1[0].title,
                d.description.len(),
                d.criteria
            );
        }
    }
}
