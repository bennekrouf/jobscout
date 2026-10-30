# JobScout skill

A skill for AI agents that use **JobScout**, the desktop app that collects job posts from LinkedIn and national job boards in nine countries and scores them against your CV. The
skill tells the agent how to use JobScout's tools well: which to call first, what
needs your yes, sensible defaults, and how to report back.

## What you need

- **JobScout**, installed from <https://mayorana.ch/en/apps/jobscout> (macOS,
  Windows or Linux). This skill does not include the app.
- A desktop agent connected to JobScout's MCP server: Claude Desktop, Claude
  Code, Codex CLI or Cursor. `references/connect.md` has the steps. Web-only
  agents such as ChatGPT can't reach an app on your computer.

## What you can ask for

- Search the boards and score the new posts against your profile
- Show your best matches with their strengths and gaps
- Mark jobs saved, applied or hidden
- Set up watches: searches the app runs on a schedule
- Hand a job to cvenom to tailor your CV

## What waits for your yes

- Replacing your profile
- Deleting a watch
- Choosing where an export is written

## Limits

The free version runs 10 searches a month; paid plans run more. Scoring with api0 uses your licence's AI credits; a local ollama costs nothing.

## Install

- **Claude Code**: unzip into `~/.claude/skills/` so the folder is
  `~/.claude/skills/jobscout/`.
- **claude.ai or Claude Desktop**: upload the zip under Skills in Settings.
- **Other agents**: put the `jobscout` folder where the agent reads skills.

## Help

Questions, licences and bug reports: <https://mayorana.ch/en/contact>.
