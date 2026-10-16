# JobScout

A desktop app (Rust + Dioxus) that collects job posts from LinkedIn, jobs.ch
and jobup.ch and ranks them against your profile.

- **Search** LinkedIn's public guest endpoints. No login is used, so no account is at risk. You can filter by keywords, location, date posted, workplace type and experience level, and choose how many pages to fetch.
- **Keep** every job found in `~/Library/Application Support/JobScout/jobs.json`, deduplicated by job id. Each job has a status (New / Saved / Applied / Hidden).
- **Read** the full description and criteria inside the app. A job's description is fetched when you first open it.
- **Score** jobs against your profile with any OpenAI-compatible model (DeepSeek by default, or a local ollama). Each score comes with a one-line verdict, strengths and gaps.
- **Export** the visible list to CSV.

## Run

```sh
cargo run            # or: dx serve
cargo test           # parsers, including real saved LinkedIn pages
```

For scoring, export the key named in Settings (default `DEEPSEEK_API_KEY`).
The key is read from the environment and never written to disk.

## Sources

| Source | Read from | Search filters |
|---|---|---|
| LinkedIn | Public guest endpoints (`/jobs-guest/...`), no login | All filters |
| jobs.ch, jobup.ch | Public search and job pages, via their schema.org `JobPosting` JSON-LD. Not `/api/`, which their robots.txt disallows | Keywords, location, date posted (filtered in the app). Workplace and experience don't apply |

Both JobCloud sites (jobs.ch and jobup.ch) share posting ids, so a job listed on
both appears once. Across LinkedIn and JobCloud, a job with the same title and
company is listed once: the copy found first is kept. A location of
"Switzerland" (or empty) means the whole country on jobs.ch and jobup.ch.

`cargo test live_ -- --ignored` runs the JobCloud parser against the live
sites, to catch markup changes the saved fixtures can't.

The **Copy** button in a job's pane puts the posting on the clipboard as plain
text: title, company, location, link, criteria and description. Text in the
pane can also be selected and copied with Cmd+C.

## Fetching backends

| Backend | How | When |
|---|---|---|
| Direct HTTP (default) | `reqwest` against `/jobs-guest/jobs/api/...` | Normal use |
| Camoufox | `helper/camoufox_fetch.py`, a long-lived Camoufox browser spoken to over JSON lines | If plain HTTP starts getting blocked |

Camoufox setup: `pip install -U camoufox && python -m camoufox fetch`. Then
pick the backend in Settings and point it at that Python. The helper's stderr
goes to `camoufox.log` in the data directory.

All requests share one pacer (2.5 s by default, plus jitter). A 429 or 999
response stops the run instead of retrying.

## Watches (scheduled searches)

"Watch this search…" saves the current search. While the app is open, a
scheduler checks once a minute for watches that are due (every 1–72 h). A due
watch searches without the page cache, scores up to 15 new jobs against your
profile, and raises one desktop notification for the new jobs at or above its
threshold (75 by default). Each job is notified about at most once. Watches
are stored in `watches.json`.

Notifications: the packaged app (`dx bundle`) notifies as JobScout. Under
`cargo run` the app has no bundle, so it notifies through `osascript` and the
notification shows as coming from Script Editor. Use "Send test notification"
in the Watches window to check.

## Caching

LinkedIn sends `Cache-Control: no-store` and no ETag, so it can't be asked
whether a page has changed. Freshness is judged by age:

| What | Rule |
|---|---|
| Job description | Saved with the job. Fresh for 12 h (Settings). Opening a job with an older description shows it and refreshes it in the background. |
| Refresh button | Always fetches, unless the last fetch was under 2 min ago. |
| Failed job page (removed / blocked) | Not asked for again for 1 h. Network errors and rate limits don't count as failures. |
| Search results page | Reused for 10 min for the same URL (Settings, 0 turns it off). Kept in memory only, so restarting the app starts fresh. |
| Scoring | Uses the saved description when fresh, otherwise fetches it first. |

## Layout

```
src/model.rs              Job, SearchParams, Settings
src/services/linkedin.rs  URLs + HTML parsing (scraper)
src/services/fetch.rs     pacing, HTTP backend, error classes
src/services/camoufox.rs  Python helper process
src/services/llm.rs       scoring prompt + OpenAI-compatible call
src/services/store.rs     JSON files, CSV
src/state.rs              shared signals + search / fetch / score actions
src/ui/                   sidebar, list, detail pane, modals
```
