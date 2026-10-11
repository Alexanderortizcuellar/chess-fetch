use anyhow::Result;
use colored::Colorize;
use inquire::{Confirm, CustomType, Select, Text};
use reqwest::Client;
use std::path::PathBuf;

pub async fn run_interactive_wizard(client: &Client) -> Result<()> {
    println!(
        "\n{}",
        "╔═══════════════════════════════════════════════════════╗"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║              ♞  CHESS-FETCH CLI WIZARD  ♞              ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║      Download Chess Games & PGNs From Everywhere      ║"
            .cyan()
            .bold()
    );
    println!(
        "{}\n",
        "╚═══════════════════════════════════════════════════════╝"
            .cyan()
            .bold()
    );

    let sources = vec![
        "1. Chess.com (Official API - User Games, Archives, Single Game)",
        "2. Lichess.org (Official API - User Games, Single Game, Tournament)",
        "3. Chessgames.com (Games, Collections, Players, Search)",
        "4. The Week in Chess (TWIC Weekly Master Tournament PGNs)",
        "5. PGN Mentor / PGN Master (Players, Openings, Events Archives)",
    ];

    let choice = Select::new("Select Chess Data Source:", sources).prompt()?;

    if choice.starts_with("1.") {
        interactive_chesscom(client).await?;
    } else if choice.starts_with("2.") {
        interactive_lichess(client).await?;
    } else if choice.starts_with("3.") {
        interactive_chessgames(client).await?;
    } else if choice.starts_with("4.") {
        interactive_twic(client).await?;
    } else if choice.starts_with("5.") {
        interactive_pgnmentor(client).await?;
    }

    Ok(())
}

async fn interactive_chesscom(client: &Client) -> Result<()> {
    let actions = vec![
        "Download User Games (with optional date & speed filters)",
        "Download Single Game by ID or URL",
        "List Available Monthly Archives for User",
    ];

    let action = Select::new("Chess.com Action:", actions).prompt()?;

    if action.starts_with("Download User Games") {
        let username = Text::new("Chess.com Username:").prompt()?;
        let since_str = Text::new("Since Date (YYYY-MM-DD, or leave empty):").prompt()?;
        let until_str = Text::new("Until Date (YYYY-MM-DD, or leave empty):").prompt()?;

        let time_classes = vec!["all", "bullet", "blitz", "rapid", "daily"];
        let tc = Select::new("Time Control:", time_classes).prompt()?;
        let time_class = if tc == "all" {
            None
        } else {
            Some(tc.to_string())
        };

        let rated_choice =
            Select::new("Game Type:", vec!["All", "Rated Only", "Unrated Only"]).prompt()?;
        let rated_only = rated_choice == "Rated Only";
        let unrated_only = rated_choice == "Unrated Only";

        let color_choice = Select::new("Player Color:", vec!["Any", "White", "Black"]).prompt()?;
        let color = match color_choice {
            "White" => Some("white".to_string()),
            "Black" => Some("black".to_string()),
            _ => None,
        };

        let result_choice =
            Select::new("Outcome Filter:", vec!["Any", "Win", "Loss", "Draw"]).prompt()?;
        let result = match result_choice {
            "Win" => Some("win".to_string()),
            "Loss" => Some("loss".to_string()),
            "Draw" => Some("draw".to_string()),
            _ => None,
        };

        let split_files = Confirm::new("Save each game into a separate PGN file?")
            .with_default(false)
            .prompt()?;

        let default_out = if split_files {
            format!("chesscom_{}", username)
        } else {
            format!("chesscom_{}.pgn", username)
        };
        let out_str = Text::new("Output Path:")
            .with_default(&default_out)
            .prompt()?;

        let filter = crate::sources::chesscom::ChessComFilter {
            username: username.clone(),
            since: parse_date_opt(&since_str),
            until: parse_date_opt(&until_str),
            year: None,
            month: None,
            time_class,
            rated_only,
            unrated_only,
            color,
            result,
            opponent: None,
            limit: None,
        };

        crate::sources::chesscom::download_user_games(
            client,
            filter,
            Some(&PathBuf::from(out_str)),
            split_files,
            false,
        )
        .await?;
    } else if action.starts_with("Download Single Game") {
        let game_id = Text::new("Chess.com Game ID or URL:").prompt()?;
        let out_str = Text::new("Output PGN File (leave empty for auto):").prompt()?;
        let out_path = if out_str.trim().is_empty() {
            None
        } else {
            Some(PathBuf::from(out_str))
        };
        crate::sources::chesscom::download_single_game(client, &game_id, out_path.as_deref())
            .await?;
    } else {
        let username = Text::new("Chess.com Username:").prompt()?;
        let archives = crate::sources::chesscom::list_archives(client, &username).await?;
        println!(
            "{} Found {} monthly archives for {}:",
            "✓".green().bold(),
            archives.len(),
            username.cyan()
        );
        for a in archives {
            println!("  {}", a);
        }
    }

    Ok(())
}

async fn interactive_lichess(client: &Client) -> Result<()> {
    let actions = vec![
        "Download User Games",
        "Download Single Game by ID or URL",
        "Download Tournament Games",
        "Download Broadcast Games by ID or URL",
        "List & Search Broadcasts (Past/Finished or Scheduled)",
    ];

    let action = Select::new("Lichess Action:", actions).prompt()?;

    if action.starts_with("Download User Games") {
        let username = Text::new("Lichess Username:").prompt()?;
        let since_str = Text::new("Since Date (YYYY-MM-DD, or leave empty):").prompt()?;
        let until_str = Text::new("Until Date (YYYY-MM-DD, or leave empty):").prompt()?;

        let perf_types = vec![
            "all",
            "bullet",
            "blitz",
            "rapid",
            "classical",
            "ultraBullet",
            "correspondence",
            "chess960",
        ];
        let perf = Select::new("Speed / Variant:", perf_types).prompt()?;
        let perf_type = if perf == "all" {
            None
        } else {
            Some(perf.to_string())
        };

        let max_games: Option<usize> =
            CustomType::<usize>::new("Max games to download (0 for all):")
                .with_default(0)
                .prompt()
                .ok()
                .filter(|&n| n != 0);

        let clocks = Confirm::new("Include clock times?")
            .with_default(true)
            .prompt()?;
        let evals = Confirm::new("Include computer evaluations?")
            .with_default(true)
            .prompt()?;
        let opening = Confirm::new("Include opening names & ECO?")
            .with_default(true)
            .prompt()?;

        let split_files = Confirm::new("Save each game into a separate PGN file?")
            .with_default(false)
            .prompt()?;

        let default_out = if split_files {
            format!("lichess_{}", username)
        } else {
            format!("lichess_{}.pgn", username)
        };
        let out_str = Text::new("Output Path:")
            .with_default(&default_out)
            .prompt()?;

        let opts = crate::sources::lichess::LichessDownloadOptions {
            username: username.clone(),
            since: parse_date_opt(&since_str),
            until: parse_date_opt(&until_str),
            max: max_games,
            perf_type,
            rated_only: false,
            casual_only: false,
            color: None,
            clocks,
            evals,
            opening,
            opponent: None,
        };

        crate::sources::lichess::download_user_games(
            client,
            opts,
            Some(&PathBuf::from(out_str)),
            split_files,
        )
        .await?;
    } else if action.starts_with("Download Single Game") {
        let game_id = Text::new("Lichess Game ID or URL:").prompt()?;
        let out_str = Text::new("Output PGN File (leave empty for auto):").prompt()?;
        let out_path = if out_str.trim().is_empty() {
            None
        } else {
            Some(PathBuf::from(out_str))
        };
        crate::sources::lichess::download_single_game(
            client,
            &game_id,
            true,
            true,
            true,
            out_path.as_deref(),
        )
        .await?;
    } else if action.starts_with("Download Tournament Games") {
        let tourney_id = Text::new("Lichess Tournament ID or URL:").prompt()?;
        let out_str = Text::new("Output PGN File (leave empty for auto):").prompt()?;
        let out_path = if out_str.trim().is_empty() {
            None
        } else {
            Some(PathBuf::from(out_str))
        };
        crate::sources::lichess::download_tournament_games(
            client,
            &tourney_id,
            out_path.as_deref(),
        )
        .await?;
    } else if action.starts_with("Download Broadcast Games") {
        let broadcast_id =
            Text::new("Broadcast Tour/Round ID or URL (e.g. T9eMnL3X or URL):").prompt()?;
        let round_id_str =
            Text::new("Specific Round ID (optional, leave empty for whole tour):").prompt()?;
        let round_id = if round_id_str.trim().is_empty() {
            None
        } else {
            Some(round_id_str.trim())
        };
        let split_files = Confirm::new("Save each game into a separate PGN file?")
            .with_default(false)
            .prompt()?;
        let out_str = Text::new("Output path (leave empty for auto):").prompt()?;
        let out_path = if out_str.trim().is_empty() {
            None
        } else {
            Some(PathBuf::from(out_str))
        };

        crate::sources::lichess::download_broadcast(
            client,
            &broadcast_id,
            round_id,
            false,
            true,
            true,
            true,
            out_path.as_deref(),
            split_files,
        )
        .await?;
    } else {
        let search =
            Text::new("Search query (tournament or player name, leave empty for all):").prompt()?;
        let search_opt = if search.trim().is_empty() {
            None
        } else {
            Some(search.as_str())
        };
        let finished_only = Confirm::new("Filter only broadcasts that have already finished?")
            .with_default(true)
            .prompt()?;
        let since_str = Text::new("Since Date (YYYY-MM-DD, or leave empty):").prompt()?;
        let until_str = Text::new("Until Date (YYYY-MM-DD, or leave empty):").prompt()?;

        let items = crate::sources::lichess::list_broadcasts(
            client,
            search_opt,
            parse_date_opt(&since_str),
            parse_date_opt(&until_str),
            finished_only,
            Some(30),
        )
        .await?;

        if !items.is_empty() {
            let want_download =
                Confirm::new("Would you like to download one of these broadcasts now?")
                    .with_default(false)
                    .prompt()?;
            if want_download {
                let options: Vec<String> = items
                    .iter()
                    .map(|i| format!("{:<10} {}", i.tour.id, i.tour.name))
                    .collect();
                let selected = Select::new("Select broadcast to download:", options).prompt()?;
                let selected_id = selected.split_whitespace().next().unwrap_or("");
                if !selected_id.is_empty() {
                    let out_str = Text::new("Output PGN file (leave empty for auto):").prompt()?;
                    let out_path = if out_str.trim().is_empty() {
                        None
                    } else {
                        Some(PathBuf::from(out_str))
                    };
                    crate::sources::lichess::download_broadcast(
                        client,
                        selected_id,
                        None,
                        false,
                        true,
                        true,
                        true,
                        out_path.as_deref(),
                        false,
                    )
                    .await?;
                }
            }
        }
    }

    Ok(())
}

async fn interactive_chessgames(client: &Client) -> Result<()> {
    let actions = vec![
        "Download Single Game by ID or URL",
        "Download Game Collection by ID or URL",
        "Download Player Games by ID or URL",
        "Search Collections (e.g. Carlsen, Fischer, Traps)",
        "Search All (Collections, Players, Games)",
    ];

    let action = Select::new("Chessgames.com Action:", actions).prompt()?;

    if action.starts_with("Download Single Game") {
        let gid = Text::new("Game ID (gid) or URL (e.g. 1234567):").prompt()?;
        let out_str = Text::new("Output PGN File (leave empty for auto):").prompt()?;
        let out_path = if out_str.trim().is_empty() {
            None
        } else {
            Some(PathBuf::from(out_str))
        };
        crate::sources::chessgames::download_single_game(client, &gid, out_path.as_deref()).await?;
    } else if action.starts_with("Download Game Collection") {
        let cid = Text::new("Collection ID (cid) or URL (e.g. 1000001):").prompt()?;
        let out_str = Text::new("Output PGN File (leave empty for auto):").prompt()?;
        let out_path = if out_str.trim().is_empty() {
            None
        } else {
            Some(PathBuf::from(out_str))
        };
        crate::sources::chessgames::download_collection(client, &cid, out_path.as_deref()).await?;
    } else if action.starts_with("Download Player Games") {
        let pid = Text::new("Player ID (pid) or URL (e.g. 10240):").prompt()?;
        let max_pages: usize = CustomType::<usize>::new("Max pages to scrape:")
            .with_default(5)
            .prompt()?;
        let out_str = Text::new("Output PGN File (leave empty for auto):").prompt()?;
        let out_path = if out_str.trim().is_empty() {
            None
        } else {
            Some(PathBuf::from(out_str))
        };
        crate::sources::chessgames::download_player_games(
            client,
            &pid,
            max_pages,
            out_path.as_deref(),
        )
        .await?;
    } else if action.starts_with("Search Collections") {
        let query =
            Text::new("Collection search keyword (e.g. Carlsen, Fischer, Opening):").prompt()?;
        let collections = crate::sources::chessgames::search_collections(client, &query).await?;
        if collections.is_empty() {
            println!(
                "{} No collections found matching '{}'",
                "!".yellow().bold(),
                query
            );
        } else {
            println!(
                "{} Found {} collections:",
                "✓".green().bold(),
                collections.len().to_string().cyan()
            );
            let choices: Vec<String> = collections
                .iter()
                .map(|(cid, title)| format!("CID: {:<10} Title: {}", cid, title))
                .collect();
            let selected = Select::new(
                "Select collection to download (or Esc/Ctrl-C to cancel):",
                choices,
            )
            .prompt()?;
            let cid = selected.split_whitespace().nth(1).unwrap_or("");
            if !cid.is_empty() {
                let out_str = Text::new("Output PGN file (leave empty for auto):").prompt()?;
                let out_path = if out_str.trim().is_empty() {
                    None
                } else {
                    Some(PathBuf::from(out_str))
                };
                crate::sources::chessgames::download_collection(client, cid, out_path.as_deref())
                    .await?;
            }
        }
    } else {
        let query = Text::new("Search query:").prompt()?;
        crate::sources::chessgames::search_chessgames(client, &query).await?;
    }

    Ok(())
}

async fn interactive_twic(client: &Client) -> Result<()> {
    let actions = vec![
        "Download Latest Issue",
        "Download Specific Issue Number",
        "Download Range of Issues (e.g. 1540 to 1550)",
        "List Recent TWIC Issues",
    ];

    let action = Select::new("The Week in Chess Action:", actions).prompt()?;

    if action.starts_with("Download Latest Issue") {
        let latest = crate::sources::twic::get_latest_issue_number(client).await?;
        println!(
            "{} Latest TWIC issue is #{}",
            "➜".blue().bold(),
            latest.to_string().cyan().bold()
        );
        let out_dir = Text::new("Output directory:").with_default(".").prompt()?;
        let keep_zip = Confirm::new("Keep .zip archive?")
            .with_default(false)
            .prompt()?;
        crate::sources::twic::download_twic_issue(
            client,
            latest,
            &PathBuf::from(out_dir),
            keep_zip,
        )
        .await?;
    } else if action.starts_with("Download Specific Issue") {
        let issue: u32 = CustomType::<u32>::new("Issue Number (e.g. 1550):").prompt()?;
        let out_dir = Text::new("Output directory:").with_default(".").prompt()?;
        let keep_zip = Confirm::new("Keep .zip archive?")
            .with_default(false)
            .prompt()?;
        crate::sources::twic::download_twic_issue(client, issue, &PathBuf::from(out_dir), keep_zip)
            .await?;
    } else if action.starts_with("Download Range") {
        let start: u32 = CustomType::<u32>::new("Start Issue Number:").prompt()?;
        let end: u32 = CustomType::<u32>::new("End Issue Number:").prompt()?;
        let out_dir = Text::new("Output directory:").with_default(".").prompt()?;
        let combine = Confirm::new("Combine all issues into a single PGN?")
            .with_default(true)
            .prompt()?;
        let comb_path = if combine {
            Some(PathBuf::from(format!("twic_{}_{}.pgn", start, end)))
        } else {
            None
        };
        let keep_zip = Confirm::new("Keep .zip archives?")
            .with_default(false)
            .prompt()?;
        crate::sources::twic::download_twic_issues_range(
            client,
            start,
            end,
            &PathBuf::from(out_dir),
            comb_path.as_deref(),
            keep_zip,
        )
        .await?;
    } else {
        let count: usize = CustomType::<usize>::new("Number of issues to list:")
            .with_default(20)
            .prompt()?;
        let issues = crate::sources::twic::list_twic_issues(client, count).await?;
        println!("{} Recent TWIC Issues:", "✓".green().bold());
        for i in issues {
            println!(
                "  Issue #{:<6} {}",
                i.issue_number.to_string().cyan(),
                i.title
            );
        }
    }

    Ok(())
}

async fn interactive_pgnmentor(client: &Client) -> Result<()> {
    let catalog = crate::sources::pgnmentor::fetch_catalog(client).await?;
    println!(
        "{} Loaded catalog: {} players, {} openings, {} events",
        "✓".green().bold(),
        catalog.players.len().to_string().cyan(),
        catalog.openings.len().to_string().cyan(),
        catalog.events.len().to_string().cyan()
    );

    let categories = vec!["1. Players", "2. Openings", "3. Events"];
    let cat_choice = Select::new("Select category:", categories).prompt()?;

    let (entries, name) = if cat_choice.starts_with("1.") {
        (&catalog.players, "Player")
    } else if cat_choice.starts_with("2.") {
        (&catalog.openings, "Opening")
    } else {
        (&catalog.events, "Event")
    };

    let query = Text::new(&format!(
        "Search {} name (e.g. Carlsen, Sicilian, Candidates):",
        name
    ))
    .prompt()?;
    let q_lower = query.to_lowercase();
    let matches: Vec<_> = entries
        .iter()
        .filter(|e| {
            e.name.to_lowercase().contains(&q_lower)
                || e.description.to_lowercase().contains(&q_lower)
        })
        .collect();

    if matches.is_empty() {
        println!("{} No matching entries found.", "!".yellow().bold());
        return Ok(());
    }

    let choices: Vec<String> = matches
        .iter()
        .map(|e| format!("{:<25} ({})", e.name, e.description))
        .collect();
    let selected_str = Select::new(&format!("Select {} to download:", name), choices).prompt()?;

    let selected_name = selected_str.split_whitespace().next().unwrap_or("");
    if let Some(entry) = matches.iter().find(|e| e.name == selected_name) {
        let out_dir = Text::new("Output directory:").with_default(".").prompt()?;
        let keep_zip = Confirm::new("Keep .zip archive?")
            .with_default(false)
            .prompt()?;
        crate::sources::pgnmentor::download_entry(client, entry, &PathBuf::from(out_dir), keep_zip)
            .await?;
    }

    Ok(())
}

fn parse_date_opt(s: &str) -> Option<chrono::NaiveDate> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    let cleaned = trimmed.replace(['/', '.'], "-");
    let parts: Vec<&str> = cleaned.split('-').collect();
    if parts.len() == 3 {
        let y = parts[0].parse::<i32>().ok()?;
        let m = parts[1].parse::<u32>().ok()?;
        let d = parts[2].parse::<u32>().ok()?;
        chrono::NaiveDate::from_ymd_opt(y, m, d)
    } else if parts.len() == 2 {
        let y = parts[0].parse::<i32>().ok()?;
        let m = parts[1].parse::<u32>().ok()?;
        chrono::NaiveDate::from_ymd_opt(y, m, 1)
    } else {
        None
    }
}
