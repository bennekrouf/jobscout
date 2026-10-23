//! How to use JobScout from Claude: what to paste, and where.
//!
//! The command is this app's own executable, read at runtime rather than
//! written down, so the snippet is right wherever it was installed — and in a
//! development build, which is what someone trying it from a checkout runs.

use dioxus::prelude::*;

use crate::state::{AppState, Modal};

/// This executable, as a client should start it.
fn command() -> String {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "jobscout".into())
}

/// The `mcpServers` entry for Claude Desktop's config file. Built with serde
/// so a Windows path's backslashes arrive escaped.
pub fn desktop_config(command: &str) -> String {
    let config = serde_json::json!({
        "mcpServers": { "jobscout": { "command": command, "args": ["mcp"] } }
    });
    serde_json::to_string_pretty(&config).unwrap_or_default()
}

/// The one line that adds it to Claude Code, for every project.
pub fn code_command(command: &str) -> String {
    format!("claude mcp add jobscout --scope user -- \"{command}\" mcp")
}

/// Where Claude Desktop keeps its config, for the person who would rather
/// open it themselves than go through the app's menu.
fn desktop_config_path() -> &'static str {
    if cfg!(target_os = "windows") {
        "%APPDATA%\\Claude\\claude_desktop_config.json"
    } else if cfg!(target_os = "macos") {
        "~/Library/Application Support/Claude/claude_desktop_config.json"
    } else {
        "~/.config/Claude/claude_desktop_config.json"
    }
}

#[component]
pub fn McpModal() -> Element {
    let mut st = use_context::<AppState>();
    let command = command();
    let desktop = desktop_config(&command);
    let code = code_command(&command);

    rsx! {
        div { class: "modal-bg",
            div { class: "modal wide",
                div { class: "modal-head",
                    h2 { "Use JobScout from Claude" }
                    button { class: "modal-close", title: "Close", onclick: move |_| st.modal.set(Modal::None), "×" }
                }
                div { class: "modal-body",
                    p { class: "muted",
                        "Ask Claude to search, read and score jobs, mark them Saved, Applied or \
                         Hidden, update your profile and manage your watches. It works on the same \
                         jobs as this window, and what it changes shows up here within a couple of \
                         seconds."
                    }

                    div { class: "mcp-step",
                        div { class: "mcp-step-title", span { class: "mcp-num", "1" } "Claude Desktop" }
                        p { class: "muted",
                            "Settings \u{2192} Developer \u{2192} Edit Config, paste this in and save, \
                             then quit Claude and open it again. If the file already has an "
                            code { "mcpServers" }
                            " section, add just the "
                            code { "jobscout" }
                            " entry to it."
                        }
                        Snippet { text: desktop.clone() }
                        p { class: "muted mcp-path", "The file: {desktop_config_path()}" }
                    }

                    div { class: "mcp-step",
                        div { class: "mcp-step-title", span { class: "mcp-num", "2" } "Claude Code" }
                        p { class: "muted", "Run this once in a terminal:" }
                        Snippet { text: code.clone() }
                    }

                    div { class: "mcp-step",
                        div { class: "mcp-step-title", span { class: "mcp-num", "3" } "Try it" }
                        p { class: "muted",
                            "Ask: \u{201c}What are my best matches this week?\u{201d}, or \u{201c}Search \
                             for data engineer jobs in Switzerland and score the new ones.\u{201d}"
                        }
                    }

                    p { class: "mcp-note",
                        "Scoring through Claude uses the model and key set in Settings, as it does \
                         here. Claude in the browser (claude.ai) can\u{2019}t use this: it only \
                         connects to servers on the internet, and JobScout runs on your computer. Any \
                         other app that speaks MCP can, with the same command: "
                        code { "{command} mcp" }
                    }

                    div { class: "modal-actions",
                        span { class: "spacer" }
                        button { class: "btn btn-primary", onclick: move |_| st.modal.set(Modal::None), "Done" }
                    }
                }
            }
        }
    }
}

/// A block of text to paste somewhere else, with its own Copy button.
#[component]
fn Snippet(text: String) -> Element {
    let mut copied = use_signal(|| false);
    let to_copy = text.clone();
    rsx! {
        div { class: "mcp-snippet-wrap",
            pre { class: "mcp-snippet", "{text}" }
            button {
                class: if copied() { "btn btn-small on mcp-copy" } else { "btn btn-small mcp-copy" },
                onclick: move |_| {
                    // The native clipboard, as the job pane's Copy uses.
                    let ok = arboard::Clipboard::new()
                        .and_then(|mut c| c.set_text(to_copy.clone()))
                        .is_ok();
                    copied.set(ok);
                },
                if copied() { "Copied ✓" } else { "Copy" }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_desktop_entry_starts_this_app_in_mcp_mode() {
        let config: serde_json::Value = serde_json::from_str(&desktop_config(
            "/Applications/JobScout.app/Contents/MacOS/jobscout",
        ))
        .unwrap();
        let entry = &config["mcpServers"]["jobscout"];
        assert_eq!(
            entry["command"],
            "/Applications/JobScout.app/Contents/MacOS/jobscout"
        );
        assert_eq!(entry["args"], serde_json::json!(["mcp"]));
    }

    #[test]
    fn a_windows_path_survives_the_trip_through_json() {
        let path = r"C:\Program Files\JobScout\jobscout.exe";
        let config: serde_json::Value = serde_json::from_str(&desktop_config(path)).unwrap();
        assert_eq!(config["mcpServers"]["jobscout"]["command"], path);
    }

    #[test]
    fn the_code_command_quotes_a_path_with_spaces() {
        assert_eq!(
            code_command("/Apps/Job Scout/jobscout"),
            "claude mcp add jobscout --scope user -- \"/Apps/Job Scout/jobscout\" mcp"
        );
    }
}
