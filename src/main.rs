mod cli;
mod interactive;
mod sources;
mod utils;

use anyhow::Result;
use clap::{CommandFactory, Parser};
use clap_complete::generate;
use cli::{
    ChessComSubcommand, ChessResultsSubcommand, ChessgamesSubcommand, Cli, Commands,
    LichessSubcommand, PgnMentorSubcommand, ShellType, TwicSubcommand,
};
use colored::Colorize;
use std::io;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let client = utils::http::build_client()?;

    match cli.command {
        Some(Commands::ChessCom(args)) => match args.command {
            ChessComSubcommand::Download {
                username,
                since,
                until,
                year,
                month,
                time_class,
                rated_only,
                unrated_only,
                color,
                result,
                vs,
                limit,
                output,
                split_files,
                pgn_bulk,
            } => {
                let filter = sources::chesscom::ChessComFilter {
                    username,
                    since: since.as_deref().and_then(parse_date),
                    until: until.as_deref().and_then(parse_date),
                    year,
                    month,
                    time_class,
                    rated_only,
                    unrated_only,
                    color,
                    result,
                    opponent: vs,
                    limit,
                };
                sources::chesscom::download_user_games(
                    &client,
                    filter,
                    output.as_deref(),
                    split_files,
                    pgn_bulk,
                )
                .await?;
            }
            ChessComSubcommand::Archives { username } => {
                let archives = sources::chesscom::list_archives(&client, &username).await?;
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
            ChessComSubcommand::Game { id, output } => {
                sources::chesscom::download_single_game(&client, &id, output.as_deref()).await?;
            }
        },

        Some(Commands::Lichess(args)) => match args.command {
            LichessSubcommand::Download {
                username,
                since,
                until,
                max,
                perf_type,
                rated_only,
                casual_only,
                color,
                clocks,
                evals,
                opening,
                vs,
                output,
                split_files,
            } => {
                let opts = sources::lichess::LichessDownloadOptions {
                    username,
                    since: since.as_deref().and_then(parse_date),
                    until: until.as_deref().and_then(parse_date),
                    max,
                    perf_type,
                    rated_only,
                    casual_only,
                    color,
                    clocks,
                    evals,
                    opening,
                    opponent: vs,
                };
                sources::lichess::download_user_games(
                    &client,
                    opts,
                    output.as_deref(),
                    split_files,
                )
                .await?;
            }
            LichessSubcommand::Game {
                id,
                clocks,
                evals,
                opening,
                output,
            } => {
                sources::lichess::download_single_game(
                    &client,
                    &id,
                    clocks,
                    evals,
                    opening,
                    output.as_deref(),
                )
                .await?;
            }
            LichessSubcommand::Tournament { id, output } => {
                sources::lichess::download_tournament_games(&client, &id, output.as_deref())
                    .await?;
            }
            LichessSubcommand::Broadcast {
                id,
                round,
                round_only,
                list,
                search,
                since,
                until,
                finished_only,
                limit,
                clocks,
                comments,
                variations,
                output,
                split_files,
            } => {
                if list || (id.is_none() && round.is_none()) {
                    sources::lichess::list_broadcasts(
                        &client,
                        search.as_deref(),
                        since.as_deref().and_then(parse_date),
                        until.as_deref().and_then(parse_date),
                        finished_only,
                        limit,
                    )
                    .await?;
                } else if let Some(target_id) = id.as_deref().or(round.as_deref()) {
                    sources::lichess::download_broadcast(
                        &client,
                        target_id,
                        round.as_deref(),
                        round_only,
                        clocks,
                        comments,
                        variations,
                        output.as_deref(),
                        split_files,
                    )
                    .await?;
                }
            }
        },

        Some(Commands::Chessgames(args)) => match args.command {
            ChessgamesSubcommand::Game { id, output } => {
                sources::chessgames::download_single_game(&client, &id, output.as_deref()).await?;
            }
            ChessgamesSubcommand::Collection { id, output } => {
                sources::chessgames::download_collection(&client, &id, output.as_deref()).await?;
            }
            ChessgamesSubcommand::Player {
                id,
                max_pages,
                output,
            } => {
                sources::chessgames::download_player_games(
                    &client,
                    &id,
                    max_pages,
                    output.as_deref(),
                )
                .await?;
            }
            ChessgamesSubcommand::Search { query } => {
                sources::chessgames::search_chessgames(&client, &query).await?;
            }
        },

        Some(Commands::Twic(args)) => match args.command {
            TwicSubcommand::Download {
                issue,
                range,
                latest,
                count,
                output_dir,
                combine,
                keep_zip,
            } => {
                if latest {
                    let latest_num = sources::twic::get_latest_issue_number(&client).await?;
                    sources::twic::download_twic_issue(&client, latest_num, &output_dir, keep_zip)
                        .await?;
                } else if let Some(n) = count {
                    let latest_num = sources::twic::get_latest_issue_number(&client).await?;
                    let start = latest_num.saturating_sub(n).max(1) + 1;
                    sources::twic::download_twic_issues_range(
                        &client,
                        start,
                        latest_num,
                        &output_dir,
                        combine.as_deref(),
                        keep_zip,
                    )
                    .await?;
                } else if let Some(r) = range {
                    let parts: Vec<&str> = r.split("..").collect();
                    if parts.len() == 2 {
                        let start: u32 = parts[0].parse()?;
                        let end: u32 = parts[1].parse()?;
                        sources::twic::download_twic_issues_range(
                            &client,
                            start,
                            end,
                            &output_dir,
                            combine.as_deref(),
                            keep_zip,
                        )
                        .await?;
                    } else {
                        anyhow::bail!("Invalid range format. Use e.g. 1500..1510");
                    }
                } else if let Some(iss) = issue {
                    sources::twic::download_twic_issue(&client, iss, &output_dir, keep_zip).await?;
                } else {
                    anyhow::bail!(
                        "Please specify --issue <NUM>, --range <A..B>, --latest, or --count <N>"
                    );
                }
            }
            TwicSubcommand::List { count } => {
                let issues = sources::twic::list_twic_issues(&client, count).await?;
                println!("{} Recent TWIC Issues:", "✓".green().bold());
                for i in issues {
                    println!(
                        "  Issue #{:<6} {}",
                        i.issue_number.to_string().cyan(),
                        i.title
                    );
                }
            }
        },

        Some(Commands::PgnMentor(args)) => {
            let catalog = sources::pgnmentor::fetch_catalog(&client).await?;
            match args.command {
                PgnMentorSubcommand::Players {
                    search,
                    list,
                    download,
                    all,
                    output_dir,
                    keep_zip,
                } => {
                    sources::pgnmentor::handle_category_action(
                        &client,
                        &catalog.players,
                        "Players",
                        search.as_deref(),
                        list,
                        download.as_deref(),
                        all,
                        &output_dir,
                        keep_zip,
                    )
                    .await?;
                }
                PgnMentorSubcommand::Openings {
                    search,
                    list,
                    download,
                    all,
                    output_dir,
                    keep_zip,
                } => {
                    sources::pgnmentor::handle_category_action(
                        &client,
                        &catalog.openings,
                        "Openings",
                        search.as_deref(),
                        list,
                        download.as_deref(),
                        all,
                        &output_dir,
                        keep_zip,
                    )
                    .await?;
                }
                PgnMentorSubcommand::Events {
                    search,
                    list,
                    download,
                    all,
                    output_dir,
                    keep_zip,
                } => {
                    sources::pgnmentor::handle_category_action(
                        &client,
                        &catalog.events,
                        "Events",
                        search.as_deref(),
                        list,
                        download.as_deref(),
                        all,
                        &output_dir,
                        keep_zip,
                    )
                    .await?;
                }
            }
        }

        Some(Commands::ChessResults(args)) => match args.command {
            ChessResultsSubcommand::Tournaments {
                query,
                fed,
                city,
                since,
                until,
                has_games,
                finished_only,
                limit,
            } => {
                sources::chessresults::search_tournaments(
                    &client,
                    query.as_deref(),
                    fed.as_deref(),
                    city.as_deref(),
                    since.as_deref().and_then(parse_date),
                    until.as_deref().and_then(parse_date),
                    has_games,
                    finished_only,
                    limit,
                )
                .await?;
            }
            ChessResultsSubcommand::Players {
                last_name,
                first_name,
                fide_id,
                fed,
                min_elo,
                limit,
            } => {
                sources::chessresults::search_players(
                    &client,
                    &last_name,
                    first_name.as_deref(),
                    fide_id.as_deref(),
                    fed.as_deref(),
                    min_elo.as_deref(),
                    limit,
                )
                .await?;
            }
            ChessResultsSubcommand::Games {
                player,
                first_name,
                fide_id,
                tournament_id,
                tournament_name,
                color,
                result,
                round_from,
                round_to,
                limit,
            } => {
                sources::chessresults::search_games(
                    &client,
                    player.as_deref(),
                    first_name.as_deref(),
                    fide_id.as_deref(),
                    tournament_id.as_deref(),
                    tournament_name.as_deref(),
                    color.as_deref(),
                    result.as_deref(),
                    round_from,
                    round_to,
                    limit,
                )
                .await?;
            }
            ChessResultsSubcommand::Download {
                tournament_id,
                player,
                first_name,
                fide_id,
                tournament_name,
                color,
                result,
                round_from,
                round_to,
                output,
                split_files,
            } => {
                sources::chessresults::download_games(
                    &client,
                    player.as_deref(),
                    tournament_id.as_deref(),
                    first_name.as_deref(),
                    fide_id.as_deref(),
                    tournament_name.as_deref(),
                    color.as_deref(),
                    result.as_deref(),
                    round_from,
                    round_to,
                    output.as_deref(),
                    split_files,
                )
                .await?;
            }
        },

        Some(Commands::Interactive) => {
            interactive::run_interactive_wizard(&client).await?;
        }

        Some(Commands::Completions(args)) => {
            let mut cmd = Cli::command();
            let name = cmd.get_name().to_string();
            match args.shell {
                ShellType::Bash => generate(
                    clap_complete::shells::Bash,
                    &mut cmd,
                    name,
                    &mut io::stdout(),
                ),
                ShellType::Elvish => generate(
                    clap_complete::shells::Elvish,
                    &mut cmd,
                    name,
                    &mut io::stdout(),
                ),
                ShellType::Fish => generate(
                    clap_complete::shells::Fish,
                    &mut cmd,
                    name,
                    &mut io::stdout(),
                ),
                ShellType::Powershell => generate(
                    clap_complete::shells::PowerShell,
                    &mut cmd,
                    name,
                    &mut io::stdout(),
                ),
                ShellType::Zsh => generate(
                    clap_complete::shells::Zsh,
                    &mut cmd,
                    name,
                    &mut io::stdout(),
                ),
            }
        }

        None => {
            // If no subcommand given, print top-level help
            let mut cmd = Cli::command();
            cmd.print_help()?;
            println!(
                "\n\n{} Tip: Run with '{}' to start the interactive download wizard!",
                "💡".yellow(),
                "chess-fetch interactive".cyan().bold()
            );
        }
    }

    Ok(())
}

fn parse_date(s: &str) -> Option<chrono::NaiveDate> {
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
