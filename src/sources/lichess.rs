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
