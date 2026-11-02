//! Searching the job boards for new posts, and scoring jobs against the profile.

use super::*;

pub(super) async fn search(args: &Value) -> Result<Value, String> {
    let country = country(args)?;
    let p = search_params(args, country)?;
    let status = licence::current();
    licence::take_search(&status)
        .map_err(|why| format!("{why} Tell the user; do not retry the search."))?;
    let settings = settings();
    let query = format!("{} · {}", p.keywords, country.label());
    let ttl = Duration::from_secs(settings.search_cache_minutes * 60);
    let mut added: Vec<String> = vec![];
    let mut notes: Vec<String> = vec![];

    for &board in &p.sources {
        let name = board.label();
        if let Err(why) = reachable(board, false, &settings) {
            notes.push(format!("{name} skipped: {why}"));
            continue;
        }
        let merge = |found| {
            let mut jobs = store::load_jobs(country);
            let new = sources::merge_new(&mut jobs, found);
            if !new.is_empty() {
                store::save_jobs(country, &jobs);
            }
            new
        };
        // Only what went wrong is worth telling; pages that worked show up
        // as the jobs returned.
        let report = |event| match event {
            SearchEvent::RateLimited(code) => notes.push(format!(
                "{name} is rate-limiting (HTTP {code}): the rest of {name} was skipped. \
                 Wait a few minutes before searching it again."
            )),
            SearchEvent::Failed(e) => notes.push(format!("{name} search failed: {e}")),
            _ => {}
        };
        added.extend(
            sources::search_source(
                board,
                country,
                &p,
                ttl,
                &settings,
                &query,
                || false,
                merge,
                report,
            )
            .await,
        );
    }

    let jobs = store::load_jobs(country);
    let new: Vec<&Job> = added
        .iter()
        .filter_map(|id| jobs.iter().find(|j| j.id == *id))
        .collect();
    let shown: Vec<Value> = new.iter().take(SEARCH_SHOWN).map(|j| list_row(j)).collect();
    let mut v =
        json!({ "country": country.label(), "query": query, "new_jobs": new.len(), "jobs": shown });
    if new.len() > SEARCH_SHOWN {
        v["more"] = json!(format!(
            "{} more new jobs are kept; list_jobs with status New lists them",
            new.len() - SEARCH_SHOWN
        ));
    }
    if !notes.is_empty() {
        v["notes"] = json!(notes);
    }
    Ok(v)
}

pub(super) fn ids(args: &Value) -> Result<Vec<String>, String> {
    let list: Vec<String> = args["ids"]
        .as_array()
        .ok_or("ids is required")?
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();
    if list.is_empty() {
        return Err("ids is empty".into());
    }
    Ok(list)
}

pub(super) async fn score_jobs(args: &Value) -> Result<Value, String> {
    let country = country(args)?;
    let ids = ids(args)?;
    if ids.len() > SCORE_MAX {
        return Err(format!(
            "at most {SCORE_MAX} jobs per call; {} were asked for",
            ids.len()
        ));
    }
    let profile = store::load_profile(country);
    if profile.trim().is_empty() {
        return Err(format!(
            "no profile for {}: set one with set_profile, or with Profile in the window",
            country.label()
        ));
    }
    let settings = settings();
    let mut results = vec![];
    let mut stopped = None;
    for id in &ids {
        if let Some(why) = &stopped {
            results.push(json!({ "id": id, "error": format!("not scored: {why}") }));
            continue;
        }
        match score_one(country, id, &profile, &settings).await {
            Ok(job) => {
                let Some(s) = job.score.as_ref() else {
                    results.push(json!({ "id": id, "error": "not scored" }));
                    continue;
                };
                results.push(json!({
                    "id": id,
                    "title": job.title,
                    "company": job.company,
                    "score": s.score,
                    "verdict": s.verdict,
                    "strengths": s.strengths,
                    "gaps": s.gaps,
                }));
            }
            Err(e) => {
                // A missing or blocked page is this job's problem; anything
                // else (the model, its key, a rate limit) would repeat for
                // every job, so the rest are not tried. The window's rule.
                if !e.starts_with("HTTP")
                    && !e.contains("no description")
                    && !e.starts_with("no job")
                {
                    stopped = Some(e.clone());
                }
                results.push(json!({ "id": id, "error": e }));
            }
        }
    }
    Ok(json!({ "country": country.label(), "results": results }))
}

pub(super) async fn score_one(
    country: Country,
    id: &str,
    profile: &str,
    settings: &Settings,
) -> Result<Job, String> {
    let job = find(country, id)?;
    match freshness(&job, settings, Utc::now()) {
        Freshness::Fresh => {}
        Freshness::Failed => return Err("no description: the job page failed recently".into()),
        Freshness::Missing | Freshness::Stale => fetch_detail(country, id, settings).await?,
    }
    let job = find(country, id)?;
    let score = llm::score(settings, profile, &job).await?;
    update(country, id, |j| j.score = Some(score))?;
    find(country, id)
}
