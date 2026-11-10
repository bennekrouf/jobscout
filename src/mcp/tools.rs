//! The tools `jobscout mcp` offers, as `tools/list` gives them.

use super::*;

pub(super) fn tools() -> Value {
    // What country means is said once, in the instructions, not in every tool.
    let country = json!({ "type": "string" });
    let ids = json!({
        "type": "array", "items": { "type": "string" },
        "description": "Job ids, from list_jobs or search.",
    });
    let statuses: Vec<&str> = Status::ALL.iter().map(|s| s.label()).collect();
    let mut scoring_choices: Vec<&str> = llm::REGIONS.iter().map(|r| r.id).collect();
    scoring_choices.push("ollama");
    let search = json!({
        "country": country,
        "keywords": { "type": "string", "description": "What to search for, e.g. \"data engineer\"." },
        "posted": {
            "type": "string", "enum": ["any", "day", "week", "month"],
            "description": "How recent the posts are. Default week.",
        },
        "sources": {
            "type": "array", "items": { "type": "string" },
            "description": "Boards to search, by name as overview lists them. Default: all of the country's.",
        },
        "pages": {
            "type": "integer", "minimum": 1, "maximum": 20,
            "description": "Pages per board: 10 posts a page on LinkedIn, about 20 elsewhere. Default 3.",
        },
        "onsite": { "type": "boolean", "description": "LinkedIn only: on-site posts." },
        "remote": { "type": "boolean", "description": "LinkedIn only: remote posts." },
        "hybrid": { "type": "boolean", "description": "LinkedIn only: hybrid posts." },
        "experience": {
            "type": "array", "items": { "type": "integer", "minimum": 1, "maximum": 6 },
            "description": "LinkedIn only: 1 Internship, 2 Entry, 3 Associate, 4 Mid-Senior, 5 Director, 6 Executive.",
        },
        "contract": {
            "type": "array", "items": { "type": "string", "enum": ["permanent", "freelance"] },
            "description": "Only these contract types: permanent (CDI) and/or freelance. LinkedIn, France Travail, Arbeitsagentur and Reed only; the other boards ignore it. Default: any.",
        },
    });
    let filters = json!({
        "country": country,
        "status": {
            "type": "string", "enum": statuses,
            "description": "Only jobs with this status. Default: every status but Hidden.",
        },
        "source": { "type": "string", "description": "Only jobs from this board." },
        "min_score": { "type": "integer", "minimum": 0, "maximum": 100, "description": "Only jobs scored at least this." },
        "unscored": { "type": "boolean", "description": "Only jobs not scored yet." },
        "text": { "type": "string", "description": "Only jobs whose title, company or location contains this." },
        "sort": {
            "type": "string", "enum": ["newest", "score"],
            "description": "newest (found most recently first, the default) or score (highest first, unscored last).",
        },
    });
    let with = |base: &Value, extra: Value| {
        let mut props = base.as_object().cloned().unwrap_or_default();
        props.extend(extra.as_object().cloned().unwrap_or_default());
        Value::Object(props)
    };
    let watch_settings = json!({
        "every_hours": {
            "type": "integer", "enum": Watch::INTERVALS,
            "description": "How often it runs. Default 6.",
        },
        "min_score": {
            "type": "integer", "minimum": 0, "maximum": 100,
            "description": "Notify about new jobs scored at least this. Default 75.",
        },
        "email": {
            "type": "boolean",
            "description": "Also email what each run finds (the matches, or every new job \
                            without a profile) to the licence's email. Needs a licence. Default false.",
        },
    });

    json!([
        {
            "name": "overview",
            "description": "Every country JobScout covers: its boards, how many jobs it keeps by \
                            status, how many are scored, its watches and whether a profile is set. \
                            Also the country open in the window, the scoring model and the licence.",
            "inputSchema": { "type": "object", "properties": {} },
        },
        {
            "name": "list_jobs",
            "description": "The jobs kept for a country, filtered and sorted, with their score and \
                            verdict when scored. Descriptions are left out: get_job has them.",
            "inputSchema": {
                "type": "object",
                "properties": with(&filters, json!({
                    "limit": { "type": "integer", "minimum": 1, "maximum": LIST_MAX, "description": format!("Default {LIST_DEFAULT}.") },
                    "offset": { "type": "integer", "minimum": 0, "description": "Skip this many, to page through." },
                })),
            },
        },
        {
            "name": "get_job",
            "description": "One job in full: description, criteria, and the score's strengths and \
                            gaps. Fetches the description from the board first when it is missing \
                            or older than the freshness set in Settings.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "country": country,
                    "id": { "type": "string" },
                    "refresh": { "type": "boolean", "description": "Fetch the description again even if it is fresh." },
                },
                "required": ["id"],
            },
        },
        {
            "name": "cv_for_job",
            "description": "Call this when the user wants a CV adapted, tailored or generated for \
                            a job. Returns the job's URL and description, fetched first when \
                            needed, as the arguments cvenom's CV tools take: pass them to \
                            cvenom-optimize-and-generate with the profile from \
                            cvenom-list-profiles, and give the user its download_url unchanged. \
                            For the tailored text only, pass them to cvenom-optimize-cv-for-job; \
                            a cover letter is cvenom-write-cover-letter, a fit check \
                            cvenom-analyze-job-fit. Does not write the CV itself, and neither should you.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "country": country,
                    "id": { "type": "string" },
                },
                "required": ["id"],
            },
        },
        {
            "name": "search",
            "description": "Searches the country's boards, adds the posts not seen before to its \
                            jobs (a post found on two boards is kept once), and returns those new \
                            ones (the first 20; list_jobs has the rest). Takes a few seconds per page. Each call counts as one of the searches \
                            the licence gives a month; overview says how many are left.",
            "inputSchema": { "type": "object", "properties": search, "required": ["keywords"] },
        },
        {
            "name": "score_jobs",
            "description": format!(
                "Scores jobs against the country's profile, 0-100 with a verdict, strengths and \
                 gaps, fetching each description first when needed. At most {SCORE_MAX} per call. \
                 A job already scored is scored again. With a cloud model each score is a paid \
                 call: score the jobs the user asks about rather than everything."
            ),
            "inputSchema": {
                "type": "object",
                "properties": { "country": country, "ids": ids },
                "required": ["ids"],
            },
        },
        {
            "name": "set_status",
            "description": "Marks jobs New, Saved, Applied or Hidden.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "country": country,
                    "ids": ids,
                    "status": { "type": "string", "enum": statuses },
                },
                "required": ["ids", "status"],
            },
        },
        {
            "name": "get_profile",
            "description": "The country's profile: the CV and wishes every score compares a job \
                            with. This is JobScout's scoring text, not the user's CV: for the CV \
                            itself, use cvenom-list-profiles and cvenom-get-cv.",
            "inputSchema": { "type": "object", "properties": { "country": country } },
        },
        {
            "name": "set_profile",
            "description": "Replaces the country's whole profile. Show the user the new text and \
                            get their agreement first. To build it from the user's CV, read the \
                            CV with cvenom-get-cv rather than writing one.",
            "inputSchema": {
                "type": "object",
                "properties": { "country": country, "text": { "type": "string" } },
                "required": ["text"],
            },
        },
        {
            "name": "set_scoring_model",
            "description": "Sets where score_jobs runs, for every country: api0 in europe, \
                            china or usa (paid from the licence's AI credits, which overview \
                            shows), or ollama on this computer (free; its model must be set in \
                            JobScout's Settings). Same as the buttons in Settings → Scoring model: \
                            a region api0 has not opened yet is refused, and the answer says \
                            which are open.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "model": { "type": "string", "enum": scoring_choices },
                },
                "required": ["model"],
            },
        },
        {
            "name": "list_watches",
            "description": "The country's watches: saved searches the window runs on a schedule, \
                            only while it is open, scoring what is new and notifying about good matches.",
            "inputSchema": { "type": "object", "properties": { "country": country } },
        },
        {
            "name": "add_watch",
            "description": "Saves a search as a watch. A scheduled run that finds new jobs counts \
                            as one of the searches the licence gives a month.",
            "inputSchema": {
                "type": "object",
                "properties": with(&search, watch_settings.clone()),
                "required": ["keywords"],
            },
        },
        {
            "name": "update_watch",
            "description": "Changes how often a watch runs, its notification threshold, or turns it on or off.",
            "inputSchema": {
                "type": "object",
                "properties": with(&watch_settings, json!({
                    "country": country,
                    "id": { "type": "string" },
                    "enabled": { "type": "boolean" },
                })),
                "required": ["id"],
            },
        },
        {
            "name": "delete_watch",
            "description": "Deletes a watch. Ask the user first.",
            "inputSchema": {
                "type": "object",
                "properties": { "country": country, "id": { "type": "string" } },
                "required": ["id"],
            },
        },
        {
            "name": "export_csv",
            "description": "Writes the jobs list_jobs would return, without its limit, to a CSV file \
                            for a spreadsheet.",
            "inputSchema": {
                "type": "object",
                "properties": with(&filters, json!({
                    "path": { "type": "string", "description": "Absolute path of the .csv file to write." },
                })),
                "required": ["path"],
            },
        },
    ])
}
