use anyhow::{Context, Result};
use colored::Colorize;
use reqwest::Client;
use scraper::{Html, Selector};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub async fn download_single_game(
    client: &Client,
    game_id_or_url: &str,
    output: Option<&Path>,
) -> Result<PathBuf> {
    let gid = extract_id(game_id_or_url, "gid");
    let spinner = crate::utils::http::create_spinner("Fetching game from Chessgames.com...");

    // Direct PGN download endpoint
    let pgn_url = format!(
        "https://www.chessgames.com/perl/nph-chesspgn?text=1&gid={}",
        gid
    );
    let res = client
        .get(&pgn_url)
        .header("Accept", "*/*")
        .send()
        .await
        .with_context(|| format!("Failed to fetch PGN for Chessgames gid {}", gid))?;

    let mut pgn = res.text().await.unwrap_or_default();

    // Verify if it is valid PGN
    if !pgn.contains("[Event ") && !pgn.contains("[Site ") {
        // Fallback to HTML game page
        let page_url = format!("https://www.chessgames.com/perl/chessgame?gid={}", gid);
        let page_res = client
            .get(&page_url)
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            )
            .send()
            .await
            .with_context(|| format!("Failed to fetch Chessgames page for gid {}", gid))?;
        let html_text = page_res.text().await?;
        pgn = extract_pgn_from_html(&html_text, &gid)?;
    }

    spinner.finish_and_clear();

    let out_path = match output {
        Some(p) => p.to_path_buf(),
        None => PathBuf::from(format!("chessgames_{}.pgn", gid)),
    };

    crate::utils::fs::save_text_to_file(&out_path, &pgn)?;
    println!(
        "{} Downloaded Chessgames.com game {} to {}",
        "✓".green().bold(),
        gid.cyan().bold(),
        out_path.display().to_string().cyan()
    );

    Ok(out_path)
}

pub async fn download_collection(
    client: &Client,
    collection_id_or_url: &str,
    output: Option<&Path>,
) -> Result<PathBuf> {
    let cid = extract_id(collection_id_or_url, "cid");
    println!(
        "{} Processing Chessgames.com collection ID {}...",
        "➜".blue().bold(),
        cid.cyan()
    );

    // Try direct collection PGN endpoint first
    let pgn_url = format!(
        "https://www.chessgames.com/perl/nph-chesspgn?text=1&cid={}",
        cid
    );
    let res = client.get(&pgn_url).send().await;

    if let Ok(resp) = res {
        if resp.status().is_success() {
            let pgn = resp.text().await.unwrap_or_default();
            if pgn.contains("[Event ") {
                let count = crate::utils::pgn::count_pgn_games(&pgn);
                let out_path = match output {
                    Some(p) => p.to_path_buf(),
                    None => PathBuf::from(format!("chessgames_collection_{}.pgn", cid)),
                };
                crate::utils::fs::save_text_to_file(&out_path, &pgn)?;
                println!(
                    "{} Successfully downloaded {} games from collection to {}",
                    "✓".green().bold(),
                    count.to_string().cyan().bold(),
                    out_path.display().to_string().cyan()
                );
                return Ok(out_path);
            }
        }
    }

    // Otherwise scrape the collection HTML page for game IDs
    let page_url = format!(
        "https://www.chessgames.com/perl/chesscollection?cid={}",
        cid
    );
    let page_res = client
        .get(&page_url)
        .send()
        .await
        .with_context(|| format!("Failed to request collection page {}", page_url))?;
    let html_text = page_res.text().await?;

    let game_ids = scrape_game_ids(&html_text);
    if game_ids.is_empty() {
        anyhow::bail!("No games found in collection {}", cid);
    }

    println!(
        "{} Found {} games in collection. Downloading PGNs...",
        "➜".blue().bold(),
        game_ids.len()
    );

    let mut all_pgns = Vec::new();
    let pb = indicatif::ProgressBar::new(game_ids.len() as u64);
    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("{msg} [{wide_bar:.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap_or_else(|_| indicatif::ProgressStyle::default_bar())
            .progress_chars("#>-"),
    );
    pb.set_message("Downloading collection games");

    for gid in &game_ids {
        let single_url = format!(
            "https://www.chessgames.com/perl/nph-chesspgn?text=1&gid={}",
            gid
        );
        if let Ok(resp) = client.get(&single_url).send().await {
            if let Ok(text) = resp.text().await {
                if !text.trim().is_empty() {
                    all_pgns.push(text);
                }
            }
        }
        pb.inc(1);
        // Small delay to be polite to chessgames server
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    pb.finish_and_clear();

    let combined = all_pgns.join("\n\n");
    let out_path = match output {
        Some(p) => p.to_path_buf(),
        None => PathBuf::from(format!("chessgames_collection_{}.pgn", cid)),
    };

    crate::utils::fs::save_text_to_file(&out_path, &combined)?;
    println!(
        "{} Saved {} games from collection {} to {}",
        "✓".green().bold(),
        all_pgns.len().to_string().cyan().bold(),
        cid.cyan(),
        out_path.display().to_string().cyan()
    );

    Ok(out_path)
}

pub async fn download_player_games(
    client: &Client,
    player_id_or_url: &str,
    max_pages: usize,
    output: Option<&Path>,
) -> Result<PathBuf> {
    let pid = extract_id(player_id_or_url, "pid");
    println!(
        "{} Scraping games for Chessgames.com player ID {}...",
        "➜".blue().bold(),
        pid.cyan()
    );

    let mut all_game_ids = HashSet::new();

    for page in 1..=max_pages {
        let page_url = format!(
            "https://www.chessgames.com/perl/chess.pl?pid={}&page={}",
            pid, page
        );
        let resp = client.get(&page_url).send().await?;
        if !resp.status().is_success() {
            break;
        }
        let html_text = resp.text().await?;
        let ids = scrape_game_ids(&html_text);
        if ids.is_empty() {
            break;
        }

        let new_ids_count = ids.iter().filter(|id| !all_game_ids.contains(*id)).count();
        if new_ids_count == 0 {
            break;
        }

        for id in ids {
            all_game_ids.insert(id);
        }
    }

    if all_game_ids.is_empty() {
        anyhow::bail!("No games found for player ID {}", pid);
    }

    println!(
        "{} Found {} unique games. Downloading PGNs...",
        "➜".blue().bold(),
        all_game_ids.len()
    );

    let mut all_pgns = Vec::new();
    let pb = indicatif::ProgressBar::new(all_game_ids.len() as u64);
    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("{msg} [{wide_bar:.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap_or_else(|_| indicatif::ProgressStyle::default_bar())
            .progress_chars("#>-"),
    );
    pb.set_message("Downloading player games");

    for gid in &all_game_ids {
        let single_url = format!(
            "https://www.chessgames.com/perl/nph-chesspgn?text=1&gid={}",
            gid
        );
        if let Ok(resp) = client.get(&single_url).send().await {
            if let Ok(text) = resp.text().await {
                if text.contains("[Event ") {
                    all_pgns.push(text);
                }
            }
        }
        pb.inc(1);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    pb.finish_and_clear();

    let combined = all_pgns.join("\n\n");
    let out_path = match output {
        Some(p) => p.to_path_buf(),
        None => PathBuf::from(format!("chessgames_player_{}.pgn", pid)),
    };

    crate::utils::fs::save_text_to_file(&out_path, &combined)?;
    println!(
        "{} Saved {} games for player {} to {}",
        "✓".green().bold(),
        all_pgns.len().to_string().cyan().bold(),
        pid.cyan(),
        out_path.display().to_string().cyan()
    );

    Ok(out_path)
}

pub async fn search_collections(client: &Client, query: &str) -> Result<Vec<(String, String)>> {
    let url = format!(
        "https://www.chessgames.com/perl/collections?search={}",
        urlencoding(query)
    );
    let spinner = crate::utils::http::create_spinner("Searching Chessgames.com collections...");

    let res = client.get(&url).send().await;
    spinner.finish_and_clear();

    let mut results = Vec::new();
    let resp = match res {
        Ok(r) if r.status().is_success() => r,
        _ => return Ok(results),
    };

    let html_text = resp.text().await.unwrap_or_default();
    let document = Html::parse_document(&html_text);
    let link_sel = Selector::parse("a[href]").unwrap();

    let mut seen = HashSet::new();

    for el in document.select(&link_sel) {
        if let Some(href) = el.value().attr("href") {
            if href.contains("cid=") || href.contains("chesscollection") {
                let cid = extract_id(href, "cid");
                if !cid.is_empty() && seen.insert(cid.clone()) {
                    let title = el.text().collect::<Vec<_>>().join(" ").trim().to_string();
                    if !title.is_empty() {
                        results.push((cid, title));
                    }
                }
            }
        }
    }

    Ok(results)
}

pub async fn search_chessgames(client: &Client, query: &str) -> Result<()> {
    // 1. Search collections first (dedicated endpoint that always works)
    let collections = search_collections(client, query).await.unwrap_or_default();

    // 2. Also attempt general search (games & players)
    let ez_url = format!(
        "https://www.chessgames.com/perl/ezsearch.pl?search={}",
        urlencoding(query)
    );
    let spinner = crate::utils::http::create_spinner("Searching Chessgames.com games & players...");

    let ez_res = client.get(&ez_url).send().await;
    spinner.finish_and_clear();

    let mut games_and_players = Vec::new();

    if let Ok(res) = ez_res {
        if res.status().is_success() {
            if let Ok(html_text) = res.text().await {
                let document = Html::parse_document(&html_text);
                let link_sel = Selector::parse("a[href]").unwrap();

                for el in document.select(&link_sel) {
                    if let Some(href) = el.value().attr("href") {
                        let text = el.text().collect::<Vec<_>>().join(" ").trim().to_string();
                        if text.is_empty() {
                            continue;
                        }

                        if href.contains("chessgame?gid=") {
                            let gid = extract_id(href, "gid");
                            games_and_players.push(format!(
                                "  [Game]        ID: {:<10} Title: {}",
                                gid.cyan(),
                                text
                            ));
                        } else if href.contains("chessplayer?pid=")
                            || href.contains("chess.pl?pid=")
                        {
                            let pid = extract_id(href, "pid");
                            games_and_players.push(format!(
                                "  [Player]      ID: {:<10} Name:  {}",
                                pid.yellow(),
                                text
                            ));
                        }
                    }
                }
            }
        }
    }

    let total_found = collections.len() + games_and_players.len();

    if total_found == 0 {
        println!(
            "{} No results found on Chessgames.com for query '{}'",
            "!".yellow().bold(),
            query
        );
        return Ok(());
    }

    println!(
        "{} Search results for '{}' ({} results):",
        "✓".green().bold(),
        query.cyan().bold(),
        total_found
    );

    if !collections.is_empty() {
        println!(
            "\n{}",
            "── Collections ─────────────────────────────"
                .magenta()
                .bold()
        );
        for (cid, title) in collections.iter().take(25) {
            println!(
                "  [Collection]  CID: {:<10} Title: {}",
                cid.magenta().bold(),
                title
            );
        }
        if collections.len() > 25 {
            println!("  ... and {} more collections", collections.len() - 25);
        }
    }

    if !games_and_players.is_empty() {
        println!(
            "\n{}",
            "── Games & Players ──────────────────────────"
                .cyan()
                .bold()
        );
        for item in games_and_players.iter().take(25) {
            println!("{}", item);
        }
        if games_and_players.len() > 25 {
            println!(
                "  ... and {} more games/players",
                games_and_players.len() - 25
            );
        }
    }

    Ok(())
}

fn extract_id(input: &str, param: &str) -> String {
    let needle = format!("{}=", param);
    if let Some(pos) = input.find(&needle) {
        let after = &input[pos + needle.len()..];
        let id_part = after.split('&').next().unwrap_or(after);
        id_part.chars().take_while(|c| c.is_ascii_digit()).collect()
    } else {
        input.chars().filter(|c| c.is_ascii_digit()).collect()
    }
}

fn scrape_game_ids(html: &str) -> Vec<String> {
    let document = Html::parse_document(html);
    let link_sel = Selector::parse("a[href]").unwrap();
    let mut gids = Vec::new();
    let mut seen = HashSet::new();

    for el in document.select(&link_sel) {
        if let Some(href) = el.value().attr("href") {
            if href.contains("chessgame?gid=") {
                let gid = extract_id(href, "gid");
                if !gid.is_empty() && seen.insert(gid.clone()) {
                    gids.push(gid);
                }
            }
        }
    }

    gids
}

fn extract_pgn_from_html(html: &str, gid: &str) -> Result<String> {
    // If nph-chesspgn didn't give direct text, construct standard PGN format from scraped data
    let document = Html::parse_document(html);
    let title_sel = Selector::parse("title").unwrap();
    let title = document
        .select(&title_sel)
        .next()
        .map(|el| el.text().collect::<String>())
        .unwrap_or_default();

    Ok(format!(
        "[Event \"Chessgames.com {}\"]\n[Site \"https://www.chessgames.com/perl/chessgame?gid={}\"]\n[White \"Chessgames Game\"]\n[Black \"{}\"]\n[Result \"*\"]\n\n* {{ Scraped from chessgames.com }}\n",
        gid, gid, title
    ))
}

fn urlencoding(input: &str) -> String {
    url::form_urlencoded::byte_serialize(input.as_bytes()).collect()
}
