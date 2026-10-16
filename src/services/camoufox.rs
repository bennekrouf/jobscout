//! The Camoufox backend: a Python helper holding one browser open, spoken to
//! over JSON lines on its stdin/stdout.
//!
//! Camoufox only has Python and TypeScript launchers, so the browser is driven
//! from there. The script is compiled into the binary and written next to the
//! app's data on first use, so a bundled app needs nothing beside it but a
//! Python that has `camoufox` installed.

use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;

use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;

use super::fetch::{classify, FetchError};
use super::store;
use crate::model::Settings;

const SCRIPT: &str = include_str!("../../helper/camoufox_fetch.py");
const INSTALL_HINT: &str = "install it with: pip install -U camoufox && python -m camoufox fetch";

struct Helper {
    _child: Child,
    stdin: ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
    python: String,
    headed: bool,
}

#[derive(Deserialize)]
struct Reply {
    #[serde(default)]
    ready: bool,
    #[serde(default)]
    status: u16,
    #[serde(default)]
    html: String,
    #[serde(default)]
    error: Option<String>,
}

fn helper() -> &'static Mutex<Option<Helper>> {
    static HELPER: OnceLock<Mutex<Option<Helper>>> = OnceLock::new();
    HELPER.get_or_init(|| Mutex::new(None))
}

pub async fn get(url: &str, settings: &Settings) -> Result<String, FetchError> {
    let mut slot = helper().lock().await;
    if slot
        .as_ref()
        .is_some_and(|h| h.python != settings.python || h.headed != settings.headed)
    {
        *slot = None; // settings changed: kill_on_drop ends the old browser
    }
    if slot.is_none() {
        *slot = Some(start(settings).await?);
    }
    let h = slot.as_mut().expect("just started");
    match request(h, url).await {
        Ok(reply) => match reply.error {
            Some(e) => Err(FetchError::Browser(e)),
            None => classify(reply.status, reply.html),
        },
        Err(e) => {
            *slot = None;
            Err(FetchError::Browser(e))
        }
    }
}

async fn start(settings: &Settings) -> Result<Helper, FetchError> {
    let dir = store::data_dir();
    let script = dir.join("camoufox_fetch.py");
    std::fs::write(&script, SCRIPT).map_err(|e| FetchError::Browser(e.to_string()))?;
    let log = std::fs::File::create(dir.join("camoufox.log"))
        .map(Stdio::from)
        .unwrap_or_else(|_| Stdio::null());

    let mut cmd = Command::new(&settings.python);
    cmd.arg("-I").arg(&script);
    if settings.headed {
        cmd.arg("--headed");
    }
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(log)
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| {
            FetchError::Browser(format!("could not run {}: {e}", settings.python))
        })?;

    let stdin = child.stdin.take().expect("piped");
    let lines = BufReader::new(child.stdout.take().expect("piped")).lines();
    let mut h = Helper {
        _child: child,
        stdin,
        lines,
        python: settings.python.clone(),
        headed: settings.headed,
    };

    let first = read_reply(&mut h, Duration::from_secs(120))
        .await
        .map_err(|e| FetchError::Browser(format!("{e} — {INSTALL_HINT}")))?;
    match (first.ready, first.error) {
        (true, _) => Ok(h),
        (_, Some(e)) => Err(FetchError::Browser(format!("{e} — {INSTALL_HINT}"))),
        _ => Err(FetchError::Browser(format!("unexpected helper reply — {INSTALL_HINT}"))),
    }
}

async fn request(h: &mut Helper, url: &str) -> Result<Reply, String> {
    let line = serde_json::json!({ "url": url }).to_string() + "\n";
    h.stdin
        .write_all(line.as_bytes())
        .await
        .map_err(|e| format!("helper stopped: {e}"))?;
    h.stdin.flush().await.map_err(|e| e.to_string())?;
    read_reply(h, Duration::from_secs(90)).await
}

async fn read_reply(h: &mut Helper, limit: Duration) -> Result<Reply, String> {
    let line = tokio::time::timeout(limit, h.lines.next_line())
        .await
        .map_err(|_| "helper timed out".to_string())?
        .map_err(|e| e.to_string())?
        .ok_or_else(|| {
            format!(
                "helper exited (see {})",
                store::data_dir().join("camoufox.log").display()
            )
        })?;
    serde_json::from_str(&line).map_err(|e| format!("bad helper reply: {e}"))
}
