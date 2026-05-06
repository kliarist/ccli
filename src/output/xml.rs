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
            Ok(Event::End(ref e)) => {
                match e.name().as_ref() {
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
                }
            }
            Ok(Event::Text(ref e)) => {
                // In quick-xml 0.39, Text events contain plain text (no entity refs).
                // Entity refs like &amp; are emitted as separate Event::GeneralRef events.
                // decode() converts raw bytes → &str (character encoding only).
                let raw = e.decode().unwrap_or_default();
                emit_text(&raw, heading_depth, code_depth, &mut li_pending, &mut output);
            }
            Ok(Event::GeneralRef(ref e)) => {
                // In quick-xml 0.39, entity references (&amp; &lt; &gt; &quot; etc.) are
                // emitted as GeneralRef events separate from surrounding Text events.
                // We resolve predefined XML entities here (T-03-05: no panic on unknown ref).
                let ref_name = e.decode().unwrap_or_default();
                if let Some(resolved) = resolve_predefined_entity(&ref_name) {
                    emit_text(resolved, heading_depth, code_depth, &mut li_pending, &mut output);
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
        assert!(out.contains("HELLO\n"), "heading needs trailing newline; got: {:?}", out);
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
        assert!(!out.contains("\n\n\n"), "must not have three consecutive newlines: {:?}", out);
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
        assert!(out.contains("x") || out.contains("visible text"),
                "Pitfall 7: ac:* tags stripped, inner text retained; got: {:?}", out);
        assert!(!out.contains("ac:"), "namespace prefix must not appear in output: {:?}", out);
        assert!(!out.contains("structured-macro"), "tag name must not appear: {:?}", out);
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
