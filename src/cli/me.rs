use crate::cli::Cli;
use crate::config;
use crate::output::OutputFormatter;

pub async fn run(cli: &Cli) -> anyhow::Result<()> {
    let config = config::load_or_error().map_err(anyhow::Error::from)?;

    let formatter = OutputFormatter::new(cli.output_config());
    let headers = &["Field", "Value"];
    formatter.print(headers, &build_rows(&config.url, config.email.as_deref()));

    Ok(())
}

fn build_rows(url: &str, email: Option<&str>) -> Vec<Vec<String>> {
    let mut rows = vec![vec![
        "URL".to_string(),
        url.trim_end_matches('/').to_string(),
    ]];
    if let Some(e) = email {
        rows.push(vec!["Email".to_string(), e.to_string()]);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_rows_with_email_includes_email_row() {
        let rows = build_rows("https://org.atlassian.net/wiki/", Some("user@example.com"));
        assert_eq!(rows[0], vec!["URL", "https://org.atlassian.net/wiki"]);
        assert_eq!(rows[1], vec!["Email", "user@example.com"]);
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn build_rows_without_email_has_only_url() {
        let rows = build_rows("https://confluence.example.com", None);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][0], "URL");
    }

    #[test]
    fn trailing_slash_stripped_from_url() {
        let rows = build_rows("https://confluence.example.com/", None);
        assert_eq!(rows[0][1], "https://confluence.example.com");
    }

    #[test]
    fn url_without_trailing_slash_is_unchanged() {
        let rows = build_rows("https://confluence.example.com", None);
        assert_eq!(rows[0][1], "https://confluence.example.com");
    }
}
