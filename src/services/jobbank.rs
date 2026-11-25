//! Job Bank (jobbank.gc.ca), the Government of Canada's job board, for
//! Canada. It lists its own postings and those of partner boards
//! (Talent.com, Indeed, Jobillico …), each with its own page on Job Bank.
//!
//! Plain HTTP works for both pages. robots.txt allows everything, with a
//! crawl delay of 5 seconds.
//!
//!   * search: `/jobsearch/jobsearch?searchstring=…&sort=D&page=N`, all of
//!     Canada, newest first, 25 postings a page, read from their cards
//!     (`article[id^=article-]`). The page has no JSON-LD.
//!   * detail: `/jobsearch/jobposting/{id}`, schema.org microdata (RDFa)
//!     rather than JSON-LD. A partner posting carries its whole description,
//!     as escaped markup, in a hidden `property="description"`; Job Bank's
//!     own postings carry only a one-line summary there and set the
//!     requirements out in sections below it.

use chrono::{DateTime, NaiveDate, Utc};
use scraper::{ElementRef, Html};

use super::text::{html_to_text, markup_to_text, squash};
use crate::model::{Job, JobDetail, SearchParams, Source, Status};

const SITE: &str = "https://www.jobbank.gc.ca";

/// `page` is 1-based, like the site's.
pub fn search_url(p: &SearchParams, page: u32) -> String {
    let mut url = super::fetch::site_url(&format!("{SITE}/jobsearch/jobsearch"));
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("searchstring", p.keywords.trim());
        q.append_pair("sort", "D");
        if page > 1 {
            q.append_pair("page", &page.to_string());
        }
    }
    url.to_string()
}

pub fn detail_url(id: &str) -> String {
    format!("{SITE}/jobsearch/jobposting/{id}")
}

/// Every card on one search page, unfiltered: the caller tells the end of the
/// results from a page that repeats what it has already seen.
pub fn parse_search(html: &str, query: &str) -> Vec<Job> {
    let now = Utc::now();
    let doc = Html::parse_document(html);
    let card = super::fetch::selector("article[id^=\"article-\"]");
    doc.select(&card)
        .filter_map(|c| to_job(c, query, now))
        .collect()
}

/// `None` when the page has no description: a removed posting, or an error
/// page.
pub fn parse_detail(html: &str) -> Option<JobDetail> {
    let doc = Html::parse_document(html);
    let sel = |s: &str| super::fetch::selector(s);

    // The hidden description is text: for a partner posting, the markup of
    // the description itself.
    let summary = doc
        .select(&sel("[property=\"description\"]"))
        .next()
        .map(|e| markup_to_text(&e.text().collect::<String>()))
        .unwrap_or_default();
    let sections: Vec<String> = doc
        .select(&sel(".job-posting-detail-requirements"))
        .map(html_to_text)
        .filter(|t| !t.is_empty())
        .collect();
    let description = if sections.is_empty() {
        summary
    } else {
        sections.join("\n\n")
    };
    if description.is_empty() {
        return None;
    }

    // The facts listed above the description, each named by a label only
    // screen readers see.
    let mut criteria = vec![];
    for li in doc.select(&sel("ul.job-posting-brief > li")) {
        let Some(label) = li.select(&sel(".wb-inv")).next() else {
            continue;
        };
        let label = squash(&label.text().collect::<String>());
        if !matches!(
            label.as_str(),
            "Work location" | "Salary" | "Terms of employment"
        ) {
            continue;
        }
        let all = squash(&li.text().collect::<String>());
        let value = all.strip_prefix(&label).unwrap_or(&all).trim();
        // The salary's currency and unit are hidden copies of what it says.
        let value = squash(&value.replace("$ ", "$"));
        if !value.is_empty() {
            criteria.push((label, value));
        }
    }

    Some(JobDetail {
        description,
        criteria,
        applicants: None,
        posted_ago: None,
        fetched_at: Utc::now(),
    })
}

fn to_job(card: ElementRef, query: &str, now: DateTime<Utc>) -> Option<Job> {
    let sel = |s: &str| super::fetch::selector(s);
    let first = |s: &str| card.select(&sel(s)).next();
    let text = |e: ElementRef| squash(&e.text().collect::<String>());

    let id = card
        .value()
        .attr("id")?
        .strip_prefix("article-")?
        .trim()
        .to_string();
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let title = first("span.noctitle").map(text).filter(|t| !t.is_empty())?;
    // `Toronto (ON)` → `Toronto, ON`, as the job page writes it.
    let location = first("li.location")
        .map(|e| text(e).trim_start_matches("Location").trim().to_string())
        .map(
            |l| match l.strip_suffix(')').and_then(|l| l.rsplit_once(" (")) {
                Some((city, province)) => format!("{city}, {province}"),
                None => l,
            },
        )
        .filter(|l| !l.is_empty())
        .unwrap_or_else(|| "Canada".into());
    let salary = first("li.salary")
        .map(|e| text(e).trim_start_matches("Salary").trim().to_string())
        .filter(|s| !s.is_empty());
    let posted = first("li.date")
        .map(text)
        .and_then(|d| NaiveDate::parse_from_str(&d, "%B %d, %Y").ok())
        .map(|d| d.format("%Y-%m-%d").to_string());
    Some(Job {
        url: detail_url(&id),
        id,
        source: Source::JobBank,
        title,
        company: first("li.business").map(text).unwrap_or_default(),
        location,
        posted,
        salary,
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
    fn keywords_go_in_the_query() {
        let p = SearchParams {
            keywords: " Rust Developer ".into(),
            ..SearchParams::default()
        };
        assert_eq!(
            search_url(&p, 1),
            "https://www.jobbank.gc.ca/jobsearch/jobsearch?searchstring=Rust+Developer&sort=D"
        );
        assert_eq!(
            search_url(&p, 2),
            "https://www.jobbank.gc.ca/jobsearch/jobsearch?searchstring=Rust+Developer&sort=D&page=2"
        );
    }

    /// Pages saved from the live site, cut down to the parts read.
    #[test]
    fn parses_saved_search_page() {
        let jobs = parse_search(
            include_str!("../../tests/fixtures/jobbank_search.html"),
            "q",
        );
        assert_eq!(jobs.len(), 25);
        let j = &jobs[0];
        assert_eq!(j.id, "50799359");
        assert_eq!(j.title, "software developer");
        assert_eq!(j.company, "Astra North Infoteck Inc.");
        assert_eq!(j.location, "Toronto, ON");
        assert_eq!(j.salary.as_deref(), Some("$10,000.00 monthly"));
        assert_eq!(j.posted.as_deref(), Some("2026-11-24"));
        assert_eq!(
            j.url,
            "https://www.jobbank.gc.ca/jobsearch/jobposting/50799359"
        );
        assert!(jobs.iter().all(|j| j.source == Source::JobBank
            && !j.title.contains("New")
            && j.posted.is_some()
            && !j.url.contains("jsessionid")));
        assert!(parse_search("<html></html>", "q").is_empty());
    }

    #[test]
    fn parses_saved_job_pages() {
        // Job Bank's own posting: the requirements sections.
        let d = parse_detail(include_str!("../../tests/fixtures/jobbank_detail.html")).unwrap();
        assert!(d.description.contains("Bachelor's degree"));
        assert!(d.description.contains("• Database"));
        assert!(d
            .criteria
            .contains(&("Work location".into(), "On site".into())));
        assert!(d
            .criteria
            .iter()
            .any(|(k, v)| k == "Terms of employment" && v.starts_with("Permanent employment")));
        assert!(d
            .criteria
            .iter()
            .any(|(k, v)| k == "Salary" && v.starts_with("$50.00")));

        // A partner's posting: its description, unescaped and as text.
        let d = parse_detail(include_str!(
            "../../tests/fixtures/jobbank_partner_detail.html"
        ))
        .unwrap();
        assert!(d.description.contains("Key Responsibilities"));
        assert!(d.description.contains("• Gather, analyze"));
        assert!(!d.description.contains("<p"));

        assert!(parse_detail("<html></html>").is_none());
    }

    /// Search against the live site: `cargo test live_ -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn live_jobbank_search_and_detail() {
        use crate::model::Settings;
        use crate::services::fetch;
        let p = SearchParams {
            keywords: "software developer".into(),
            ..SearchParams::default()
        };
        let s = Settings::default();
        let body = fetch::get(&search_url(&p, 1), &s).await.unwrap();
        let jobs = parse_search(&body, "q");
        assert!(!jobs.is_empty());
        let body = fetch::get(&jobs[0].url, &s).await.unwrap();
        assert!(parse_detail(&body).is_some());
    }
}
