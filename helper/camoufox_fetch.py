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
                    resp = page.goto(url, wait_until="domcontentloaded", timeout=45000)
                    reply({"status": resp.status if resp else 0, "html": page.content()})
                except Exception as e:  # one bad page must not end the session
                    reply({"error": str(e)})
    except Exception as e:
        reply({"error": f"Camoufox failed to start: {e}"})


if __name__ == "__main__":
    main()
