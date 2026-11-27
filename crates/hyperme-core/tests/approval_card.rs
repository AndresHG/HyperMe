//! Slice 3 of specs/166-gmail-approvals: what the approval card draws: the server's Markdown as
//! safe text spans (A-5, R-11) and the choice ↵ picks (A-1, R-13). Tests cite the spec.

use hyperme_core::approval::{Choice, Span, default_choice, spans};

fn choices(values: &[&str]) -> Vec<Choice> {
    values
        .iter()
        .map(|v| Choice {
            value: (*v).to_owned(),
            label: (*v).to_owned(),
        })
        .collect()
}

/// AC-10: HTML, images, scripts and `javascript:` links come out as text; nothing is markup.
#[test]
fn markdown_becomes_text_spans_only() {
    let message = "Read **3 threads** from *outside*.\n\n<script>alert(1)</script>\n\
                   <img src=x onerror=alert(1)>\n- `inbox` first\n- [x](javascript:alert(1))";
    let got = spans(message);
    assert_eq!(
        got,
        vec![
            Span::Text("Read ".into()),
            Span::Bold("3 threads".into()),
            Span::Text(" from ".into()),
            Span::Italic("outside".into()),
            Span::Text(".".into()),
            Span::Break,
            // A wrapped line joins its paragraph with a space.
            Span::Text("<script>alert(1)</script> <img src=x onerror=alert(1)>".into()),
            Span::Item,
            Span::Code("inbox".into()),
            Span::Text(" first".into()),
            Span::Item,
            Span::Link {
                text: "x".into(),
                url: "javascript:alert(1)".into()
            },
        ]
    );
    // Every span is plain text for the web view (drawn with textContent).
    let json = serde_json::to_string(&got).unwrap();
    assert!(json.contains(r#""kind":"link""#), "{json}");
}

/// R-11: unclosed markers stay as they are.
#[test]
fn unclosed_markers_stay_text() {
    assert_eq!(
        spans("2 * 3 = 6 and **open"),
        vec![Span::Text("2 * 3 = 6 and **open".into())]
    );
    assert_eq!(spans("[no link"), vec![Span::Text("[no link".into())]);
}

/// AC-16, A-1: for a read pull, ↵ picks the broadest "Always allow" offered (this tool, then
/// these owners, then this resource); otherwise "Allow once"; never a deny.
#[test]
fn enter_picks_the_broadest_always_for_reads() {
    let gmail = choices(&["ALLOW_ONCE", "ALLOW_TOOL_ALWAYS", "DENY_ONCE"]);
    assert_eq!(default_choice(&gmail, true), Some(1));
    let calendar = choices(&[
        "ALLOW_ONCE",
        "ALLOW_OWNER_ALWAYS",
        "ALLOW_TOOL_ALWAYS",
        "DENY_ONCE",
    ]);
    assert_eq!(default_choice(&calendar, true), Some(2));
    let owners = choices(&["ALLOW_ONCE", "ALLOW_OWNER_ALWAYS", "DENY_ONCE"]);
    assert_eq!(default_choice(&owners, true), Some(1));
    assert_eq!(default_choice(&calendar, false), Some(0));
    let deny_first = choices(&["DENY_ONCE", "ALLOW_ONCE"]);
    assert_eq!(default_choice(&deny_first, true), Some(1));
    assert_eq!(default_choice(&choices(&["DENY_ONCE"]), true), None);
    let deny_always = choices(&["ALLOW_ONCE", "DENY_TOOL_ALWAYS", "ALLOW_TOOL_ALWAYS"]);
    assert_eq!(default_choice(&deny_always, true), Some(2));
    let other = choices(&["yes", "no"]);
    assert_eq!(default_choice(&other, true), Some(0));
}

/// R-11: tool names and addresses with underscores stay words; `_x_` at word edges is italic;
/// accented text is safe.
#[test]
fn underscores_inside_words_stay_text() {
    assert_eq!(
        spans("search_threads and list_drafts"),
        vec![Span::Text("search_threads and list_drafts".into())]
    );
    assert_eq!(
        spans("an _important_ note"),
        vec![
            Span::Text("an ".into()),
            Span::Italic("important".into()),
            Span::Text(" note".into())
        ]
    );
    assert_eq!(
        spans("¿Qué **más**? — `ñ`"),
        vec![
            Span::Text("¿Qué ".into()),
            Span::Bold("más".into()),
            Span::Text("? — ".into()),
            Span::Code("ñ".into())
        ]
    );
}
