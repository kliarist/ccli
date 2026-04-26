//! Bare tab-separated values renderer.
//!
//! IMPORTANT: This module does NOT use comfy-table. comfy-table's `NOTHING` preset
//! removes borders but still pads cells with spaces to align columns — that breaks
//! `awk -F'\t'`, `cut -f2`, and `sort -t$'\t'` pipelines (RESEARCH.md Pitfall 5).
//! Simple `fields.join("\t")` is the correct approach for OUT-02.

use std::io::{self, Write};

/// Render headers + rows as tab-separated values to stdout.
///
/// `col_indices` selects which columns of each row to print, in the order listed.
/// `headers` is already filtered/ordered to match `col_indices` by the caller
/// (see `OutputFormatter::resolve_columns`).
pub fn render(
    headers: &[String],
    col_indices: &[usize],
    rows: &[Vec<String>],
    no_headers: bool,
) {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    // Errors writing to stdout are not actionable in a CLI; ignore the Result.
    let _ = render_to(&mut handle, headers, col_indices, rows, no_headers);
}

/// Testable variant of `render` accepting an explicit writer.
pub(crate) fn render_to<W: Write>(
    writer: &mut W,
    headers: &[String],
    col_indices: &[usize],
    rows: &[Vec<String>],
    no_headers: bool,
) -> io::Result<()> {
    if !no_headers {
        writeln!(writer, "{}", headers.join("\t"))?;
    }
    for row in rows {
        let filtered: Vec<&str> = col_indices.iter()
            .map(|&i| row[i].as_str())
            .collect();
        writeln!(writer, "{}", filtered.join("\t"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capture(headers: &[&str], col_indices: &[usize], rows: &[Vec<String>], no_headers: bool) -> String {
        let owned: Vec<String> = headers.iter().map(|s| s.to_string()).collect();
        let mut buf: Vec<u8> = Vec::new();
        render_to(&mut buf, &owned, col_indices, rows, no_headers).expect("write to Vec");
        String::from_utf8(buf).expect("utf8")
    }

    #[test]
    fn renders_headers_and_rows_tab_separated() {
        let out = capture(
            &["a", "b"],
            &[0, 1],
            &[vec!["1".into(), "2".into()]],
            false,
        );
        assert_eq!(out, "a\tb\n1\t2\n");
    }

    #[test]
    fn no_headers_suppresses_header_line() {
        let out = capture(
            &["a", "b"],
            &[0, 1],
            &[vec!["1".into(), "2".into()]],
            true,
        );
        assert_eq!(out, "1\t2\n");
    }

    #[test]
    fn col_indices_filters_columns_in_order() {
        let out = capture(
            &["a", "c"],
            &[0, 2],
            &[vec!["1".into(), "2".into(), "3".into()]],
            false,
        );
        assert_eq!(out, "a\tc\n1\t3\n");
    }

    #[test]
    fn col_indices_can_reorder_columns() {
        let out = capture(
            &["c", "a"],
            &[2, 0],
            &[vec!["1".into(), "2".into(), "3".into()]],
            false,
        );
        assert_eq!(out, "c\ta\n3\t1\n");
    }

    #[test]
    fn empty_rows_only_prints_header() {
        let out = capture(&["a", "b"], &[0, 1], &[], false);
        assert_eq!(out, "a\tb\n");
    }

    #[test]
    fn empty_rows_with_no_headers_prints_nothing() {
        let out = capture(&["a", "b"], &[0, 1], &[], true);
        assert_eq!(out, "");
    }
}
