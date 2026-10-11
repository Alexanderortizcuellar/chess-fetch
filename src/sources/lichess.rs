use anyhow::{Context, Result};
use chrono::NaiveDate;
use colored::Colorize;
use reqwest::Client;
use std::path::{Path, PathBuf};

pub struct LichessDownloadOptions {
    pub username: String,
    pub since: Option<NaiveDate>,
    pub until: Option<NaiveDate>,
    pub max: Option<usize>,
    pub perf_type: Option<String>,
    pub rated_only: bool,
    pub casual_only: bool,
    pub color: Option<String>,
    pub clocks: bool,
    pub evals: bool,
    pub opening: bool,
    pub opponent: Option<String>,
}

pub async fn download_user_games(
    client: &Client,
    options: LichessDownloadOptions,
    output: Option<&Path>,
    split_files: bool,
) -> Result<usize> {
    let base_url = format!("https://lichess.org/api/games/user/{}", options.username);
    let mut query_params = Vec::new();

    if let Some(since) = options.since {
        let ts = since
            .and_hms_opt(0, 0, 0)
            .map(|dt| dt.and_utc().timestamp_millis())
            .unwrap_or(0);
        query_params.push(("since".to_string(), ts.to_string()));
    }

    if let Some(until) = options.until {
        let ts = until
            .and_hms_opt(23, 59, 59)
            .map(|dt| dt.and_utc().timestamp_millis())
            .unwrap_or(0);
        query_params.push(("until".to_string(), ts.to_string()));
    }

    if let Some(max) = options.max {
        query_params.push(("max".to_string(), max.to_string()));
    }

    if let Some(perf) = options.perf_type {
        query_params.push(("perfType".to_string(), perf));
    }

    if options.rated_only {
        query_params.push(("rated".to_string(), "true".to_string()));
    } else if options.casual_only {
        query_params.push(("rated".to_string(), "false".to_string()));
    }

    if let Some(color) = options.color {
        query_params.push(("color".to_string(), color));
    }

    if options.clocks {
        query_params.push(("clocks".to_string(), "true".to_string()));
    }

    if options.evals {
        query_params.push(("evals".to_string(), "true".to_string()));
    }

    if options.opening {
        query_params.push(("opening".to_string(), "true".to_string()));
    }

    if let Some(opp) = options.opponent {
        query_params.push(("vs".to_string(), opp));
    }

    query_params.push(("moves".to_string(), "true".to_string()));
    query_params.push(("tags".to_string(), "true".to_string()));

    let spinner = crate::utils::http::create_spinner("Connecting to Lichess API...");

    let req = client
        .get(&base_url)
        .header("Accept", "application/x-chess-pgn")
        .query(&query_params);

    let res = req.send().await.with_context(|| {
        format!(
            "Failed to request Lichess games for user '{}'",
            options.username
        )
    })?;

    if res.status() == reqwest::StatusCode::NOT_FOUND {
        spinner.finish_and_clear();
        anyhow::bail!("Lichess user '{}' not found", options.username);
    }

    if !res.status().is_success() {
        spinner.finish_and_clear();
        anyhow::bail!("Lichess API returned error: {}", res.status());
    }

    spinner.set_message("Downloading PGN stream from Lichess...");
    let pgn_text = res
        .text()
        .await
        .context("Failed to read PGN response text from Lichess")?;
    spinner.finish_and_clear();

    if pgn_text.trim().is_empty() {
        println!(
            "{} No games found for user '{}' with the specified filters.",
            "!".yellow().bold(),
            options.username
        );
        return Ok(0);
    }

    let parsed_games = crate::utils::pgn::split_pgn_games(&pgn_text);
    let count = parsed_games.len();

    if split_files {
        let default_dir = PathBuf::from(format!("lichess_{}", options.username));
        let dir = output.unwrap_or(&default_dir);
        crate::utils::pgn::save_split_games(&parsed_games, dir)?;
        println!(
            "{} Successfully saved {} individual games to directory '{}'",
            "✓".green().bold(),
            count.to_string().cyan().bold(),
            dir.display().to_string().cyan()
        );
    } else {
        let default_name = format!("lichess_{}.pgn", options.username);
        let out_file = output
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(default_name));
        crate::utils::fs::save_text_to_file(&out_file, &pgn_text)?;
        println!(
            "{} Successfully saved {} games to '{}'",
            "✓".green().bold(),
            count.to_string().cyan().bold(),
            out_file.display().to_string().cyan()
        );
    }

    Ok(count)
}

pub async fn download_single_game(
    client: &Client,
    game_id_or_url: &str,
    clocks: bool,
    evals: bool,
    opening: bool,
    output: Option<&Path>,
) -> Result<PathBuf> {
    let clean_id = if game_id_or_url.contains("lichess.org/") {
        let segs: Vec<&str> = game_id_or_url
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();
        // ID could be 8 chars or full segment
        let last = segs.last().unwrap_or(&game_id_or_url);
        // Remove #move if present
        let id_part = last.split('#').next().unwrap_or(last);
        if id_part.len() > 8 {
            &id_part[..8]
        } else {
            id_part
        }
    } else {
        if game_id_or_url.len() > 8 {
            &game_id_or_url[..8]
        } else {
            game_id_or_url
        }
    };

    let url = format!("https://lichess.org/game/export/{}", clean_id);
    let spinner = crate::utils::http::create_spinner("Fetching Lichess game...");

    let mut query = vec![("moves", "true"), ("tags", "true")];
    if clocks {
        query.push(("clocks", "true"));
    }
    if evals {
        query.push(("evals", "true"));
    }
    if opening {
        query.push(("opening", "true"));
    }

    let res = client
        .get(&url)
        .header("Accept", "application/x-chess-pgn")
        .query(&query)
        .send()
        .await
        .with_context(|| format!("Failed to fetch game {}", clean_id))?;

    if res.status() == reqwest::StatusCode::NOT_FOUND {
        spinner.finish_and_clear();
        anyhow::bail!("Lichess game ID '{}' not found", clean_id);
    }
    if !res.status().is_success() {
        spinner.finish_and_clear();
        anyhow::bail!("Lichess API returned error: {}", res.status());
    }

    let pgn = res.text().await?;
    spinner.finish_and_clear();

    let out_path = match output {
        Some(p) => p.to_path_buf(),
        None => PathBuf::from(format!("lichess_game_{}.pgn", clean_id)),
    };

    crate::utils::fs::save_text_to_file(&out_path, &pgn)?;
    println!(
        "{} Saved game to {}",
        "✓".green().bold(),
        out_path.display().to_string().cyan()
    );
    Ok(out_path)
}

pub async fn download_tournament_games(
    client: &Client,
    tournament_id_or_url: &str,
    output: Option<&Path>,
) -> Result<PathBuf> {
    let clean_id = if tournament_id_or_url.contains("lichess.org/tournament/") {
        tournament_id_or_url
            .split("/tournament/")
            .nth(1)
            .unwrap_or(tournament_id_or_url)
            .split('?')
            .next()
            .unwrap_or(tournament_id_or_url)
    } else {
        tournament_id_or_url
    };

    let url = format!("https://lichess.org/api/tournament/{}/games", clean_id);
    let spinner =
        crate::utils::http::create_spinner("Downloading tournament games from Lichess...");

    let res = client
        .get(&url)
        .header("Accept", "application/x-chess-pgn")
        .send()
        .await
        .with_context(|| format!("Failed to fetch tournament games for {}", clean_id))?;

    if !res.status().is_success() {
        spinner.finish_and_clear();
        anyhow::bail!("Failed to download tournament: HTTP {}", res.status());
    }

    let pgn = res.text().await?;
    spinner.finish_and_clear();

    let count = crate::utils::pgn::count_pgn_games(&pgn);
    let out_path = match output {
        Some(p) => p.to_path_buf(),
        None => PathBuf::from(format!("lichess_tournament_{}.pgn", clean_id)),
    };

    crate::utils::fs::save_text_to_file(&out_path, &pgn)?;
    println!(
        "{} Downloaded {} tournament games to {}",
        "✓".green().bold(),
        count.to_string().cyan().bold(),
        out_path.display().to_string().cyan()
    );
    Ok(out_path)
}

#[allow(dead_code)]
#[derive(Debug, serde::Deserialize, Clone)]
pub struct BroadcastTour {
    pub id: String,
    pub name: String,
    pub slug: Option<String>,
    pub url: Option<String>,
    pub tier: Option<u32>,
    pub dates: Option<Vec<i64>>,
    pub description: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, serde::Deserialize, Clone)]
pub struct BroadcastRound {
    pub id: String,
    pub name: String,
    pub slug: Option<String>,
    pub finished: Option<bool>,
    #[serde(rename = "finishedAt")]
    pub finished_at: Option<i64>,
    #[serde(rename = "startsAt")]
    pub starts_at: Option<i64>,
    pub url: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, serde::Deserialize, Clone)]
pub struct BroadcastItem {
    pub tour: BroadcastTour,
    pub rounds: Option<Vec<BroadcastRound>>,
    #[serde(rename = "defaultRoundId")]
    pub default_round_id: Option<String>,
    pub group: Option<String>,
}

pub async fn fetch_broadcasts(client: &Client) -> Result<Vec<BroadcastItem>> {
    let url = "https://lichess.org/api/broadcast";
    let spinner = crate::utils::http::create_spinner("Fetching Lichess broadcasts catalog...");
    let res = client
        .get(url)
        .header("Accept", "application/x-ndjson")
        .send()
        .await
        .with_context(|| "Failed to fetch Lichess broadcasts")?;

    if !res.status().is_success() {
        spinner.finish_and_clear();
        anyhow::bail!("Failed to fetch broadcasts: HTTP {}", res.status());
    }

    let text = res.text().await?;
    spinner.finish_and_clear();

    let mut items = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            if let Ok(item) = serde_json::from_str::<BroadcastItem>(trimmed) {
                items.push(item);
            }
        }
    }

    Ok(items)
}

pub async fn list_broadcasts(
    client: &Client,
    search: Option<&str>,
    since: Option<NaiveDate>,
    until: Option<NaiveDate>,
    finished_only: bool,
    limit: Option<usize>,
) -> Result<Vec<BroadcastItem>> {
    let mut broadcasts = fetch_broadcasts(client).await?;
    let now_ms = chrono::Utc::now().timestamp_millis();

    broadcasts.retain(|item| {
        // Finished / has played games filter
        let has_played_games = if let Some(rounds) = &item.rounds {
            let finished_count = rounds.iter().filter(|r| r.finished == Some(true)).count();
            finished_count > 0
        } else if let Some(dates) = &item.tour.dates {
            dates.first().map(|&start| start < now_ms).unwrap_or(false)
        } else {
            false
        };

        if finished_only && !has_played_games {
            return false;
        }

        // Date range filters
        if let Some(dates) = &item.tour.dates {
            if let Some(first_ts) = dates.first() {
                if let Some(dt) = chrono::DateTime::from_timestamp_millis(*first_ts) {
                    let d = dt.date_naive();
                    if let Some(since_d) = since {
                        if d < since_d {
                            return false;
                        }
                    }
                    if let Some(until_d) = until {
                        if d > until_d {
                            return false;
                        }
                    }
                }
            }
        }

        // Search query
        if let Some(q) = search {
            let q_lower = q.to_lowercase();
            let name_match = item.tour.name.to_lowercase().contains(&q_lower);
            let desc_match = item
                .tour
                .description
                .as_deref()
                .map(|d| d.to_lowercase().contains(&q_lower))
                .unwrap_or(false);
            let group_match = item
                .group
                .as_deref()
                .map(|g| g.to_lowercase().contains(&q_lower))
                .unwrap_or(false);
            if !name_match && !desc_match && !group_match {
                return false;
            }
        }

        true
    });

    let display_count = limit.unwrap_or(30).min(broadcasts.len());
    println!(
        "{} Found {} broadcasts (showing top {}):",
        "✓".green().bold(),
        broadcasts.len().to_string().cyan().bold(),
        display_count
    );
    println!(
        "{:<10} {:<15} {:<45} {:<14} {}",
        "Tour ID".bold(),
        "Status".bold(),
        "Tournament Name".bold(),
        "Rounds".bold(),
        "Dates".bold()
    );
    println!("{}", "-".repeat(95));

    for item in broadcasts.iter().take(display_count) {
        let (status_str, rounds_str) = if let Some(rounds) = &item.rounds {
            let total = rounds.len();
            let finished_count = rounds.iter().filter(|r| r.finished == Some(true)).count();
            if total > 0 && finished_count == total {
                (
                    "Finished".green(),
                    format!("{}/{} played", finished_count, total),
                )
            } else if finished_count > 0 {
                (
                    "In Progress".cyan(),
                    format!("{}/{} played", finished_count, total),
                )
            } else {
                ("Upcoming".yellow(), format!("{} rnds", total))
            }
        } else {
            ("Scheduled".normal(), "-".to_string())
        };

        let dates_str = if let Some(dates) = &item.tour.dates {
            let start = dates
                .first()
                .and_then(|ts| chrono::DateTime::from_timestamp_millis(*ts))
                .map(|dt| dt.format("%Y-%m-%d").to_string())
                .unwrap_or_default();
            let end = dates
                .last()
                .and_then(|ts| chrono::DateTime::from_timestamp_millis(*ts))
                .map(|dt| dt.format("%Y-%m-%d").to_string())
                .unwrap_or_default();
            if start == end {
                start
            } else {
                format!("{} to {}", start, end)
            }
        } else {
            "-".to_string()
        };

        let short_name = if item.tour.name.chars().count() > 43 {
            format!("{}...", item.tour.name.chars().take(40).collect::<String>())
        } else {
            item.tour.name.clone()
        };

        println!(
            "{:<10} {:<15} {:<45} {:<12} {}",
            item.tour.id.cyan().bold(),
            status_str,
            short_name,
            rounds_str,
            dates_str.dimmed()
        );
    }

    Ok(broadcasts)
}

#[allow(clippy::too_many_arguments)]
pub async fn download_broadcast(
    client: &Client,
    id_or_url: &str,
    round_id: Option<&str>,
    round_only: bool,
    clocks: bool,
    comments: bool,
    variations: bool,
    output: Option<&Path>,
    split_files: bool,
) -> Result<PathBuf> {
    let (mut target_type, clean_id) =
        parse_broadcast_id_or_url(client, id_or_url, round_id, round_only).await;

    let mut query = Vec::new();
    if !clocks {
        query.push(("clocks", "false"));
    }
    if !comments {
        query.push(("comments", "false"));
    }
    if !variations {
        query.push(("variations", "false"));
    }

    let spinner = crate::utils::http::create_spinner("Downloading Lichess broadcast PGN...");

    let url = match target_type {
        BroadcastTarget::Round => {
            format!("https://lichess.org/api/broadcast/round/{}.pgn", clean_id)
        }
        BroadcastTarget::Tour => format!("https://lichess.org/api/broadcast/{}.pgn", clean_id),
    };

    let mut res = client
        .get(&url)
        .query(&query)
        .send()
        .await
        .with_context(|| format!("Failed to download broadcast {}", clean_id))?;

    // If a Tour request returned 404, check if the ID was actually a Round ID
    if res.status() == reqwest::StatusCode::NOT_FOUND && target_type == BroadcastTarget::Tour {
        let round_fallback = format!("https://lichess.org/api/broadcast/round/{}.pgn", clean_id);
        if let Ok(round_res) = client.get(&round_fallback).query(&query).send().await {
            if round_res.status().is_success() {
                println!(
                    "{} '{}' was identified as a Round ID. Downloading round...",
                    "ℹ".cyan().bold(),
                    clean_id.cyan()
                );
                target_type = BroadcastTarget::Round;
                res = round_res;
            }
        }
    }

    if res.status() == reqwest::StatusCode::NOT_FOUND {
        spinner.finish_and_clear();
        anyhow::bail!(
            "Lichess broadcast '{}' not found. Ensure the ID or URL is valid.",
            clean_id
        );
    }
    if !res.status().is_success() {
        spinner.finish_and_clear();
        anyhow::bail!("Failed to download broadcast: HTTP {}", res.status());
    }

    let pgn = res.text().await?;
    spinner.finish_and_clear();

    if pgn.trim().is_empty() {
        println!(
            "{} Broadcast '{}' returned empty PGN (no completed games yet).",
            "!".yellow().bold(),
            clean_id
        );
    }

    let parsed_games = crate::utils::pgn::split_pgn_games(&pgn);
    let count = parsed_games.len();

    let target_prefix = match target_type {
        BroadcastTarget::Tour => "broadcast_tour",
        BroadcastTarget::Round => "broadcast_round",
    };

    let out_path = if split_files {
        let default_dir = PathBuf::from(format!("lichess_{}_{}", target_prefix, clean_id));
        let dir = output.unwrap_or(&default_dir);
        crate::utils::pgn::save_split_games(&parsed_games, dir)?;
        println!(
            "{} Successfully saved {} broadcast games into directory '{}'",
            "✓".green().bold(),
            count.to_string().cyan().bold(),
            dir.display().to_string().cyan()
        );
        dir.to_path_buf()
    } else {
        let default_file = PathBuf::from(format!("lichess_{}_{}.pgn", target_prefix, clean_id));
        let out_file = output.map(PathBuf::from).unwrap_or(default_file);
        crate::utils::fs::save_text_to_file(&out_file, &pgn)?;
        println!(
            "{} Successfully downloaded {} broadcast games to '{}'",
            "✓".green().bold(),
            count.to_string().cyan().bold(),
            out_file.display().to_string().cyan()
        );
        out_file
    };

    Ok(out_path)
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum BroadcastTarget {
    Tour,
    Round,
}

async fn parse_broadcast_id_or_url(
    client: &Client,
    input: &str,
    explicit_round: Option<&str>,
    round_only: bool,
) -> (BroadcastTarget, String) {
    if let Some(r) = explicit_round {
        let clean = r.split('/').next_back().unwrap_or(r);
        return (BroadcastTarget::Round, clean.to_string());
    }

    let trimmed = input.trim().trim_end_matches('/');
    if trimmed.contains("lichess.org/broadcast/") {
        let parts: Vec<&str> = trimmed
            .split("/broadcast/")
            .nth(1)
            .unwrap_or("")
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        if parts.len() >= 3 {
            let round_id = parts[2];
            if round_only {
                return (BroadcastTarget::Round, round_id.to_string());
            }

            // By default, try resolving the parent tournament (Tour ID) to download all rounds
            if let Some((tour_id, tour_name)) =
                resolve_tour_from_broadcast_url(client, trimmed).await
            {
                println!(
                    "{} Detected tournament '{}' (Tour ID: {}). Downloading all rounds...",
                    "ℹ".cyan().bold(),
                    tour_name.cyan(),
                    tour_id.bold()
                );
                println!(
                    "   {} Use {} to download only this single round.",
                    "Tip:".dimmed(),
                    "--round-only".yellow()
                );
                return (BroadcastTarget::Tour, tour_id);
            }

            // Fallback to round if tour could not be resolved
            return (BroadcastTarget::Round, round_id.to_string());
        } else if parts.len() == 2 {
            let tour_id = parts[1];
            return (BroadcastTarget::Tour, tour_id.to_string());
        }
    }

    (BroadcastTarget::Tour, trimmed.to_string())
}

async fn resolve_tour_from_broadcast_url(client: &Client, url: &str) -> Option<(String, String)> {
    let res = client.get(url).send().await.ok()?;
    if !res.status().is_success() {
        return None;
    }
    let html = res.text().await.ok()?;

    // Search for <script type="application/json" id="page-init-data">
    let marker = r#"id="page-init-data">"#;
    let start_pos = html.find(marker)? + marker.len();
    let rest = &html[start_pos..];
    let end_pos = rest.find("</script>")?;
    let json_text = &rest[..end_pos];

    let json: serde_json::Value = serde_json::from_str(json_text).ok()?;
    let tour = json.get("relay")?.get("tour")?;
    let id = tour.get("id")?.as_str()?.to_string();
    let name = tour
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("")
        .to_string();

    Some((id, name))
}
