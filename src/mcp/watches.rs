//! Saved searches: listing, adding and changing them.

use super::*;

pub(super) fn watch_json(w: &Watch) -> Value {
    let p = &w.search;
    let boards: Vec<&str> = p.sources.iter().map(|s| s.label()).collect();
    json!({
        "id": w.id,
        "keywords": w.name(),
        "boards": boards,
        "posted": p.posted.label(),
        "pages": p.pages,
        "every_hours": w.every_hours,
        "min_score": w.min_score,
        "enabled": w.enabled,
        "email": w.email,
        "last_run": w.last_run.map(|t| t.to_rfc3339()),
        "last_result": w.last_result,
    })
}

pub(super) fn watch_settings(args: &Value, w: &mut Watch) -> Result<(), String> {
    if let Some(h) = args["every_hours"].as_u64() {
        let h = u32::try_from(h).unwrap_or(0);
        if !Watch::INTERVALS.contains(&h) {
            return Err(format!("every_hours is one of {:?}", Watch::INTERVALS));
        }
        w.every_hours = h;
    }
    if let Some(m) = args["min_score"].as_u64() {
        w.min_score = m.min(100) as u8;
    }
    if let Some(e) = args["email"].as_bool() {
        if e && licence::saved_key().is_none() {
            return Err(
                "email needs a licence: the user pastes one in JobScout's Pro window".into(),
            );
        }
        w.email = e;
    }
    Ok(())
}

pub(super) fn add_watch(args: &Value) -> Result<Value, String> {
    let country = country(args)?;
    let search = search_params(args, country)?;
    let mut w = Watch::new(search);
    watch_settings(args, &mut w)?;
    let mut watches = store::load_watches(country);
    watches.push(w.clone());
    store::save_watches(country, &watches);
    Ok(json!({
        "country": country.label(),
        "watch": watch_json(&w),
        "note": "It runs while the JobScout window is open, within a minute of being due.",
    }))
}

pub(super) fn update_watch(args: &Value) -> Result<Value, String> {
    let country = country(args)?;
    let id = text(args, "id")?;
    let mut watches = store::load_watches(country);
    let w = watches
        .iter_mut()
        .find(|w| w.id == id)
        .ok_or_else(|| format!("no watch {id} in {}", country.label()))?;
    watch_settings(args, w)?;
    if let Some(on) = args["enabled"].as_bool() {
        w.enabled = on;
    }
    let v = watch_json(w);
    store::save_watches(country, &watches);
    Ok(json!({ "country": country.label(), "watch": v }))
}
