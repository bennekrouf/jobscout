//! LinkedIn's public guest endpoints — the ones the logged-out jobs page uses.
//!
//! No account is involved: nothing here can get a profile restricted, and the
//! only cost of being too eager is a 429 that clears by itself.
//!
//!   * search: `/jobs-guest/jobs/api/seeMoreJobPostings/search` returns a
//!     fragment of `<li>` cards, 10 per page, paged with `start`.
//!   * detail: `/jobs-guest/jobs/api/jobPosting/{id}` returns the top card,
//!     the description and the criteria list.

use chrono::Utc;
use scraper::{ElementRef, Html, Selector};

use super::text::{html_to_text, squash};

use crate::model::{Contract, Country, Job, JobDetail, SearchParams, Source, Status};

const BASE: &str = "https://www.linkedin.com/jobs-guest/jobs/api";

/// LinkedIn covers every country, so the loaded one is the location.
pub fn search_url(p: &SearchParams, country: Country, start: u32) -> String {
    let mut url = super::fetch::site_url(&format!("{BASE}/seeMoreJobPostings/search"));
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("keywords", p.keywords.trim());
        q.append_pair("location", country.label());
        if let Some(code) = p.posted.code() {
            q.append_pair("f_TPR", code);
        }
        let workplace: Vec<&str> = [(p.onsite, "1"), (p.remote, "2"), (p.hybrid, "3")]
            .into_iter()
            .filter_map(|(on, code)| on.then_some(code))
            .collect();
        if !workplace.is_empty() {
            q.append_pair("f_WT", &workplace.join(","));
        }
        if !p.experience.is_empty() {
            let levels: Vec<String> = p.experience.iter().map(u8::to_string).collect();
            q.append_pair("f_E", &levels.join(","));
        }
        // Job types: F full-time, which is how LinkedIn lists a CDI, and
        // C contract, which is where freelance missions go.
        if !p.contract.is_empty() {
            let types: Vec<&str> = p
                .contract
                .iter()
                .map(|c| match c {
                    Contract::Permanent => "F",
                    Contract::Freelance => "C",
                })
                .collect();
            q.append_pair("f_JT", &types.join(","));
        }
        q.append_pair("start", &start.to_string());
    }
    url.to_string()
}

pub fn detail_url(id: &str) -> String {
    format!("{BASE}/jobPosting/{id}")
}

pub fn view_url(id: &str) -> String {
    format!("https://www.linkedin.com/jobs/view/{id}/")
}

/// Every job card on one search page. An empty result means the end of the
/// results — or a page that is not a results page at all, which ends a search
/// just the same.
pub fn parse_search(html: &str, query: &str) -> Vec<Job> {
    let doc = Html::parse_document(html);
    let card = sel(".base-search-card");
    let link = sel("a.base-card__full-link");
    let title = sel(".base-search-card__title");
    let company = sel(".base-search-card__subtitle");
    let location = sel(".job-search-card__location");
    let time = sel("time");
    let salary = sel(".job-search-card__salary-info");

    let now = Utc::now();
    doc.select(&card)
        .filter_map(|c| {
            let href = c
                .select(&link)
                .next()
                .and_then(|a| a.attr("href"))
                .or_else(|| c.attr("href"))
                .unwrap_or_default();
            let id = c
                .attr("data-entity-urn")
                .and_then(|urn| urn.rsplit(':').next())
                .map(str::to_string)
                .or_else(|| id_from_url(href))?;
            let title = first_text(c, &title)?;
            Some(Job {
                url: view_url(&id),
                id,
                source: Source::LinkedIn,
                title,
                company: first_text(c, &company).unwrap_or_default(),
                location: first_text(c, &location).unwrap_or_default(),
                posted: c
                    .select(&time)
                    .next()
                    .and_then(|t| t.attr("datetime"))
                    .map(str::to_string),
                salary: first_text(c, &salary),
                found_at: now,
                query: query.to_string(),
                status: Status::New,
                detail: None,
                detail_failed_at: None,
                score: None,
                notified: false,
            })
        })
        .collect()
}

/// The description and criteria of one posting; `None` when the page has no
/// description, which is what a block page or a removed posting looks like.
pub fn parse_detail(html: &str) -> Option<JobDetail> {
    let doc = Html::parse_document(html);
    let description = doc
        .select(&sel(".show-more-less-html__markup"))
        .next()
        .map(html_to_text)
        .filter(|d| !d.is_empty())?;

    let item = sel(".description__job-criteria-item");
    let head = sel(".description__job-criteria-subheader");
    let value = sel(".description__job-criteria-text");
    let criteria = doc
        .select(&item)
        .filter_map(|li| Some((first_text(li, &head)?, first_text(li, &value)?)))
        .collect();

    let root = doc.root_element();
    Some(JobDetail {
        description,
        criteria,
        applicants: first_text(root, &sel(".num-applicants__caption")),
        posted_ago: first_text(root, &sel(".posted-time-ago__text")),
        fetched_at: Utc::now(),
    })
}

fn sel(s: &str) -> Selector {
    super::fetch::selector(s)
}

fn first_text(el: ElementRef, s: &Selector) -> Option<String> {
    el.select(s)
        .next()
        .map(|e| squash(&e.text().collect::<String>()))
        .filter(|t| !t.is_empty())
}

/// `…-at-keyrock-4451499757?position=1…` → `4451499757`.
fn id_from_url(href: &str) -> Option<String> {
    let path = href.split('?').next()?.trim_end_matches('/');
    let tail = path.rsplit(['-', '/']).next()?;
    (!tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit())).then(|| tail.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_from_view_url() {
        assert_eq!(
            id_from_url(
                "https://ch.linkedin.com/jobs/view/senior-rust-at-keyrock-4451499757?position=1"
            )
            .as_deref(),
            Some("4451499757")
        );
        assert_eq!(id_from_url("https://example.com/jobs/").as_deref(), None);
    }

    #[test]
    fn search_url_carries_filters() {
        let p = SearchParams {
            keywords: "rust developer".into(),
            remote: true,
            hybrid: true,
            experience: vec![3, 4],
            contract: vec![Contract::Permanent, Contract::Freelance],
            ..SearchParams::default()
        };
        let url = search_url(&p, Country::Singapore, 20);
        assert!(url.contains("f_JT=F%2CC"));
        assert!(url.contains("keywords=rust+developer"));
        assert!(url.contains("location=Singapore"));
        assert!(url.contains("f_WT=2%2C3"));
        assert!(url.contains("f_E=3%2C4"));
        assert!(url.contains("f_TPR=r604800"));
        assert!(url.ends_with("start=20"));
    }

    #[test]
    fn parses_card_and_detail() {
        let card = r#"<li><div class="base-card base-search-card" data-entity-urn="urn:li:jobPosting:42">
            <a class="base-card__full-link" href="https://ch.linkedin.com/jobs/view/x-42?pos=1"></a>
            <h3 class="base-search-card__title"> Rust   Engineer </h3>
            <h4 class="base-search-card__subtitle"><a>Acme</a></h4>
            <span class="job-search-card__location">Zurich</span>
            <time datetime="2026-10-01"></time></div></li>"#;
        let jobs = parse_search(card, "q");
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].id, "42");
        assert_eq!(jobs[0].title, "Rust Engineer");
        assert_eq!(jobs[0].company, "Acme");
        assert_eq!(jobs[0].posted.as_deref(), Some("2026-10-01"));

        let detail = r#"<div class="show-more-less-html__markup"><p>About us</p><ul><li>Rust</li><li>Tokio</li></ul></div>
            <li class="description__job-criteria-item"><h3 class="description__job-criteria-subheader">Seniority level</h3>
            <span class="description__job-criteria-text">Mid-Senior</span></li>"#;
        let d = parse_detail(detail).unwrap();
        assert_eq!(d.description, "About us\n\n• Rust\n• Tokio");
        assert_eq!(
            d.criteria,
            vec![("Seniority level".into(), "Mid-Senior".into())]
        );
        assert!(parse_detail("<html>blocked</html>").is_none());
    }

    /// Real pages saved from the guest endpoints — fails when LinkedIn changes its markup.
    #[test]
    fn parses_saved_linkedin_pages() {
        let search = include_str!("../../tests/fixtures/search.html");
        let jobs = parse_search(search, "rust developer · Switzerland");
        assert_eq!(jobs.len(), 10);
        assert!(jobs
            .iter()
            .all(|j| !j.id.is_empty() && !j.title.is_empty() && !j.company.is_empty()));
        assert_eq!(jobs[0].id, "4451499757");

        let detail = parse_detail(include_str!("../../tests/fixtures/job.html")).unwrap();
        assert!(detail.description.len() > 200);
        assert_eq!(detail.criteria.len(), 3);
        assert!(detail.applicants.is_some());
        eprintln!(
            "{:#?}\n{}",
            jobs[0],
            &detail.description[..400.min(detail.description.len())]
        );
    }
}
