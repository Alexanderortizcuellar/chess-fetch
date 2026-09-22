use anyhow::{Context, Result};
use chrono::NaiveDate;
use colored::Colorize;
use reqwest::Client;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct ArchivesResponse {
    archives: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct MonthlyGamesResponse {
    games: Vec<ChessComGame>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ChessComGame {
    pub url: Option<String>,
    pub pgn: Option<String>,
    pub time_control: Option<String>,
    pub end_time: Option<i64>,
    pub rated: Option<bool>,
    pub time_class: Option<String>,
    pub rules: Option<String>,
    pub white: Option<ChessComPlayer>,
    pub black: Option<ChessComPlayer>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ChessComPlayer {
    pub username: Option<String>,
    pub rating: Option<u32>,
    pub result: Option<String>,
}

pub struct ChessComFilter {
    pub username: String,
    pub since: Option<NaiveDate>,
    pub until: Option<NaiveDate>,
    pub year: Option<i32>,
    pub month: Option<u32>,
    pub time_class: Option<String>,
    pub rated_only: bool,
    pub unrated_only: bool,
    pub color: Option<String>,
    pub result: Option<String>,
    pub opponent: Option<String>,
    pub limit: Option<usize>,
}

pub async fn list_archives(client: &Client, username: &str) -> Result<Vec<String>> {
    let url = format!(
        "https://api.chess.com/pub/player/{}/games/archives",
        username.to_lowercase()
    );
    let res = client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("Failed to fetch archives for user '{}'", username))?;

    if res.status() == reqwest::StatusCode::NOT_FOUND {
        anyhow::bail!("Chess.com user '{}' not found", username);
    }
    if !res.status().is_success() {
        anyhow::bail!("Chess.com API returned error: {}", res.status());
    }

    let data: ArchivesResponse = res
        .json()
        .await
        .context("Failed to parse archives JSON from Chess.com")?;
    Ok(data.archives)
}

pub async fn download_single_game(
    client: &Client,
    game_id_or_url: &str,
    output: Option<&Path>,
) -> Result<PathBuf> {
    let spinner = crate::utils::http::create_spinner("Fetching Chess.com game...");

    // Check if input is a callback URL or game ID
    let raw_id = if game_id_or_url.contains("chess.com") {
        game_id_or_url
            .split('/')
            .rfind(|s| !s.is_empty())
            .unwrap_or(game_id_or_url)
    } else {
        game_id_or_url
    };

    // Chess.com live game callback format
    let callback_url = format!("https://www.chess.com/callback/live/game/{}", raw_id);
    let res = client.get(&callback_url).send().await;

    let pgn_text = match res {
        Ok(resp) if resp.status().is_success() => {
            let json_val: serde_json::Value = resp.json().await.unwrap_or_default();
            json_val
                .get("game")
                .and_then(|g| g.get("pgn"))
                .and_then(|p| p.as_str())
                .map(|s| s.to_string())
        }
        _ => None,
    };

    let pgn = match pgn_text {
        Some(p) if !p.trim().is_empty() => p,
        _ => {
            // Fallback: try fetching page or daily game endpoint
            let daily_url = format!("https://api.chess.com/pub/game/daily/{}", raw_id);
            let resp = client.get(&daily_url).send().await?;
            if resp.status().is_success() {
                let json_val: serde_json::Value = resp.json().await?;
                json_val
                    .get("pgn")
                    .and_then(|p| p.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| anyhow::anyhow!("Game found but no PGN field returned"))?
            } else {
                anyhow::bail!("Could not retrieve PGN for Chess.com game ID {}", raw_id);
            }
        }
    };

    spinner.finish_and_clear();

    let out_path = match output {
        Some(p) => p.to_path_buf(),
        None => PathBuf::from(format!("chesscom_game_{}.pgn", raw_id)),
    };

    crate::utils::fs::save_text_to_file(&out_path, &pgn)?;
    println!(
        "{} Saved game to {}",
        "✓".green().bold(),
        out_path.display().to_string().cyan()
    );
    Ok(out_path)
}

pub async fn download_user_games(
    client: &Client,
    filter: ChessComFilter,
    output: Option<&Path>,
    split_files: bool,
    raw_pgn_bulk: bool,
) -> Result<usize> {
    let archives = list_archives(client, &filter.username).await?;
    if archives.is_empty() {
        println!(
            "{} No game archives found for user '{}'",
            "!".yellow().bold(),
            filter.username
        );
        return Ok(0);
    }

    // Filter archive URLs by year, month, since, until
    let mut matching_archives = Vec::new();
    for arch in archives {
        // arch format: "https://api.chess.com/pub/player/{user}/games/YYYY/MM"
        let parts: Vec<&str> = arch.split('/').collect();
        if parts.len() >= 2 {
            let y_str = parts[parts.len() - 2];
            let m_str = parts[parts.len() - 1];
            if let (Ok(y), Ok(m)) = (y_str.parse::<i32>(), m_str.parse::<u32>()) {
                if let Some(target_y) = filter.year {
                    if y != target_y {
                        continue;
                    }
                }
                if let Some(target_m) = filter.month {
                    if m != target_m {
                        continue;
                    }
                }
                if let Some(since_date) = filter.since {
                    // Archive end of month
                    let arch_end_date = NaiveDate::from_ymd_opt(
                        if m == 12 { y + 1 } else { y },
                        if m == 12 { 1 } else { m + 1 },
                        1,
                    )
                    .unwrap_or(since_date);
                    if arch_end_date <= since_date {
                        continue;
                    }
                }
                if let Some(until_date) = filter.until {
                    let arch_start_date = NaiveDate::from_ymd_opt(y, m, 1).unwrap_or(until_date);
                    if arch_start_date > until_date {
                        continue;
                    }
                }
                matching_archives.push((y, m, arch));
            }
        }
    }

    if matching_archives.is_empty() {
        println!(
            "{} No monthly archives matched the date filter criteria.",
            "!".yellow().bold()
        );
        return Ok(0);
    }

    println!(
        "{} Found {} monthly archives for user '{}'. Fetching games...",
        "➜".blue().bold(),
        matching_archives.len(),
        filter.username.cyan()
    );

    let mut all_games_pgn = Vec::new();
    let mut total_downloaded = 0;

    let pb = indicatif::ProgressBar::new(matching_archives.len() as u64);
    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("{msg} [{wide_bar:.cyan/blue}] {pos}/{len} months ({eta})")
            .unwrap_or_else(|_| indicatif::ProgressStyle::default_bar())
            .progress_chars("#>-"),
    );
    pb.set_message("Downloading archives");

    for (_y, _m, arch_url) in matching_archives {
        if raw_pgn_bulk
            && filter.time_class.is_none()
            && !filter.rated_only
            && !filter.unrated_only
            && filter.opponent.is_none()
            && filter.result.is_none()
            && filter.color.is_none()
            && filter.since.is_none()
            && filter.until.is_none()
        {
            // Fast direct bulk PGN download: `https://api.chess.com/pub/player/{user}/games/YYYY/MM/pgn`
            let pgn_url = format!("{}/pgn", arch_url);
            let res = client.get(&pgn_url).send().await;
            if let Ok(resp) = res {
                if resp.status().is_success() {
                    let pgn_data = resp.text().await.unwrap_or_default();
                    if !pgn_data.trim().is_empty() {
                        all_games_pgn.push(pgn_data);
                    }
                }
            }
        } else {
            // Fetch JSON for detailed filtering
            let res = client.get(&arch_url).send().await;
            if let Ok(resp) = res {
                if resp.status().is_success() {
                    if let Ok(month_data) = resp.json::<MonthlyGamesResponse>().await {
                        for g in month_data.games {
                            if let Some(pgn) = &g.pgn {
                                if passes_chesscom_filter(&g, &filter) {
                                    all_games_pgn.push(pgn.clone());
                                    total_downloaded += 1;
                                    if let Some(lim) = filter.limit {
                                        if total_downloaded >= lim {
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        pb.inc(1);
        if let Some(lim) = filter.limit {
            if total_downloaded >= lim {
                break;
            }
        }
    }
    pb.finish_and_clear();

    if all_games_pgn.is_empty() {
        println!(
            "{} No games found matching the given filters.",
            "!".yellow().bold()
        );
        return Ok(0);
    }

    let combined_pgn = all_games_pgn.join("\n\n");
    let parsed_games = crate::utils::pgn::split_pgn_games(&combined_pgn);
    let count = parsed_games.len();

    if split_files {
        let default_dir = PathBuf::from(format!("chesscom_{}", filter.username));
        let dir = output.unwrap_or(&default_dir);
        crate::utils::pgn::save_split_games(&parsed_games, dir)?;
        println!(
            "{} Successfully saved {} individual game PGNs to directory '{}'",
            "✓".green().bold(),
            count.to_string().cyan().bold(),
            dir.display().to_string().cyan()
        );
    } else {
        let default_name = format!("chesscom_{}.pgn", filter.username);
        let out_file = output
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(default_name));
        crate::utils::fs::save_text_to_file(&out_file, &combined_pgn)?;
        println!(
            "{} Successfully saved {} games to '{}'",
            "✓".green().bold(),
            count.to_string().cyan().bold(),
            out_file.display().to_string().cyan()
        );
    }

    Ok(count)
}

fn passes_chesscom_filter(g: &ChessComGame, filter: &ChessComFilter) -> bool {
    let user_lower = filter.username.to_lowercase();

    // Check rated/unrated
    if filter.rated_only && g.rated != Some(true) {
        return false;
    }
    if filter.unrated_only && g.rated != Some(false) {
        return false;
    }

    // Check time class
    if let Some(tc) = &filter.time_class {
        if g.time_class.as_deref().map(|s| s.to_lowercase()) != Some(tc.to_lowercase()) {
            return false;
        }
    }

    let is_white = g
        .white
        .as_ref()
        .and_then(|w| w.username.as_ref())
        .map(|u| u.to_lowercase())
        == Some(user_lower.clone());
    let is_black = g
        .black
        .as_ref()
        .and_then(|b| b.username.as_ref())
        .map(|u| u.to_lowercase())
        == Some(user_lower.clone());

    // Check color
    if let Some(color) = &filter.color {
        match color.to_lowercase().as_str() {
            "white" | "w" if !is_white => return false,
            "black" | "b" if !is_black => return false,
            _ => {}
        }
    }

    // Check opponent
    if let Some(opp) = &filter.opponent {
        let opp_lower = opp.to_lowercase();
        let opp_user = if is_white {
            g.black.as_ref().and_then(|b| b.username.as_ref())
        } else {
            g.white.as_ref().and_then(|w| w.username.as_ref())
        };
        if let Some(ou) = opp_user {
            if !ou.to_lowercase().contains(&opp_lower) {
                return false;
            }
        } else {
            return false;
        }
    }

    // Check result
    if let Some(res) = &filter.result {
        let user_result = if is_white {
            g.white.as_ref().and_then(|w| w.result.as_ref())
        } else {
            g.black.as_ref().and_then(|b| b.result.as_ref())
        };

        if let Some(ur) = user_result {
            let matches = match res.to_lowercase().as_str() {
                "win" => ur == "win",
                "loss" => {
                    ur != "win"
                        && ![
                            "agreed",
                            "repetition",
                            "stalemate",
                            "insufficient",
                            "timevsinsufficient",
                            "50move",
                        ]
                        .contains(&ur.as_str())
                }
                "draw" => [
                    "agreed",
                    "repetition",
                    "stalemate",
                    "insufficient",
                    "timevsinsufficient",
                    "50move",
                ]
                .contains(&ur.as_str()),
                _ => true,
            };
            if !matches {
                return false;
            }
        }
    }

    // Check date from end_time
    if let Some(end_ts) = g.end_time {
        if let Some(dt) = chrono::DateTime::from_timestamp(end_ts, 0) {
            let naive_d = dt.date_naive();
            if let Some(since_d) = filter.since {
                if naive_d < since_d {
                    return false;
                }
            }
            if let Some(until_d) = filter.until {
                if naive_d > until_d {
                    return false;
                }
            }
        }
    }

    true
}
