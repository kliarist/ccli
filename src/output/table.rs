//! Aligned terminal table renderer using comfy-table.
//!
//! Used when stdout is a TTY and `--plain` is not set. Always uses
//! `UTF8_FULL_CONDENSED` preset with `ContentArrangement::Dynamic` for
//! responsive width handling (RESEARCH.md Pattern 5 + Code Examples).

use comfy_table::presets::UTF8_FULL_CONDENSED;
use comfy_table::{ContentArrangement, Table};

/// Render headers + rows as an aligned table; returns the rendered String.
pub(crate) fn render_string(
    headers: &[String],
    col_indices: &[usize],
    rows: &[Vec<String>],
    no_headers: bool,
) -> String {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic);

    if !no_headers {
        table.set_header(headers.iter().map(|s| s.as_str()).collect::<Vec<_>>());
    }
    for row in rows {
        let filtered: Vec<&str> = col_indices
            .iter()
            .map(|&i| row.get(i).map(|s| s.as_str()).unwrap_or(""))
            .collect();
        table.add_row(filtered);
    }
    table.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(strs: &[&str]) -> Vec<String> {
        strs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn renders_headers_and_rows() {
        let out = render_string(
            &h(&["Key", "Name"]),
            &[0, 1],
            &[vec!["DEV".into(), "Development".into()]],
            false,
        );
        assert!(out.contains("Key"), "output missing header 'Key': {}", out);
        assert!(
            out.contains("Name"),
            "output missing header 'Name': {}",
            out
        );
        assert!(out.contains("DEV"), "output missing cell 'DEV': {}", out);
        assert!(
            out.contains("Development"),
            "output missing cell 'Development': {}",
            out
        );
    }

    #[test]
    fn no_headers_omits_header_text() {
        let out = render_string(
            &h(&["UNIQUE_HEADER_TOKEN", "AnotherHeader"]),
            &[0, 1],
            &[vec!["DEV".into(), "Development".into()]],
            true,
        );
        assert!(
            !out.contains("UNIQUE_HEADER_TOKEN"),
            "header should be suppressed: {}",
            out
        );
        assert!(out.contains("DEV"), "rows should still print: {}", out);
    }

    #[test]
    fn col_indices_filters_columns() {
        let out = render_string(
            &h(&["Key"]),
            &[0],
            &[vec!["DEV".into(), "ShouldBeOmitted".into()]],
            false,
        );
        assert!(out.contains("Key"), "header missing: {}", out);
        assert!(out.contains("DEV"), "first col missing: {}", out);
        assert!(
            !out.contains("ShouldBeOmitted"),
            "second col should be filtered out: {}",
            out
        );
    }

    #[test]
    fn uses_utf8_box_drawing_characters() {
        let out = render_string(&h(&["A"]), &[0], &[vec!["1".into()]], false);
        // UTF8_FULL_CONDENSED uses heavy box-drawing characters.
        let has_box_char = out.contains('┃')
            || out.contains('━')
            || out.contains('┏')
            || out.contains('┗')
            || out.contains('┓')
            || out.contains('┛')
            || out.contains('│')
            || out.contains('─');
        assert!(
            has_box_char,
            "expected UTF-8 box-drawing characters in output: {}",
            out
        );
    }
}
