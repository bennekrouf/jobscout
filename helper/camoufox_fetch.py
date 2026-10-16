"""Camoufox page loader for JobScout.

One long-lived browser. Reads one JSON request per line on stdin,
{"url": "..."}, and writes one JSON reply per line on stdout:
{"status": 200, "html": "..."} or {"error": "..."}.
The first line written is {"ready": true}, or {"error": ...} if Camoufox
cannot start.

Setup: pip install -U camoufox && python -m camoufox fetch
"""

import json
import sys
import time

# Cloudflare's interstitial ("Just a moment...") runs a script in the page
# and then reloads it with the real content. A browser gets through by
# waiting; returning straight away would hand back the interstitial.
CHALLENGE_TITLES = ("just a moment", "un instant", "einen moment")
CHALLENGE_WAIT_S = 30


def is_challenge(page):
    # An empty or unreadable title is a page mid-reload, not one through.
    try:
        title = page.title().strip().lower()
        return not title or title.startswith(CHALLENGE_TITLES) or "_cf_chl_opt" in page.content()
    except Exception:
        return True


def load(page, url):
    resp = page.goto(url, wait_until="domcontentloaded", timeout=45000)
    status = resp.status if resp else 0
    if not is_challenge(page):
        return status
    deadline = time.monotonic() + CHALLENGE_WAIT_S
    while time.monotonic() < deadline:
        page.wait_for_timeout(1000)
        if not is_challenge(page):
            try:
                page.wait_for_load_state("domcontentloaded", timeout=15000)
            except Exception:
                pass
            if not is_challenge(page):  # still through once it has settled
                return 200
    # Never got through: report the block, never the interstitial as a page.
    return status if status >= 400 else 403


def reply(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()


def main():
    try:
        from camoufox.sync_api import Camoufox
    except ImportError as e:
        reply({"error": f"camoufox is not installed for {sys.executable}: {e}"})
        return

    headless = "--headed" not in sys.argv
    try:
        with Camoufox(headless=headless, humanize=True) as browser:
            page = browser.new_page()
            reply({"ready": True})
            for line in sys.stdin:
                line = line.strip()
                if not line:
                    continue
                try:
                    url = json.loads(line)["url"]
                    status = load(page, url)
                    reply({"status": status, "html": page.content()})
                except Exception as e:  # one bad page must not end the session
                    reply({"error": str(e)})
    except Exception as e:
        reply({"error": f"Camoufox failed to start: {e}"})


if __name__ == "__main__":
    main()
