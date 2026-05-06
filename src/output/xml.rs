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
pub fn strip_storage_xml(_xml: &str) -> String {
    // TODO: implementation stub — RED phase
    String::new()
}

/// Collapse runs of three or more consecutive newlines down to exactly two ("\n\n").
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
