use anyhow::{Context, Result};
use colored::Colorize;
use reqwest::Client;
use scraper::{Html, Selector};
use std::path::Path;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TwicIssueInfo {
    pub issue_number: u32,
    pub title: String,
    pub date: String,
    pub zip_url: String,
}

pub async fn list_twic_issues(client: &Client, limit: usize) -> Result<Vec<TwicIssueInfo>> {
    let url = "https://theweekinchess.com/twic-downloads-page";
    let spinner =
        crate::utils::http::create_spinner("Fetching The Week in Chess downloads page...");

    let res = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("Failed to fetch {}", url))?;
    let html_text = res.text().await?;
    spinner.finish_and_clear();

    let document = Html::parse_document(&html_text);
    let link_sel = Selector::parse("a[href]").unwrap();

    let mut issues = Vec::new();

    for el in document.select(&link_sel) {
        if let Some(href) = el.value().attr("href") {
            if href.contains("twic") && href.ends_with("g.zip") {
                // Extract number e.g. "zips/twic1550g.zip"
                if let Some(num_str) = href
                    .split("twic")
                    .nth(1)
                    .and_then(|s| s.strip_suffix("g.zip"))
                {
                    if let Ok(num) = num_str.parse::<u32>() {
                        let full_url = if href.starts_with("http") {
                            href.to_string()
                        } else if href.starts_with('/') {
                            format!("https://theweekinchess.com{}", href)
                        } else {
                            format!("https://theweekinchess.com/{}", href)
                        };

                        let text = el.text().collect::<Vec<_>>().join(" ").trim().to_string();
                        let title = if text.is_empty() {
                            format!("The Week in Chess {}", num)
                        } else {
                            text
                        };

                        issues.push(TwicIssueInfo {
                            issue_number: num,
                            title,
                            date: "".to_string(),
                            zip_url: full_url,
                        });
                    }
                }
            }
        }
    }

    // Deduplicate and sort descending by issue number
    issues.sort_by_key(|a| std::cmp::Reverse(a.issue_number));
    issues.dedup_by_key(|i| i.issue_number);

    if limit > 0 && issues.len() > limit {
        issues.truncate(limit);
    }

    Ok(issues)
}

pub async fn get_latest_issue_number(client: &Client) -> Result<u32> {
    let list = list_twic_issues(client, 5).await;
    if let Ok(issues) = list {
        if let Some(first) = issues.first() {
            return Ok(first.issue_number);
        }
    }

    // Fallback: probe around 1550-1600
    let mut high = 1600;
    while high >= 1400 {
        let url = format!("https://theweekinchess.com/zips/twic{}g.zip", high);
        let resp = client.head(&url).send().await;
        if let Ok(r) = resp {
            if r.status().is_success() {
                return Ok(high);
            }
        }
        high -= 1;
    }

    anyhow::bail!("Could not determine latest TWIC issue number")
}

pub async fn download_twic_issue(
    client: &Client,
    issue_number: u32,
    output_dir: &Path,
    keep_zip: bool,
) -> Result<String> {
    let zip_url = format!("https://theweekinchess.com/zips/twic{}g.zip", issue_number);
    let msg = format!("Downloading TWIC issue #{}...", issue_number);
    let zip_bytes = crate::utils::http::download_bytes_with_progress(client, &zip_url, &msg)
        .await
        .with_context(|| format!("Failed to download TWIC issue #{}", issue_number))?;

    if keep_zip {
        let zip_path = output_dir.join(format!("twic{}g.zip", issue_number));
        crate::utils::fs::save_bytes_to_file(&zip_path, &zip_bytes)?;
    }

    // Extract PGN from zip
    let pgn_text = crate::utils::fs::extract_single_pgn_from_zip(&zip_bytes)?;
    let pgn_file_name = format!("twic{}.pgn", issue_number);
    let pgn_path = output_dir.join(&pgn_file_name);
    crate::utils::fs::save_text_to_file(&pgn_path, &pgn_text)?;

    let count = crate::utils::pgn::count_pgn_games(&pgn_text);
    println!(
        "{} Saved TWIC #{} ({} games) to {}",
        "✓".green().bold(),
        issue_number.to_string().cyan().bold(),
        count.to_string().yellow(),
        pgn_path.display().to_string().cyan()
    );

    Ok(pgn_text)
}

pub async fn download_twic_issues_range(
    client: &Client,
    start_issue: u32,
    end_issue: u32,
    output_dir: &Path,
    combine_file: Option<&Path>,
    keep_zip: bool,
) -> Result<()> {
    let mut combined_pgns = Vec::new();
    let total = (end_issue - start_issue + 1) as usize;

    println!(
        "{} Downloading TWIC issues {} through {} ({} total)...",
        "➜".blue().bold(),
        start_issue.to_string().cyan(),
        end_issue.to_string().cyan(),
        total
    );

    for issue in start_issue..=end_issue {
        match download_twic_issue(client, issue, output_dir, keep_zip).await {
            Ok(pgn) => {
                if combine_file.is_some() {
                    combined_pgns.push(pgn);
                }
            }
            Err(e) => {
                eprintln!(
                    "{} Failed to download TWIC #{}: {}",
                    "✗".red().bold(),
                    issue,
                    e
                );
            }
        }
    }

    if let Some(comb_path) = combine_file {
        if !combined_pgns.is_empty() {
            let full_text = combined_pgns.join("\n\n");
            crate::utils::fs::save_text_to_file(comb_path, &full_text)?;
            let count = crate::utils::pgn::count_pgn_games(&full_text);
            println!(
                "{} Combined {} issues ({} total games) into '{}'",
                "✓".green().bold(),
                combined_pgns.len().to_string().cyan(),
                count.to_string().yellow().bold(),
                comb_path.display().to_string().cyan()
            );
        }
    }

    Ok(())
}
