//! The jobs kept for a country: the overview, listing, reading one, its status, a CV for it,
//! and the CSV export.

use super::*;

pub(super) fn overview() -> Value {
    let settings = store::load_settings();
    let countries: Vec<Value> = Country::ALL
        .iter()
        .map(|&c| {
            let jobs = store::load_jobs(c);
            let mut by_status = Map::new();
            for s in Status::ALL {
                let n = jobs.iter().filter(|j| j.status == s).count();
                by_status.insert(s.label().into(), json!(n));
            }
            let sources: Vec<&str> = c.sources().iter().map(|s| s.label()).collect();
            json!({
                "code": c.code(),
                "name": c.label(),
                "boards": sources,
                "jobs": jobs.len(),
                "by_status": by_status,
                "scored": jobs.iter().filter(|j| j.score.is_some()).count(),
                "watches": store::load_watches(c).len(),
                "profile_chars": store::load_profile(c).trim().chars().count(),
            })
        })
        .collect();
    let status = licence::current();
    let limit = status.monthly_searches();
    json!({
        "open_in_window": settings.country.code(),
        "countries": countries,
        "scoring_model": settings.scoring_model(),
        "licence": status.describe(),
        "searches_this_month": { "used": licence::searches_used(), "limit": limit },
    })
}

/// A job without its description, as lists show it.
/// A job in a list: its row without what only get_job needs.
pub(super) fn list_row(j: &Job) -> Value {
    let mut v = row(j);
    if let Some(o) = v.as_object_mut() {
        o.remove("has_description");
    }
    v
}

pub(super) fn row(j: &Job) -> Value {
    let mut v = json!({
        "id": j.id,
        "source": j.source.label(),
        "title": j.title,
        "company": j.company,
        "location": j.location,
        "status": j.status.label(),
        "url": j.url,
        "found": j.found_at.format("%Y-%m-%d").to_string(),
        "has_description": j.detail.is_some(),
    });
    // `json!({ .. })` always makes an object.
    #[allow(clippy::expect_used)]
    let o = v.as_object_mut().expect("an object");
    if let Some(p) = &j.posted {
        o.insert("posted".into(), json!(p));
    }
    if let Some(s) = &j.salary {
        o.insert("salary".into(), json!(s));
    }
    if let Some(s) = &j.score {
        o.insert("score".into(), json!(s.score));
        o.insert("verdict".into(), json!(s.verdict));
    }
    v
}

/// The jobs the list filters in `args` keep, in the order asked for.
pub(super) fn filtered<'a>(
    args: &Value,
    country: Country,
    jobs: &'a [Job],
) -> Result<Vec<&'a Job>, String> {
    let wanted = args["status"].as_str().map(status).transpose()?;
    let board = args["source"]
        .as_str()
        .map(|v| source(v, country))
        .transpose()?;
    let min_score = args["min_score"].as_u64();
    let unscored = args["unscored"].as_bool().unwrap_or(false);
    let needle = args["text"]
        .as_str()
        .map(|t| t.trim().to_lowercase())
        .filter(|t| !t.is_empty());

    let mut out: Vec<&Job> = jobs
        .iter()
        .filter(|j| match wanted {
            Some(s) => j.status == s,
            None => j.status != Status::Hidden,
        })
        .filter(|j| board.is_none_or(|b| j.source == b))
        .filter(|j| min_score.is_none_or(|m| j.score.as_ref().is_some_and(|s| s.score as u64 >= m)))
        .filter(|j| !unscored || j.score.is_none())
        .filter(|j| {
            needle.as_ref().is_none_or(|n| {
                [&j.title, &j.company, &j.location]
                    .iter()
                    .any(|f| f.to_lowercase().contains(n.as_str()))
            })
        })
        .collect();
    match args["sort"].as_str().unwrap_or("newest") {
        // The file keeps the newest first already.
        "newest" => {}
        "score" => out.sort_by_key(|j| std::cmp::Reverse(j.score.as_ref().map(|s| s.score))),
        other => return Err(format!("sort is newest or score, not {other}")),
    }
    Ok(out)
}

pub(super) fn list_jobs(args: &Value) -> Result<Value, String> {
    let country = country(args)?;
    let jobs = store::load_jobs(country);
    let all = filtered(args, country, &jobs)?;
    let limit = args["limit"]
        .as_u64()
        .map_or(LIST_DEFAULT, |n| (n as usize).clamp(1, LIST_MAX));
    let offset = args["offset"].as_u64().unwrap_or(0) as usize;
    let page: Vec<Value> = all
        .iter()
        .skip(offset)
        .take(limit)
        .map(|j| list_row(j))
        .collect();
    Ok(json!({
        "country": country.label(),
        "matching": all.len(),
        "returned": page.len(),
        "offset": offset,
        "jobs": page,
    }))
}

/// The job `args` names, its description fetched first when it is missing,
/// stale or `refresh` asks for it. A failed fetch still returns what is
/// kept, with the reason.
pub(super) async fn fresh_job(args: &Value) -> Result<(Job, Option<String>), String> {
    let country = country(args)?;
    let id = text(args, "id")?;
    let settings = settings();
    let job = find(country, &id)?;
    let refresh = args["refresh"].as_bool().unwrap_or(false);
    let mut note = None;
    let wanted = refresh
        || matches!(
            freshness(&job, &settings, Utc::now()),
            Freshness::Missing | Freshness::Stale
        );
    if wanted {
        if let Err(e) = fetch_detail(country, &id, &settings).await {
            note = Some(format!("description not fetched: {e}"));
        }
    }
    Ok((find(country, &id)?, note))
}

pub(super) async fn get_job(args: &Value) -> Result<Value, String> {
    let (job, note) = fresh_job(args).await?;
    let mut v = row(&job);
    // `json!({ .. })` always makes an object.
    #[allow(clippy::expect_used)]
    let o = v.as_object_mut().expect("an object");
    if let Some(d) = &job.detail {
        let description: String = d.description.chars().take(DESCRIPTION_CAP).collect();
        o.insert("description".into(), json!(description));
        if d.description.chars().count() > DESCRIPTION_CAP {
            o.insert("description_truncated".into(), json!(true));
        }
        let criteria: Map<String, Value> = d
            .criteria
            .iter()
            .map(|(k, v)| (k.clone(), json!(v)))
            .collect();
        o.insert("criteria".into(), Value::Object(criteria));
        if let Some(a) = &d.applicants {
            o.insert("applicants".into(), json!(a));
        }
        o.insert(
            "description_fetched".into(),
            json!(d.fetched_at.to_rfc3339()),
        );
    }
    if let Some(s) = &job.score {
        o.insert("strengths".into(), json!(s.strengths));
        o.insert("gaps".into(), json!(s.gaps));
        o.insert("scored".into(), json!(s.scored_at.to_rfc3339()));
    }
    if let Some(n) = note {
        o.insert("note".into(), json!(n));
    }
    Ok(v)
}

/// What cvenom's CV tools need to tailor a CV to a job. JobScout cannot call
/// another MCP server; the client does, so this hands it the arguments and
/// names the tool, rather than leaving the model to write a CV itself.
pub(super) async fn cv_for_job(args: &Value) -> Result<Value, String> {
    let (job, note) = fresh_job(args).await?;
    let mut cvenom = json!({ "job_url": job.url, "job_title": job.title, "company": job.company });
    if let Some(d) = &job.detail {
        let description: String = d.description.chars().take(DESCRIPTION_CAP).collect();
        cvenom["job_description"] = json!(description);
    }
    let mut v = json!({
        "job": { "id": job.id, "title": job.title, "company": job.company, "location": job.location },
        "cvenom_arguments": cvenom,
        "next": "Call cvenom-list-profiles for the profile name (ask the user if there are \
                 several), then cvenom-optimize-and-generate with profile and all the \
                 cvenom_arguments above (job_title and company name the PDF); set lang to fr or de when the posting is in French or \
                 German. Give the user the download_url it returns, unchanged. For the tailored \
                 text without a PDF, use cvenom-optimize-cv-for-job with the same arguments; for \
                 a cover letter, cvenom-write-cover-letter with job_description and lang.",
    });
    if let Some(n) = note {
        v["note"] = json!(n);
    }
    Ok(v)
}

/// Settings' Scoring model buttons: an api0 region, or ollama.
pub(super) async fn set_scoring_model(args: &Value) -> Result<Value, String> {
    let model = text(args, "model")?.to_lowercase();
    let mut s = store::load_settings();
    if model == "ollama" {
        s.use_ollama();
    } else {
        let region = llm::REGIONS
            .iter()
            .find(|r| r.id == model || r.label.eq_ignore_ascii_case(&model))
            .ok_or_else(|| format!("no scoring model {model}; use europe, china, usa or ollama"))?;
        desktop_kit::region::ensure_open(region).await?;
        s.use_region(region.id);
    }
    store::save_settings(&s);
    let mut v = json!({ "scoring_model": s.scoring_model() });
    if s.is_ollama() && s.llm_model.trim().is_empty() {
        v["note"] =
            json!("ollama has no model set: choose one in JobScout's Settings before scoring");
    } else if s.region().is_some() && llm::api0_key().is_none() {
        v["note"] = json!("api0 needs a licence: get free AI credits or paste the licence number in JobScout's Pro window");
    }
    Ok(v)
}

pub(super) fn find(country: Country, id: &str) -> Result<Job, String> {
    store::load_jobs(country)
        .into_iter()
        .find(|j| j.id == id)
        .ok_or_else(|| format!("no job {id} in {}", country.label()))
}

/// Applies `f` to the job on disk as it is now, not as it was when a slow
/// request started, so a change the window made meanwhile is kept.
pub(super) fn update(country: Country, id: &str, f: impl FnOnce(&mut Job)) -> Result<(), String> {
    let mut jobs = store::load_jobs(country);
    let job = jobs
        .iter_mut()
        .find(|j| j.id == id)
        .ok_or("job no longer listed")?;
    f(job);
    store::save_jobs(country, &jobs);
    Ok(())
}

pub(super) async fn fetch_detail(
    country: Country,
    id: &str,
    settings: &Settings,
) -> Result<(), String> {
    let job = find(country, id)?;
    reachable(job.source, true, settings)?;
    sources::fetch_job_detail(&job, settings, |detail| {
        update(country, id, |j| sources::record_detail(j, detail))
    })
    .await
}

pub(super) fn set_status(args: &Value) -> Result<Value, String> {
    let country = country(args)?;
    let ids = ids(args)?;
    let to = status(&text(args, "status")?)?;
    let mut jobs = store::load_jobs(country);
    let mut missing = vec![];
    let mut changed = 0;
    for id in &ids {
        match jobs.iter_mut().find(|j| j.id == *id) {
            Some(j) => {
                j.status = to;
                changed += 1;
            }
            None => missing.push(id.clone()),
        }
    }
    if changed > 0 {
        store::save_jobs(country, &jobs);
    }
    let mut v = json!({ "country": country.label(), "status": to.label(), "changed": changed });
    if !missing.is_empty() {
        v["not_found"] = json!(missing);
    }
    Ok(v)
}

pub(super) fn export_csv(args: &Value) -> Result<Value, String> {
    let country = country(args)?;
    let path = std::path::PathBuf::from(text(args, "path")?);
    if !path.is_absolute() {
        return Err("path must be absolute".into());
    }
    let jobs = store::load_jobs(country);
    let rows = filtered(args, country, &jobs)?;
    let failed = |e: std::io::Error| format!("{}: {} ({e})", path.display(), io_reason(&e));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(failed)?;
    }
    std::fs::write(&path, store::to_csv(&rows)).map_err(failed)?;
    Ok(
        json!({ "country": country.label(), "jobs": rows.len(), "path": path.display().to_string() }),
    )
}

/// An I/O failure in English. The OS text that follows it in brackets comes
/// in the system's display language, which on a French Windows is French.
pub(super) fn io_reason(e: &std::io::Error) -> &'static str {
    use std::io::ErrorKind::*;
    match e.kind() {
        NotFound => "not found",
        PermissionDenied => "permission denied",
        AlreadyExists => "already exists",
        IsADirectory => "is a folder",
        ReadOnlyFilesystem => "read-only drive",
        StorageFull => "the disk is full",
        _ => "could not write",
    }
}
