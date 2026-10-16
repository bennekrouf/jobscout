//! Plain text out of job-description markup, shared by every source.

use scraper::{node::Node, ElementRef, Html};

pub fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `html_to_text` for markup held in a string (JSON-LD descriptions).
pub fn markup_to_text(markup: &str) -> String {
    html_to_text(Html::parse_fragment(markup).root_element())
}

/// Readable text from description markup: paragraphs and list items on their
/// own lines, bullets kept, runs of blank lines folded into one.
pub fn html_to_text(root: ElementRef) -> String {
    let mut raw = String::new();
    walk(root, &mut raw);

    let mut out = String::new();
    let mut blank = true;
    for line in raw.lines().map(squash) {
        if line.is_empty() || line == "•" {
            if !blank {
                out.push('\n');
                blank = true;
            }
        } else {
            out.push_str(&line);
            out.push('\n');
            blank = false;
        }
    }
    out.trim().to_string()
}

fn walk(el: ElementRef, out: &mut String) {
    let name = el.value().name();
    let block = matches!(
        name,
        "p" | "div" | "ul" | "ol" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "section"
    );
    match name {
        "br" => out.push('\n'),
        "li" => out.push_str("\n• "),
        _ if block => out.push_str("\n\n"),
        _ => {}
    }
    for child in el.children() {
        match child.value() {
            Node::Text(t) => out.push_str(&t.replace('\n', " ")),
            Node::Element(_) => {
                if let Some(c) = ElementRef::wrap(child) {
                    walk(c, out);
                }
            }
            _ => {}
        }
    }
    if block {
        out.push_str("\n\n");
    }
}

