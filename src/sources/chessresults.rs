use anyhow::{Context, Result};
use chrono::NaiveDate;
use colored::Colorize;
use reqwest::Client;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const BASE_URL: &str = "https://chess-results.com";

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TournamentInfo {
    pub id: String,
    pub name: String,
    pub fed: String,
    pub start_date: String,
    pub end_date: String,
    pub city: String,
    pub rounds: String,
    pub players: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PlayerTournamentInfo {
    pub player_name: String,
    pub elo: String,
    pub club: String,
    pub fed: String,
    pub tournament_name: String,
    pub date: String,
    pub tournament_id: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct GameInfo {
    pub white: String,
    pub black: String,
    pub result: String,
    pub dbkey: String,
    pub round: String,
    pub tournament: String,
    pub moves: String,
}

pub fn parse_tnr_id(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.contains("tnr") {
        if let Some(pos) = trimmed.find("tnr") {
            let after = &trimmed[pos + 3..];
            let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
            if !digits.is_empty() {
                return digits;
            }
        }
    }
    trimmed
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>()
}

fn extract_aspnet_tokens(html: &str) -> Result<(String, String, String)> {
    let vs = extract_input_value(html, "__VIEWSTATE")
        .context("Could not find __VIEWSTATE token on chess-results.com")?;
    let vs_gen = extract_input_value(html, "__VIEWSTATEGENERATOR").unwrap_or_default();
    let ev = extract_input_value(html, "__EVENTVALIDATION")
        .context("Could not find __EVENTVALIDATION token on chess-results.com")?;
    Ok((vs, vs_gen, ev))
}

fn extract_input_value(html: &str, name: &str) -> Option<String> {
    let pattern = format!(r#"name="{}"[^>]*value="([^"]*)""#, name);
    let re = regex::Regex::new(&pattern).ok()?;
    if let Some(caps) = re.captures(html) {
        return Some(caps[1].to_string());
    }
    // Try alternate attribute order: value then name
    let pattern2 = format!(r#"value="([^"]*)"[^>]*name="{}""#, name);
    let re2 = regex::Regex::new(&pattern2).ok()?;
    re2.captures(html).map(|caps| caps[1].to_string())
}

#[allow(clippy::too_many_arguments)]
pub async fn search_tournaments(
    client: &Client,
    query: Option<&str>,
    fed: Option<&str>,
    city: Option<&str>,
    since: Option<NaiveDate>,
    until: Option<NaiveDate>,
    has_games: bool,
    finished_only: bool,
    limit: Option<usize>,
) -> Result<Vec<TournamentInfo>> {
    let spinner = crate::utils::http::create_spinner("Connecting to Chess-Results server...");

    let get_url = format!("{}/TurnierSuche.aspx?lan=1", BASE_URL);
    let get_res = client
        .get(&get_url)
        .send()
        .await
        .context("Failed to connect to chess-results.com")?;

    let cluster_url = get_res.url().clone();
    let html = get_res.text().await?;
    let (vs, vs_gen, ev) = extract_aspnet_tokens(&html)?;

    spinner.set_message("Searching tournaments on Chess-Results...");

    let mut form = HashMap::new();
    form.insert("__LASTFOCUS", "".to_string());
    form.insert("__EVENTTARGET", "".to_string());
    form.insert("__EVENTARGUMENT", "".to_string());
    form.insert("__VIEWSTATE", vs);
    form.insert("__VIEWSTATEGENERATOR", vs_gen);
    form.insert("__EVENTVALIDATION", ev);

    form.insert("ctl00$P1$txt_leiter", "".to_string());
    form.insert("ctl00$P1$txt_tnr", "".to_string());
    form.insert("ctl00$P1$txt_bez", query.unwrap_or("").to_string());
    form.insert("ctl00$P1$txt_veranstalter", "".to_string());
    form.insert("ctl00$P1$txt_Hauptschiedsrichter", "".to_string());
    form.insert("ctl00$P1$txt_Schiedsrichter", "".to_string());
    form.insert("ctl00$P1$txt_ort", city.unwrap_or("").to_string());

    if let Some(d) = since {
        form.insert("ctl00$P1$txt_von_tag", d.format("%d.%m.%Y").to_string());
    } else {
        form.insert("ctl00$P1$txt_von_tag", "".to_string());
    }

    if let Some(d) = until {
        form.insert("ctl00$P1$txt_bis_tag", d.format("%d.%m.%Y").to_string());
    } else {
        form.insert("ctl00$P1$txt_bis_tag", "".to_string());
    }

    form.insert("ctl00$P1$txt_eventid", "".to_string());
    form.insert("ctl00$P1$combo_art", "5".to_string()); // All tournaments
    form.insert("ctl00$P1$combo_sort", "1".to_string()); // Last update

    let fed_val = fed
        .map(|f| f.trim().to_uppercase())
        .unwrap_or_else(|| "-".to_string());
    form.insert("ctl00$P1$combo_land", fed_val);

    form.insert("ctl00$P1$combo_bedenkzeit", "0".to_string());

    // 0: 100 lines, 1: 250 lines, 2: 500 lines
    let rows_code = match limit {
        Some(n) if n > 250 => "2",
        Some(n) if n > 100 => "1",
        _ => "0",
    };
    form.insert("ctl00$P1$combo_anzahl_zeilen", rows_code.to_string());

    if finished_only {
        form.insert("ctl00$P1$cbox_zuEnde", "on".to_string());
    }
    if has_games {
        form.insert("ctl00$P1$cbox_partien_vorhanden", "on".to_string());
    }

    form.insert("ctl00$P1$cb_suchen", "Search".to_string());

    let post_res = client
        .post(cluster_url)
        .form(&form)
        .send()
        .await
        .context("Failed to post search query to chess-results.com")?;

    let result_html = post_res.text().await?;
    spinner.finish_and_clear();

    let tournaments = parse_tournament_search_results(&result_html);

    if tournaments.is_empty() {
        println!(
            "{} No tournaments found matching your criteria on Chess-Results.",
            "!".yellow().bold()
        );
        return Ok(tournaments);
    }

    let display_count = limit.unwrap_or(25).min(tournaments.len());
    println!(
        "{} Found {} tournaments on Chess-Results (showing {}):",
        "✓".green().bold(),
        tournaments.len().to_string().cyan().bold(),
        display_count
    );
    println!(
        "{:<10} {:<5} {:<50} {:<12} {:<12} {:<5} {:<6}",
        "TNR ID".bold(),
        "FED".bold(),
        "Tournament Name".bold(),
        "Start Date".bold(),
        "End Date".bold(),
        "Rnds".bold(),
        "Plyrs".bold()
    );
    println!("{}", "-".repeat(105));

    for t in tournaments.iter().take(display_count) {
        let short_name = if t.name.chars().count() > 48 {
            format!("{}...", t.name.chars().take(45).collect::<String>())
        } else {
            t.name.clone()
        };

        println!(
            "{:<10} {:<5} {:<50} {:<12} {:<12} {:<5} {:<6}",
            t.id.cyan().bold(),
            t.fed.yellow(),
            short_name,
            t.start_date.dimmed(),
            t.end_date.dimmed(),
            t.rounds,
            t.players
        );
    }

    Ok(tournaments)
}

fn parse_tournament_search_results(html: &str) -> Vec<TournamentInfo> {
    let mut results = Vec::new();
    let re_row = regex::Regex::new(r"(?s)<tr[^>]*>(.*?)</tr>").unwrap();
    let re_tnr = regex::Regex::new(r"tnr(\d+)\.aspx").unwrap();
    let re_clean = regex::Regex::new(r"<[^>]+>").unwrap();
    let re_cell = regex::Regex::new(r"(?s)<td[^>]*>(.*?)</td>").unwrap();

    for cap in re_row.captures_iter(html) {
        let row_content = &cap[1];
        if let Some(tnr_cap) = re_tnr.captures(row_content) {
            let id = tnr_cap[1].to_string();

            // Split into cells
            let cells: Vec<String> = re_cell
                .captures_iter(row_content)
                .map(|c| {
                    let text = re_clean.replace_all(&c[1], " ").to_string();
                    text.split_whitespace().collect::<Vec<_>>().join(" ")
                })
                .collect();

            // Typical cell structure in search table:
            // 0: No
            // 1: Tournament Name
            // 2: Fed
            // 3: Flag / icon
            // 4: Duration or last update
            // 5: Start Date (YYYY/MM/DD)
            // 6: End Date (YYYY/MM/DD)
            // ...
            if cells.len() >= 7 {
                let name = decode_html_entities(&cells.get(1).cloned().unwrap_or_default());
                let fed = cells.get(2).cloned().unwrap_or_default();
                let start_date = cells.get(5).cloned().unwrap_or_default();
                let end_date = cells.get(6).cloned().unwrap_or_default();
                let city = decode_html_entities(&cells.get(11).cloned().unwrap_or_default());
                let rounds = cells.get(15).cloned().unwrap_or_default();
                let players = cells.get(16).cloned().unwrap_or_default();

                if !name.is_empty() {
                    results.push(TournamentInfo {
                        id,
                        name,
                        fed,
                        start_date,
                        end_date,
                        city,
                        rounds,
                        players,
                    });
                }
            }
        }
    }

    results
}

pub async fn search_players(
    client: &Client,
    last_name: &str,
    first_name: Option<&str>,
    fide_id: Option<&str>,
    fed: Option<&str>,
    min_elo: Option<&str>,
    limit: Option<usize>,
) -> Result<Vec<PlayerTournamentInfo>> {
    let spinner = crate::utils::http::create_spinner("Connecting to Chess-Results server...");

    let get_url = format!("{}/SpielerSuche.aspx?lan=1", BASE_URL);
    let get_res = client
        .get(&get_url)
        .send()
        .await
        .context("Failed to connect to chess-results.com")?;

    let cluster_url = get_res.url().clone();
    let html = get_res.text().await?;
    let (vs, vs_gen, ev) = extract_aspnet_tokens(&html)?;

    spinner.set_message("Searching players on Chess-Results...");

    let mut form = HashMap::new();
    form.insert("__LASTFOCUS", "".to_string());
    form.insert("__EVENTTARGET", "".to_string());
    form.insert("__EVENTARGUMENT", "".to_string());
    form.insert("__VIEWSTATE", vs);
    form.insert("__VIEWSTATEGENERATOR", vs_gen);
    form.insert("__EVENTVALIDATION", ev);

    form.insert("ctl00$P1$txt_nachname", last_name.to_string());
    form.insert("ctl00$P1$txt_vorname", first_name.unwrap_or("").to_string());
    form.insert("ctl00$P1$txt_verein", "".to_string());
    form.insert("ctl00$P1$txt_ident", "".to_string());
    form.insert("ctl00$P1$txt_fideID", fide_id.unwrap_or("").to_string());
    form.insert("ctl00$P1$txt_von_tag", "".to_string());
    form.insert("ctl00$P1$txt_bis_tag", "".to_string());
    form.insert("ctl00$P1$txt_GJahr", "".to_string());
    form.insert("ctl00$P1$txt_min_elo", min_elo.unwrap_or("").to_string());
    form.insert("ctl00$P1$txt_FED", fed.unwrap_or("").to_string());
    form.insert("ctl00$P1$txt_Fed_tur", "".to_string());
    form.insert("ctl00$P1$combo_Sort", "0".to_string());
    form.insert("ctl00$P1$combo_anzahl_zeilen", "1".to_string()); // 250
    form.insert("ctl00$P1$cb_suchen", "Search".to_string());

    let post_res = client
        .post(cluster_url)
        .form(&form)
        .send()
        .await
        .context("Failed to post player search to chess-results.com")?;

    let result_html = post_res.text().await?;
    spinner.finish_and_clear();

    let players = parse_player_search_results(&result_html);

    if players.is_empty() {
        println!(
            "{} No player entries found for '{}' on Chess-Results.",
            "!".yellow().bold(),
            last_name
        );
        return Ok(players);
    }

    let display_count = limit.unwrap_or(25).min(players.len());
    println!(
        "{} Found {} player participations on Chess-Results (showing {}):",
        "✓".green().bold(),
        players.len().to_string().cyan().bold(),
        display_count
    );
    println!(
        "{:<30} {:<6} {:<5} {:<10} {:<12} {}",
        "Player Name".bold(),
        "Elo".bold(),
        "FED".bold(),
        "TNR ID".bold(),
        "Date".bold(),
        "Tournament".bold()
    );
    println!("{}", "-".repeat(105));

    for p in players.iter().take(display_count) {
        let short_tour = if p.tournament_name.chars().count() > 40 {
            format!(
                "{}...",
                p.tournament_name.chars().take(37).collect::<String>()
            )
        } else {
            p.tournament_name.clone()
        };

        println!(
            "{:<30} {:<6} {:<5} {:<10} {:<12} {}",
            p.player_name.bold(),
            p.elo.yellow(),
            p.fed,
            p.tournament_id.cyan(),
            p.date.dimmed(),
            short_tour
        );
    }

    Ok(players)
}

fn decode_html_entities(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&#189;", "½")
        .replace("&frac12;", "½")
        .replace("&#189;-&#189;", "1/2-1/2")
        .replace("&frac12;-&frac12;", "1/2-1/2")
}

fn parse_player_search_results(html: &str) -> Vec<PlayerTournamentInfo> {
    let mut results = Vec::new();
    let re_row = regex::Regex::new(r"(?s)<tr[^>]*>(.*?)</tr>").unwrap();
    let re_tnr = regex::Regex::new(r"tnr(\d+)\.aspx").unwrap();
    let re_clean = regex::Regex::new(r"<[^>]+>").unwrap();
    let re_cell = regex::Regex::new(r"(?s)<td[^>]*>(.*?)</td>").unwrap();

    for cap in re_row.captures_iter(html) {
        let row_content = &cap[1];
        if let Some(tnr_cap) = re_tnr.captures(row_content) {
            let tournament_id = tnr_cap[1].to_string();

            let cells: Vec<String> = re_cell
                .captures_iter(row_content)
                .map(|c| {
                    let text = re_clean.replace_all(&c[1], " ").to_string();
                    text.split_whitespace().collect::<Vec<_>>().join(" ")
                })
                .collect();

            // Columns in player search:
            // 0: Player Name
            // 1: NatElo
            // 2: FideElo
            // 3: Club
            // 4: FED
            // 5: Tournament Name
            // 6: Date (YYYY/MM/DD)
            if cells.len() >= 7 {
                let player_name = decode_html_entities(&cells[0]);
                let elo = if cells[2] != "0" && !cells[2].is_empty() {
                    cells[2].clone()
                } else {
                    cells[1].clone()
                };
                let club = decode_html_entities(&cells[3]);
                let fed = cells[4].clone();
                let tournament_name = decode_html_entities(&cells[5]);
                let date = cells[6].clone();

                if !player_name.is_empty() {
                    results.push(PlayerTournamentInfo {
                        player_name,
                        elo,
                        club,
                        fed,
                        tournament_name,
                        date,
                        tournament_id,
                    });
                }
            }
        }
    }

    results
}

#[allow(clippy::too_many_arguments)]
pub async fn search_games(
    client: &Client,
    last_name: Option<&str>,
    first_name: Option<&str>,
    fide_id: Option<&str>,
    tournament_id: Option<&str>,
    tournament_name: Option<&str>,
    color: Option<&str>,
    result: Option<&str>,
    round_from: Option<u32>,
    round_to: Option<u32>,
    limit: Option<usize>,
) -> Result<Vec<GameInfo>> {
    let spinner = crate::utils::http::create_spinner("Connecting to Chess-Results server...");

    let get_url = format!("{}/PartieSuche.aspx?lan=1", BASE_URL);
    let get_res = client
        .get(&get_url)
        .send()
        .await
        .context("Failed to connect to chess-results.com")?;

    let cluster_url = get_res.url().clone();
    let html = get_res.text().await?;
    let (vs, vs_gen, ev) = extract_aspnet_tokens(&html)?;

    spinner.set_message("Searching games on Chess-Results...");

    let clean_tnr = tournament_id.map(parse_tnr_id);

    let mut form = HashMap::new();
    form.insert("__LASTFOCUS", "".to_string());
    form.insert("__EVENTTARGET", "".to_string());
    form.insert("__EVENTARGUMENT", "".to_string());
    form.insert("__VIEWSTATE", vs);
    form.insert("__VIEWSTATEGENERATOR", vs_gen);
    form.insert("__EVENTVALIDATION", ev);

    form.insert("ctl00$P1$txt_nachname", last_name.unwrap_or("").to_string());
    form.insert("ctl00$P1$txt_vorname", first_name.unwrap_or("").to_string());
    form.insert("ctl00$P1$Txt_FideID", fide_id.unwrap_or("").to_string());
    form.insert("ctl00$P1$Txt_NatID", "".to_string());
    form.insert(
        "ctl00$P1$txt_dbkey",
        clean_tnr.as_deref().unwrap_or("").to_string(),
    );
    form.insert(
        "ctl00$P1$txt_bez",
        tournament_name.unwrap_or("").to_string(),
    );
    form.insert("ctl00$P1$txt_von_tag", "".to_string());
    form.insert("ctl00$P1$txt_bis_tag", "".to_string());
    form.insert(
        "ctl00$P1$txt_rdvon",
        round_from.map(|r| r.to_string()).unwrap_or_default(),
    );
    form.insert(
        "ctl00$P1$txt_rdbis",
        round_to.map(|r| r.to_string()).unwrap_or_default(),
    );

    let color_val = match color.map(|c| c.to_lowercase()).as_deref() {
        Some("white") | Some("w") => "w",
        Some("black") | Some("b") => "s",
        _ => "-",
    };
    form.insert("ctl00$P1$combo_spielerfarbe", color_val.to_string());

    let res_val = match result {
        Some("1-0") | Some("win") => "1-0",
        Some("0-1") | Some("loss") => "0-1",
        Some("1/2-1/2") | Some("draw") => "1/2-1/2",
        _ => "-",
    };
    form.insert("ctl00$P1$combo_ergebnis", res_val.to_string());
    form.insert("ctl00$P1$combo_anzahl_zeilen", "1".to_string()); // 250
    form.insert("ctl00$P1$cb_SuchenPartie", "Search".to_string());

    let post_res = client
        .post(cluster_url)
        .form(&form)
        .send()
        .await
        .context("Failed to post game search to chess-results.com")?;

    let result_html = post_res.text().await?;
    spinner.finish_and_clear();

    let games = parse_games_grid(&result_html);

    if games.is_empty() {
        println!(
            "{} No games found matching your search criteria on Chess-Results.",
            "!".yellow().bold()
        );
        return Ok(games);
    }

    let display_count = limit.unwrap_or(20).min(games.len());
    println!(
        "{} Found {} games on Chess-Results (showing {}):",
        "✓".green().bold(),
        games.len().to_string().cyan().bold(),
        display_count
    );
    println!(
        "{:<35} {:<8} {:<10} {:<4} {}",
        "Pairing (White - Black)".bold(),
        "Result".bold(),
        "TNR ID".bold(),
        "Rd".bold(),
        "Tournament".bold()
    );
    println!("{}", "-".repeat(105));

    for g in games.iter().take(display_count) {
        let pairing = format!("{} - {}", g.white, g.black);
        let short_pairing = if pairing.chars().count() > 33 {
            format!("{}...", pairing.chars().take(30).collect::<String>())
        } else {
            pairing
        };

        let short_tour = if g.tournament.chars().count() > 40 {
            format!("{}...", g.tournament.chars().take(37).collect::<String>())
        } else {
            g.tournament.clone()
        };

        println!(
            "{:<35} {:<8} {:<10} {:<4} {}",
            short_pairing,
            g.result.yellow().bold(),
            g.dbkey.cyan(),
            g.round,
            short_tour.dimmed()
        );
    }

    Ok(games)
}

fn parse_games_grid(html: &str) -> Vec<GameInfo> {
    let mut results = Vec::new();

    // Check if table with class "Grid" exists
    let grid_re =
        regex::Regex::new(r#"(?s)<table[^>]+class=["']Grid["'][^>]*>(.*?)</table>"#).unwrap();
    let grid_html = match grid_re.captures(html) {
        Some(c) => c[1].to_string(),
        None => return results,
    };

    let re_row = regex::Regex::new(r"(?s)<tr[^>]*>(.*?)</tr>").unwrap();
    let re_clean = regex::Regex::new(r"<[^>]+>").unwrap();
    let re_cell = regex::Regex::new(r"(?s)<td[^>]*>(.*?)</td>").unwrap();

    for cap in re_row.captures_iter(&grid_html) {
        let row_content = &cap[1];
        let cells: Vec<String> = re_cell
            .captures_iter(row_content)
            .map(|c| {
                let text = re_clean.replace_all(&c[1], " ").to_string();
                text.split_whitespace().collect::<Vec<_>>().join(" ")
            })
            .collect();

        // Columns in Grid:
        // 0: Game icon / Show
        // 1: Board Pairings (e.g. "GM Sadhwani, Raunak (2576) - GM Carlsen, Magnus (2832)")
        // 2: Result (e.g. "1-0", "0-1", "1/2-1/2")
        // 3: dbkey (tnr ID)
        // 4: Rd (round number)
        // 5: Tournament Name
        // 6: PGN / first moves
        if cells.len() >= 6 {
            let pairing_text = cells.get(1).cloned().unwrap_or_default();
            let result_str = cells
                .get(2)
                .cloned()
                .unwrap_or_default()
                .replace("&#189;-&#189;", "1/2-1/2")
                .replace("&frac12;-&frac12;", "1/2-1/2");
            let dbkey = cells.get(3).cloned().unwrap_or_default();
            let round = cells.get(4).cloned().unwrap_or_default();
            let tournament = cells.get(5).cloned().unwrap_or_default();
            let moves = cells.get(6).cloned().unwrap_or_default();

            let mut white = pairing_text.clone();
            let mut black = String::new();
            if let Some(pos) = pairing_text.find(" - ") {
                white = pairing_text[..pos].trim().to_string();
                black = pairing_text[pos + 3..].trim().to_string();
            }

            if !white.is_empty() {
                results.push(GameInfo {
                    white,
                    black,
                    result: result_str,
                    dbkey,
                    round,
                    tournament,
                    moves,
                });
            }
        }
    }

    results
}

#[allow(clippy::too_many_arguments)]
pub async fn download_games(
    client: &Client,
    last_name: Option<&str>,
    tournament_id: Option<&str>,
    first_name: Option<&str>,
    fide_id: Option<&str>,
    tournament_name: Option<&str>,
    color: Option<&str>,
    result: Option<&str>,
    round_from: Option<u32>,
    round_to: Option<u32>,
    output: Option<&Path>,
    split_files: bool,
) -> Result<PathBuf> {
    if last_name.is_none()
        && tournament_id.is_none()
        && fide_id.is_none()
        && tournament_name.is_none()
    {
        anyhow::bail!(
            "Please provide at least one search filter: --player, --id / --tnr, --tournament, or --fide-id"
        );
    }

    let spinner = crate::utils::http::create_spinner("Connecting to Chess-Results server...");

    let get_url = format!("{}/PartieSuche.aspx?lan=1", BASE_URL);
    let get_res = client
        .get(&get_url)
        .send()
        .await
        .context("Failed to connect to chess-results.com")?;

    let cluster_url = get_res.url().clone();
    let html = get_res.text().await?;
    let (vs, vs_gen, ev) = extract_aspnet_tokens(&html)?;

    spinner.set_message("Searching games for download on Chess-Results...");

    let clean_tnr = tournament_id.map(parse_tnr_id);

    // Step 1: POST search to populate session
    let mut form1 = HashMap::new();
    form1.insert("__LASTFOCUS", "".to_string());
    form1.insert("__EVENTTARGET", "".to_string());
    form1.insert("__EVENTARGUMENT", "".to_string());
    form1.insert("__VIEWSTATE", vs);
    form1.insert("__VIEWSTATEGENERATOR", vs_gen);
    form1.insert("__EVENTVALIDATION", ev);

    form1.insert("ctl00$P1$txt_nachname", last_name.unwrap_or("").to_string());
    form1.insert("ctl00$P1$txt_vorname", first_name.unwrap_or("").to_string());
    form1.insert("ctl00$P1$Txt_FideID", fide_id.unwrap_or("").to_string());
    form1.insert("ctl00$P1$Txt_NatID", "".to_string());
    form1.insert(
        "ctl00$P1$txt_dbkey",
        clean_tnr.as_deref().unwrap_or("").to_string(),
    );
    form1.insert(
        "ctl00$P1$txt_bez",
        tournament_name.unwrap_or("").to_string(),
    );
    form1.insert("ctl00$P1$txt_von_tag", "".to_string());
    form1.insert("ctl00$P1$txt_bis_tag", "".to_string());
    form1.insert(
        "ctl00$P1$txt_rdvon",
        round_from.map(|r| r.to_string()).unwrap_or_default(),
    );
    form1.insert(
        "ctl00$P1$txt_rdbis",
        round_to.map(|r| r.to_string()).unwrap_or_default(),
    );

    let color_val = match color.map(|c| c.to_lowercase()).as_deref() {
        Some("white") | Some("w") => "w",
        Some("black") | Some("b") => "s",
        _ => "-",
    };
    form1.insert("ctl00$P1$combo_spielerfarbe", color_val.to_string());

    let res_val = match result {
        Some("1-0") | Some("win") => "1-0",
        Some("0-1") | Some("loss") => "0-1",
        Some("1/2-1/2") | Some("draw") => "1/2-1/2",
        _ => "-",
    };
    form1.insert("ctl00$P1$combo_ergebnis", res_val.to_string());
    form1.insert("ctl00$P1$combo_anzahl_zeilen", "1".to_string()); // 250
    form1.insert("ctl00$P1$cb_SuchenPartie", "Search".to_string());

    let post_res1 = client
        .post(cluster_url.clone())
        .form(&form1)
        .send()
        .await
        .context("Failed to search games on chess-results.com")?;

    let result_html1 = post_res1.text().await?;

    // Step 2: Extract new tokens from search results and download PGN
    let (vs2, vs_gen2, ev2) = extract_aspnet_tokens(&result_html1)
        .context("Could not extract updated session tokens after game search")?;

    spinner.set_message("Downloading PGN archive from Chess-Results...");

    let mut form2 = form1;
    form2.insert("__VIEWSTATE", vs2);
    form2.insert("__VIEWSTATEGENERATOR", vs_gen2);
    form2.insert("__EVENTVALIDATION", ev2);
    form2.remove("ctl00$P1$cb_SuchenPartie");
    form2.insert(
        "ctl00$P1$cb_DownLoadPGN",
        "Download as PGN-File".to_string(),
    );

    let post_res2 = client
        .post(cluster_url)
        .form(&form2)
        .send()
        .await
        .context("Failed to download PGN from chess-results.com")?;

    if !post_res2.status().is_success() {
        spinner.finish_and_clear();
        anyhow::bail!(
            "Failed to download PGN: HTTP {} from Chess-Results",
            post_res2.status()
        );
    }

    let pgn_bytes = post_res2.bytes().await?;
    let pgn_text = String::from_utf8_lossy(&pgn_bytes).to_string();
    spinner.finish_and_clear();

    if pgn_text.trim().is_empty() {
        anyhow::bail!(
            "Chess-Results returned an empty PGN file (no games were found for this query)."
        );
    }

    let games = crate::utils::pgn::split_pgn_games(&pgn_text);
    let count = games.len();

    let default_name = if let Some(tnr) = &clean_tnr {
        format!("chess_results_tnr_{}.pgn", tnr)
    } else if let Some(player) = last_name {
        format!("chess_results_player_{}.pgn", player)
    } else {
        "chess_results_games.pgn".to_string()
    };

    let out_path = if split_files {
        let dir_name = default_name.trim_end_matches(".pgn");
        let dir = output
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(dir_name));
        crate::utils::pgn::save_split_games(&games, &dir)?;
        println!(
            "{} Successfully saved {} Chess-Results games into directory '{}'",
            "✓".green().bold(),
            count.to_string().cyan().bold(),
            dir.display().to_string().cyan()
        );
        dir
    } else {
        let out_file = output
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(default_name));
        crate::utils::fs::save_text_to_file(&out_file, &pgn_text)?;
        println!(
            "{} Successfully downloaded {} Chess-Results games to '{}'",
            "✓".green().bold(),
            count.to_string().cyan().bold(),
            out_file.display().to_string().cyan()
        );
        out_file
    };

    Ok(out_path)
}
