//! Output subsystem: TTY-aware dispatch between aligned table and bare TSV.
//!
//! Locked decisions implemented:
//! - D-06 TTY auto-detect: TTY → table, piped → TSV (no `--plain` required).
//!   `--plain` still forces TSV even when stdout is a TTY.
//! - D-07 `--columns` takes comma-separated names: `--columns key,name,status`.
//! - OUT-01..OUT-04 requirements wired end-to-end through `OutputFormatter::print`.

pub mod table;
pub mod tsv;
pub mod xml;
pub use xml::strip_storage_xml;

use is_terminal::IsTerminal;

/// Output flags parsed from the global CLI args (Plan 04 wires these).
pub struct OutputConfig {
    pub plain: bool,
    pub no_headers: bool,
    /// `None` = all columns; `Some(vec)` = only these columns, in this order.
    pub columns: Option<Vec<String>>,
}

/// TTY-aware dispatcher. Created once per command invocation in main.rs (Plan 04+).
pub struct OutputFormatter {
    config: OutputConfig,
    is_tty: bool,
}

impl OutputFormatter {
    pub fn new(config: OutputConfig) -> Self {
        let is_tty = std::io::stdout().is_terminal();
        Self { config, is_tty }
    }

    /// Test-only constructor that lets us pin the TTY state.
    #[cfg(test)]
    pub(crate) fn with_tty(config: OutputConfig, is_tty: bool) -> Self {
        Self { config, is_tty }
    }

    /// Print headers + rows applying column filter, --no-headers, --plain, and TTY auto-detect.
    pub fn print(&self, headers: &[&str], rows: &[Vec<String>]) {
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        let _ = self.print_to(&mut handle, headers, rows);
    }

    /// Testable variant of `print` that writes to an arbitrary `Write` impl (W-02).
    ///
    /// Allows behavioral tests of the 2×2 TTY/plain dispatch matrix without
    /// capturing stdout. Production callers use `print()` instead.
    pub(crate) fn print_to<W: std::io::Write>(
        &self,
        writer: &mut W,
        headers: &[&str],
        rows: &[Vec<String>],
    ) -> std::io::Result<()> {
        let (active_headers, col_indices) = self.resolve_columns(headers);

        if self.is_tty && !self.config.plain {
            // OUT-01: aligned table when output is interactive
            let rendered = table::render_string(&active_headers, &col_indices, rows, self.config.no_headers);
            writeln!(writer, "{}", rendered)?;
        } else {
            // OUT-02: TSV when --plain OR stdout is piped (D-06 auto-detect)
            tsv::render_to(writer, &active_headers, &col_indices, rows, self.config.no_headers)?;
        }
        Ok(())
    }

    /// Build (active_header_strings, column_index_list) from the optional --columns filter.
    ///
    /// Lookup is case-insensitive. Unknown column names are silently skipped
    /// (Phase 1 keeps it forgiving; Phase 2+ may add strict validation per command).
    pub(crate) fn resolve_columns(&self, headers: &[&str]) -> (Vec<String>, Vec<usize>) {
        match &self.config.columns {
            None => {
                let indices: Vec<usize> = (0..headers.len()).collect();
                let names: Vec<String> = headers.iter().map(|s| s.to_string()).collect();
                (names, indices)
            }
            Some(selected) => {
                let lower_headers: Vec<String> = headers.iter()
                    .map(|s| s.to_lowercase())
                    .collect();
                let mut names = Vec::new();
                let mut indices = Vec::new();
                for col in selected {
                    let col_lower = col.to_lowercase();
                    if let Some(i) = lower_headers.iter().position(|h| h == &col_lower) {
                        names.push(headers[i].to_string());
                        indices.push(i);
                    }
                }
                (names, indices)
            }
        }
    }
}

/// Parse a `--columns` value into a list of column names.
///
/// Splits on `,`, trims whitespace, drops empty entries (D-07).
///
/// Examples:
///   "key,name,status"     -> ["key", "name", "status"]
///   "key, name , status"  -> ["key", "name", "status"]
///   "key,,name"           -> ["key", "name"]
///   ""                    -> []
pub fn parse_columns(s: &str) -> Vec<String> {
    s.split(',')
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_columns_basic() {
        assert_eq!(parse_columns("key,name,status"), vec!["key", "name", "status"]);
    }

    #[test]
    fn parse_columns_trims_whitespace() {
        assert_eq!(parse_columns("key, name , status"), vec!["key", "name", "status"]);
    }

    #[test]
    fn parse_columns_filters_empty_entries() {
        assert_eq!(parse_columns("key,,name"), vec!["key", "name"]);
    }

    #[test]
    fn parse_columns_empty_input_returns_empty_vec() {
        let v: Vec<String> = parse_columns("");
        assert!(v.is_empty());
    }

    #[test]
    fn parse_columns_only_commas_returns_empty() {
        let v: Vec<String> = parse_columns(",,,");
        assert!(v.is_empty());
    }

    #[test]
    fn resolve_columns_none_returns_all() {
        let f = OutputFormatter::with_tty(
            OutputConfig { plain: false, no_headers: false, columns: None },
            true,
        );
        let (names, idx) = f.resolve_columns(&["A", "B", "C"]);
        assert_eq!(names, vec!["A", "B", "C"]);
        assert_eq!(idx, vec![0, 1, 2]);
    }

    #[test]
    fn resolve_columns_some_filters_and_reorders_case_insensitive() {
        let f = OutputFormatter::with_tty(
            OutputConfig {
                plain: false,
                no_headers: false,
                columns: Some(vec!["status".into(), "key".into()]),
            },
            true,
        );
        let (names, idx) = f.resolve_columns(&["Key", "Name", "Status"]);
        assert_eq!(names, vec!["Status", "Key"]);
        assert_eq!(idx, vec![2, 0]);
    }

    #[test]
    fn resolve_columns_skips_unknown_names() {
        let f = OutputFormatter::with_tty(
            OutputConfig {
                plain: false,
                no_headers: false,
                columns: Some(vec!["key".into(), "nonexistent".into(), "name".into()]),
            },
            true,
        );
        let (names, idx) = f.resolve_columns(&["Key", "Name", "Status"]);
        assert_eq!(names, vec!["Key", "Name"]);
        assert_eq!(idx, vec![0, 1]);
    }

    #[test]
    fn output_config_struct_fields_exist() {
        // Compile-time check: ensure the public struct fields stay stable.
        let cfg = OutputConfig { plain: true, no_headers: true, columns: Some(vec!["a".into()]) };
        assert!(cfg.plain);
        assert!(cfg.no_headers);
        assert_eq!(cfg.columns.as_ref().unwrap().len(), 1);
    }

    // ── W-02: TTY/plain dispatch matrix (2×2) ────────────────────────
    // Tests the D-06 auto-detect: piped stdout (is_tty=false, plain=false) → TSV.
    // Uses print_to() with Vec<u8> buffer so tests don't touch stdout.

    fn capture_print(is_tty: bool, plain: bool, headers: &[&str], rows: &[Vec<String>]) -> String {
        let cfg = OutputConfig { plain, no_headers: false, columns: None };
        let f = OutputFormatter::with_tty(cfg, is_tty);
        let mut buf: Vec<u8> = Vec::new();
        f.print_to(&mut buf, headers, rows).expect("print_to");
        String::from_utf8(buf).expect("utf8")
    }

    #[test]
    fn dispatch_tty_no_plain_uses_table_renderer() {
        // is_tty=true, plain=false → table (contains box-drawing chars)
        let out = capture_print(true, false, &["A", "B"], &[vec!["1".into(), "2".into()]]);
        let has_box = out.contains('┃') || out.contains('━') || out.contains('┏')
            || out.contains('┗') || out.contains('│') || out.contains('─');
        assert!(has_box, "TTY+plain=false must use table renderer; got: {}", out);
    }

    #[test]
    fn dispatch_tty_plain_uses_tsv_renderer() {
        // is_tty=true, plain=true → TSV (--plain forces TSV even on TTY)
        let out = capture_print(true, true, &["A", "B"], &[vec!["1".into(), "2".into()]]);
        assert!(out.contains("A\tB\n"), "TTY+plain=true must use TSV; got: {}", out);
        assert!(!out.contains('┃') && !out.contains('━'), "must not have box chars; got: {}", out);
    }

    #[test]
    fn dispatch_piped_no_plain_uses_tsv_renderer() {
        // is_tty=false, plain=false → TSV (D-06 auto-detect — piped/redirected stdout)
        let out = capture_print(false, false, &["A", "B"], &[vec!["1".into(), "2".into()]]);
        assert!(out.contains("A\tB\n"), "piped stdout must auto-detect TSV; got: {}", out);
        assert!(!out.contains('┃') && !out.contains('━'), "must not have box chars; got: {}", out);
    }

    #[test]
    fn dispatch_piped_plain_uses_tsv_renderer() {
        // is_tty=false, plain=true → TSV (both conditions force TSV)
        let out = capture_print(false, true, &["A", "B"], &[vec!["1".into(), "2".into()]]);
        assert!(out.contains("A\tB\n"), "piped+plain must use TSV; got: {}", out);
    }
}
