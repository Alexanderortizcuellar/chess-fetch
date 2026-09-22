use anyhow::{Context, Result};
use colored::Colorize;
use reqwest::Client;
use scraper::{Html, Selector};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct PgnMentorEntry {
    pub name: String,
    pub filename: String,
    pub description: String,
    pub download_url: String,
    pub category: String, // "player", "opening", "event"
}

pub struct PgnMentorCatalog {
    pub players: Vec<PgnMentorEntry>,
    pub openings: Vec<PgnMentorEntry>,
    pub events: Vec<PgnMentorEntry>,
}

pub async fn fetch_catalog(client: &Client) -> Result<PgnMentorCatalog> {
    let url = "https://www.pgnmentor.com/files.html";
    let spinner = crate::utils::http::create_spinner("Fetching PGN Mentor index...");

    let res = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("Failed to fetch {}", url))?;
    let html_text = res.text().await?;
    spinner.finish_and_clear();

    let document = Html::parse_document(&html_text);
    let tr_sel = Selector::parse("tr").unwrap();
    let a_sel = Selector::parse("a").unwrap();
    let td_sel = Selector::parse("td").unwrap();

    let mut players = Vec::new();
    let mut openings = Vec::new();
    let mut events = Vec::new();

    for tr in document.select(&tr_sel) {
        let tds: Vec<_> = tr.select(&td_sel).collect();
        if tds.is_empty() {
            continue;
        }

        // Check links in first td
        for a in tds[0].select(&a_sel) {
            if let Some(href) = a.value().attr("href") {
                if href.ends_with(".zip") {
                    let desc = if tds.len() >= 2 {
                        tds[1]
                            .text()
                            .collect::<Vec<_>>()
                            .join(" ")
                            .replace('\u{a0}', " ")
                            .replace("&nbsp;", " ")
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" ")
                    } else {
                        "".to_string()
                    };

                    let file_name = href.split('/').next_back().unwrap_or(href).to_string();
                    let name = file_name
                        .strip_suffix(".zip")
                        .unwrap_or(&file_name)
                        .to_string();

                    let full_url = if href.starts_with("http") {
                        href.to_string()
                    } else if href.starts_with('/') {
                        format!("https://www.pgnmentor.com{}", href)
                    } else {
                        format!("https://www.pgnmentor.com/{}", href)
                    };

                    let entry = PgnMentorEntry {
                        name: name.clone(),
                        filename: file_name.clone(),
                        description: desc,
                        download_url: full_url,
                        category: if href.contains("players/") {
                            "player".to_string()
                        } else if href.contains("openings/") {
                            "opening".to_string()
                        } else {
                            "event".to_string()
                        },
                    };

                    if href.contains("players/") {
                        if !players
                            .iter()
                            .any(|p: &PgnMentorEntry| p.filename == file_name)
                        {
                            players.push(entry);
                        }
                    } else if href.contains("openings/") {
                        if !openings
                            .iter()
                            .any(|p: &PgnMentorEntry| p.filename == file_name)
                        {
                            openings.push(entry);
                        }
                    } else if href.contains("events/")
                        && !events
                            .iter()
                            .any(|p: &PgnMentorEntry| p.filename == file_name)
                    {
                        events.push(entry);
                    }
                }
            }
        }
    }

    Ok(PgnMentorCatalog {
        players,
        openings,
        events,
    })
}

pub async fn download_entry(
    client: &Client,
    entry: &PgnMentorEntry,
    output_dir: &Path,
    keep_zip: bool,
) -> Result<PathBuf> {
    let msg = format!("Downloading {} ({})...", entry.name, entry.category);
    let zip_bytes =
        crate::utils::http::download_bytes_with_progress(client, &entry.download_url, &msg).await?;

    if keep_zip {
        let zip_path = output_dir.join(&entry.filename);
        crate::utils::fs::save_bytes_to_file(&zip_path, &zip_bytes)?;
    }

    let pgn_text = crate::utils::fs::extract_single_pgn_from_zip(&zip_bytes)?;
    let pgn_name = format!("{}.pgn", entry.name);
    let pgn_path = output_dir.join(&pgn_name);
    crate::utils::fs::save_text_to_file(&pgn_path, &pgn_text)?;

    let count = crate::utils::pgn::count_pgn_games(&pgn_text);
    println!(
        "{} Saved {} ({} games, {}) to {}",
        "✓".green().bold(),
        entry.name.cyan().bold(),
        count.to_string().yellow(),
        entry.description.dimmed(),
        pgn_path.display().to_string().cyan()
    );

    Ok(pgn_path)
}

#[allow(clippy::too_many_arguments)]
pub async fn handle_category_action(
    client: &Client,
    entries: &[PgnMentorEntry],
    category_name: &str,
    search: Option<&str>,
    list: bool,
    download: Option<&str>,
    download_all: bool,
    output_dir: &Path,
    keep_zip: bool,
) -> Result<()> {
    let query_lower = search.map(|s| s.to_lowercase());

    let filtered: Vec<&PgnMentorEntry> = entries
        .iter()
        .filter(|e| {
            if let Some(q) = &query_lower {
                e.name.to_lowercase().contains(q) || e.description.to_lowercase().contains(q)
            } else {
                true
            }
        })
        .collect();

    if let Some(target_name) = download {
        let target_lower = target_name.to_lowercase();
        let matched: Vec<&PgnMentorEntry> = entries
            .iter()
            .filter(|e| {
                e.name.to_lowercase().contains(&target_lower)
                    || e.name.to_lowercase() == target_lower
            })
            .collect();

        if matched.is_empty() {
            println!(
                "{} No {} matching '{}' found in PGN Mentor catalog.",
                "!".yellow().bold(),
                category_name,
                target_name
            );
            return Ok(());
        }

        for entry in matched {
            download_entry(client, entry, output_dir, keep_zip).await?;
        }
        return Ok(());
    }

    if download_all {
        println!(
            "{} Downloading all {} {} files to '{}'...",
            "➜".blue().bold(),
            filtered.len(),
            category_name,
            output_dir.display()
        );
        for entry in &filtered {
            if let Err(e) = download_entry(client, entry, output_dir, keep_zip).await {
                eprintln!(
                    "{} Failed to download {}: {}",
                    "✗".red().bold(),
                    entry.name,
                    e
                );
            }
        }
        return Ok(());
    }

    if list || search.is_some() {
        println!(
            "{} Available {} in PGN Mentor ({} items):",
            "➜".blue().bold(),
            category_name.bold(),
            filtered.len().to_string().cyan().bold()
        );
        println!(
            "{:<30} {:<50}",
            "Name / ID".bold(),
            "Description / Games".bold()
        );
        println!("{}", "-".repeat(80));
        for e in filtered.iter().take(100) {
            println!("{:<30} {:<50}", e.name.cyan(), e.description);
        }
        if filtered.len() > 100 {
            println!(
                "  ... and {} more items (use --search to narrow down)",
                filtered.len() - 100
            );
        }
    } else {
        println!(
            "{} Total {} available: {}. Use --list, --search <query>, or --download <name>.",
            "➜".blue().bold(),
            category_name,
            entries.len().to_string().cyan().bold()
        );
    }

    Ok(())
}
