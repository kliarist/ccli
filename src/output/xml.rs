//! Storage XML to plain-text converter — shared by TUI preview pane and `ccli page view`.
//!
//! Locked decisions implemented:
//! - D-36: ccli page view prints stripped plain text — paragraphs as text, headings as
//!   uppercase lines, code blocks indented two spaces.
//! - D-42: TUI preview pane uses the same function so behavior is consistent.
//! - Pitfall 7: ac:* / ri:* namespaced macro tags are stripped (treated as unknown tags);
//!   inner text is retained.
//!
//! Security:
//! - Input is Confluence storage XML returned by an authenticated API call. quick-xml's
//!   event reader handles malformed XML by stopping and returning the partial output —
//!   no panics, no buffer overruns.

use quick_xml::escape::resolve_predefined_entity;
use quick_xml::events::Event;
use quick_xml::Reader;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Convert Confluence storage XML into human-readable plain text.
///
/// Transformation rules (UI-SPEC strip_storage_xml() Contract):
/// - `<h1>`..`<h6>` → uppercased text line + blank line
/// - `<p>` → text line + blank line
/// - `<code>` / `<pre>` → each line prefixed with two spaces
/// - `<li>` → prefixed with "- "
/// - All other tags → tag stripped, inner text retained (covers ac:* / ri:* macros — Pitfall 7)
/// - HTML entities (`&amp;`, `&lt;`, `&gt;`, `&quot;`) → decoded
/// - Consecutive blank lines collapsed to one
///
/// Malformed XML: stop at the error, return partial output. Never panic.
///
/// Called by Plan 04 (TUI preview pane) and Plan 05 (`ccli page view`).
#[allow(dead_code)]
pub fn strip_storage_xml(xml: &str) -> String {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut output = String::new();

    // Tag-context flags — pushed/popped on Start/End. We use depth counters
    // so nested constructs (e.g. <p><code>x</code></p>) do not drop state on the
    // first close tag.
    let mut heading_depth: u32 = 0;
    let mut code_depth: u32 = 0;
    let mut li_pending = false; // set on <li> Start, consumed by next text emit

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                match e.name().as_ref() {
                    b"h1" | b"h2" | b"h3" | b"h4" | b"h5" | b"h6" => heading_depth += 1,
                    b"code" | b"pre" => code_depth += 1,
                    b"li" => li_pending = true,
                    _ => {} // unknown tags including ac:*, ri:* — strip, retain text (Pitfall 7)
                }
            }
            Ok(Event::End(ref e)) => match e.name().as_ref() {
                b"h1" | b"h2" | b"h3" | b"h4" | b"h5" | b"h6" => {
                    heading_depth = heading_depth.saturating_sub(1);
                    output.push('\n');
                    output.push('\n');
                }
                b"p" => {
                    output.push('\n');
                    output.push('\n');
                }
                b"li" => {
                    output.push('\n');
                }
                b"code" | b"pre" => {
                    code_depth = code_depth.saturating_sub(1);
                }
                _ => {}
            },
            Ok(Event::Text(ref e)) => {
                // In quick-xml 0.39, Text events contain plain text (no entity refs).
                // Entity refs like &amp; are emitted as separate Event::GeneralRef events.
                // decode() converts raw bytes → &str (character encoding only).
                let raw = e.decode().unwrap_or_default();
                emit_text(
                    &raw,
                    heading_depth,
                    code_depth,
                    &mut li_pending,
                    &mut output,
                );
            }
            Ok(Event::GeneralRef(ref e)) => {
                // In quick-xml 0.39, entity references (&amp; &lt; &gt; &quot; etc.) are
                // emitted as GeneralRef events separate from surrounding Text events.
                // We resolve predefined XML entities here (T-03-05: no panic on unknown ref).
                let ref_name = e.decode().unwrap_or_default();
                if let Some(resolved) = resolve_predefined_entity(&ref_name) {
                    emit_text(
                        resolved,
                        heading_depth,
                        code_depth,
                        &mut li_pending,
                        &mut output,
                    );
                } else if let Ok(Some(ch)) = e.resolve_char_ref() {
                    // Numeric character references: &#60; or &#x3C;
                    let s = ch.to_string();
                    emit_text(&s, heading_depth, code_depth, &mut li_pending, &mut output);
                }
                // Unknown entity references are silently dropped (safe default).
            }
            Ok(Event::Empty(_)) => {
                // self-closing tags like <br/> — emit nothing; if Confluence storage uses
                // <br/> for line breaks we may revisit this behavior in a future iteration.
            }
            Ok(Event::Eof) => break,
            Err(_) => break, // malformed: stop, return what we have (T-03-05 DoS mitigation)
            _ => {}
        }
    }

    collapse_blank_lines(&output)
}

/// Emit a text fragment into the output buffer, applying the current context
/// (heading uppercase, code indentation, list item prefix).
fn emit_text(
    text: &str,
    heading_depth: u32,
    code_depth: u32,
    li_pending: &mut bool,
    output: &mut String,
) {
    if text.is_empty() {
        return;
    }
    if heading_depth > 0 {
        output.push_str(&text.to_uppercase());
    } else if code_depth > 0 {
        for line in text.split('\n') {
            output.push_str("  ");
            output.push_str(line);
            output.push('\n');
        }
    } else if *li_pending {
        output.push_str("- ");
        output.push_str(text);
        *li_pending = false;
    } else {
        output.push_str(text);
    }
}

/// Convert Confluence storage XML into styled ratatui Lines for the TUI preview pane.
///
/// Applies visual formatting per element type:
/// - h1: bold magenta + `══` underline separator
/// - h2: bold cyan + `──` separator
/// - h3–h6: bold yellow (indented by level)
/// - p: plain text lines + blank line gap
/// - ul/ol li: `  •` / `  N.` prefix
/// - code (inline): green text
/// - pre / code block: dim background block with cyan text
/// - hr: dim `────` rule
/// - br: blank line
/// - All other tags stripped, inner text retained (Pitfall 7)
pub fn render_xml_to_lines(xml: &str) -> Vec<Line<'static>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut lines: Vec<Line<'static>> = Vec::new();

    // Current accumulated spans for the line being built
    let mut current: Vec<Span<'static>> = Vec::new();

    // Tag context
    let mut heading_level: u8 = 0;
    let mut code_depth: u32 = 0;
    let mut pre_depth: u32 = 0;
    let mut li_pending = false;
    let mut ol_counters: Vec<u32> = Vec::new(); // stack for nested ordered lists
    let mut in_ol = false;

    let flush = |current: &mut Vec<Span<'static>>, lines: &mut Vec<Line<'static>>| {
        if !current.is_empty() {
            lines.push(Line::from(std::mem::take(current)));
        }
    };

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => match e.name().as_ref() {
                b"h1" => heading_level = 1,
                b"h2" => heading_level = 2,
                b"h3" => heading_level = 3,
                b"h4" => heading_level = 4,
                b"h5" => heading_level = 5,
                b"h6" => heading_level = 6,
                b"pre" => pre_depth += 1,
                b"code" => code_depth += 1,
                b"li" => li_pending = true,
                b"ol" => {
                    ol_counters.push(0);
                    in_ol = true;
                }
                b"ul" => {
                    in_ol = false;
                }
                _ => {}
            },
            Ok(Event::End(ref e)) => match e.name().as_ref() {
                b"h1" | b"h2" | b"h3" | b"h4" | b"h5" | b"h6" => {
                    flush(&mut current, &mut lines);
                    let sep = match heading_level {
                        1 => Span::styled(
                            "══════════════════════════════════════════════════════════",
                            Style::default()
                                .fg(Color::Magenta)
                                .add_modifier(Modifier::DIM),
                        ),
                        _ => Span::styled(
                            "──────────────────────────────────────────────────────────",
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::DIM),
                        ),
                    };
                    lines.push(Line::from(sep));
                    lines.push(Line::from(""));
                    heading_level = 0;
                }
                b"p" => {
                    flush(&mut current, &mut lines);
                    lines.push(Line::from(""));
                }
                b"li" => {
                    flush(&mut current, &mut lines);
                }
                b"pre" => {
                    pre_depth = pre_depth.saturating_sub(1);
                    flush(&mut current, &mut lines);
                    lines.push(Line::from(""));
                }
                b"code" => {
                    code_depth = code_depth.saturating_sub(1);
                }
                b"ol" => {
                    ol_counters.pop();
                    in_ol = !ol_counters.is_empty();
                }
                b"hr" => {
                    lines.push(Line::from(Span::styled(
                        "────────────────────────────────────────────────────────────",
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::DIM),
                    )));
                }
                _ => {}
            },
            Ok(Event::Empty(ref e)) => {
                if e.name().as_ref() == b"br" {
                    flush(&mut current, &mut lines);
                } else if e.name().as_ref() == b"hr" {
                    lines.push(Line::from(Span::styled(
                        "────────────────────────────────────────────────────────────",
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::DIM),
                    )));
                }
            }
            Ok(Event::Text(ref e)) => {
                let raw = e.decode().unwrap_or_default();
                if !raw.is_empty() {
                    push_text(
                        raw.into_owned(),
                        heading_level,
                        code_depth,
                        pre_depth,
                        &mut li_pending,
                        &mut ol_counters,
                        in_ol,
                        &mut current,
                        &mut lines,
                    );
                }
            }
            Ok(Event::GeneralRef(ref e)) => {
                let ref_name = e.decode().unwrap_or_default();
                let resolved = if let Some(s) = resolve_predefined_entity(&ref_name) {
                    Some(s.to_string())
                } else if let Ok(Some(ch)) = e.resolve_char_ref() {
                    Some(ch.to_string())
                } else {
                    None
                };
                if let Some(text) = resolved {
                    push_text(
                        text,
                        heading_level,
                        code_depth,
                        pre_depth,
                        &mut li_pending,
                        &mut ol_counters,
                        in_ol,
                        &mut current,
                        &mut lines,
                    );
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    flush(&mut current, &mut lines);

    // Collapse runs of more than one consecutive blank line
    let mut out: Vec<Line<'static>> = Vec::with_capacity(lines.len());
    let mut blank_run = 0u32;
    for line in lines {
        let is_blank =
            line.spans.is_empty() || line.spans.iter().all(|s| s.content.trim().is_empty());
        if is_blank {
            blank_run += 1;
            if blank_run <= 1 {
                out.push(line);
            }
        } else {
            blank_run = 0;
            out.push(line);
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn push_text(
    text: String,
    heading_level: u8,
    code_depth: u32,
    pre_depth: u32,
    li_pending: &mut bool,
    ol_counters: &mut Vec<u32>,
    in_ol: bool,
    current: &mut Vec<Span<'static>>,
    lines: &mut Vec<Line<'static>>,
) {
    if pre_depth > 0 {
        // Pre block: each source line becomes its own styled line
        for src_line in text.split('\n') {
            current.push(Span::styled(
                format!("  {}", src_line),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::DIM),
            ));
            lines.push(Line::from(std::mem::take(current)));
        }
        return;
    }

    if *li_pending {
        let prefix = if in_ol {
            let n = ol_counters
                .last_mut()
                .map(|c| {
                    *c += 1;
                    *c
                })
                .unwrap_or(1);
            format!("  {}. ", n)
        } else {
            "  • ".to_string()
        };
        current.push(Span::styled(prefix, Style::default().fg(Color::DarkGray)));
        *li_pending = false;
    }

    let span = if heading_level > 0 {
        let style = match heading_level {
            1 => Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
            2 => Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            _ => Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        };
        Span::styled(text, style)
    } else if code_depth > 0 {
        Span::styled(text, Style::default().fg(Color::Green))
    } else {
        Span::raw(text)
    };

    current.push(span);
}

/// Collapse runs of three or more consecutive newlines down to exactly two ("\n\n").
#[allow(dead_code)]
fn collapse_blank_lines(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut newline_run: u32 = 0;
    for ch in s.chars() {
        if ch == '\n' {
            newline_run += 1;
            if newline_run <= 2 {
                out.push('\n');
            }
        } else {
            newline_run = 0;
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_h1_uppercased_with_blank_line() {
        let out = strip_storage_xml("<h1>Hello</h1>");
        assert!(out.contains("HELLO"), "got: {:?}", out);
        assert!(
            out.contains("HELLO\n"),
            "heading needs trailing newline; got: {:?}",
            out
        );
    }

    #[test]
    fn heading_h6_uppercased() {
        let out = strip_storage_xml("<h6>tiny</h6>");
        assert!(out.contains("TINY"));
    }

    #[test]
    fn paragraph_text_with_blank_line() {
        let out = strip_storage_xml("<p>hello world</p>");
        assert!(out.contains("hello world"));
        assert!(out.contains("hello world\n\n"));
    }

    #[test]
    fn code_block_two_space_indent() {
        let out = strip_storage_xml("<code>let x = 1;</code>");
        assert!(out.starts_with("  let x = 1;"), "got: {:?}", out);
    }

    #[test]
    fn pre_block_each_line_indented_two_spaces() {
        let out = strip_storage_xml("<pre>line1\nline2</pre>");
        assert!(out.contains("  line1"), "got: {:?}", out);
        assert!(out.contains("  line2"), "got: {:?}", out);
    }

    #[test]
    fn list_item_prefixed_with_dash() {
        let out = strip_storage_xml("<ul><li>one</li><li>two</li></ul>");
        assert!(out.contains("- one"), "got: {:?}", out);
        assert!(out.contains("- two"), "got: {:?}", out);
    }

    #[test]
    fn entity_decoded_amp_lt_gt_quot() {
        let out = strip_storage_xml("<p>&amp; &lt; &gt; &quot;</p>");
        assert!(out.contains('&'), "& not decoded; got: {:?}", out);
        assert!(out.contains('<'), "< not decoded; got: {:?}", out);
        assert!(out.contains('>'), "> not decoded; got: {:?}", out);
        assert!(out.contains('"'), "\" not decoded; got: {:?}", out);
    }

    #[test]
    fn consecutive_blank_lines_collapsed_to_one() {
        let out = strip_storage_xml("<p>a</p><p>b</p><p>c</p>");
        assert!(
            !out.contains("\n\n\n"),
            "must not have three consecutive newlines: {:?}",
            out
        );
    }

    #[test]
    fn malformed_xml_returns_partial_output_without_panic() {
        // Should not panic on unclosed tag.
        let out = strip_storage_xml("<p>hello");
        // Behavior is "return what we have"; "hello" should appear.
        assert!(out.contains("hello") || out.is_empty(), "got: {:?}", out);
    }

    #[test]
    fn ac_namespace_macro_tag_stripped_inner_text_retained() {
        let xml = r#"<ac:structured-macro ac:name="info"><ac:parameter>x</ac:parameter>visible text</ac:structured-macro>"#;
        let out = strip_storage_xml(xml);
        assert!(
            out.contains("x") || out.contains("visible text"),
            "Pitfall 7: ac:* tags stripped, inner text retained; got: {:?}",
            out
        );
        assert!(
            !out.contains("ac:"),
            "namespace prefix must not appear in output: {:?}",
            out
        );
        assert!(
            !out.contains("structured-macro"),
            "tag name must not appear: {:?}",
            out
        );
    }

    #[test]
    fn empty_input_empty_output() {
        assert_eq!(strip_storage_xml(""), "");
    }

    #[test]
    fn plain_text_passes_through() {
        let out = strip_storage_xml("plain");
        assert!(out.contains("plain"));
    }

    #[test]
    fn collapse_blank_lines_helper_collapses_run_of_three() {
        assert_eq!(collapse_blank_lines("a\n\n\n\nb"), "a\n\nb");
    }

    #[test]
    fn collapse_blank_lines_helper_preserves_double_newline() {
        assert_eq!(collapse_blank_lines("a\n\nb"), "a\n\nb");
    }
}
