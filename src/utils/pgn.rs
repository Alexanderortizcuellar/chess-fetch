use chrono::NaiveDate;
use std::collections::HashMap;
use std::path::Path;

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct PgnGame {
    pub tags: HashMap<String, String>,
    pub movetext: String,
    pub raw: String,
}

#[allow(dead_code)]
impl PgnGame {
    pub fn get_tag(&self, key: &str) -> Option<&str> {
        self.tags.get(key).map(|s| s.as_str())
    }

    pub fn white(&self) -> &str {
        self.tags
            .get("White")
            .map(|s| s.as_str())
            .unwrap_or("Unknown")
    }

    pub fn black(&self) -> &str {
        self.tags
            .get("Black")
            .map(|s| s.as_str())
            .unwrap_or("Unknown")
    }

    pub fn result(&self) -> &str {
        self.tags.get("Result").map(|s| s.as_str()).unwrap_or("*")
    }

    pub fn date_str(&self) -> &str {
        self.tags
            .get("Date")
            .or_else(|| self.tags.get("UTCDate"))
            .map(|s| s.as_str())
            .unwrap_or("????.??.??")
    }

    pub fn parse_date(&self) -> Option<NaiveDate> {
        let d = self.date_str();
        let cleaned = d.replace(['/', '-'], ".");
        let parts: Vec<&str> = cleaned.split('.').collect();
        if parts.len() == 3 {
            let year = parts[0].parse::<i32>().ok()?;
            let month = parts[1].parse::<u32>().ok()?;
            let day = parts[2].parse::<u32>().ok()?;
            NaiveDate::from_ymd_opt(year, month, day)
        } else {
            None
        }
    }

    pub fn default_filename(&self, index: usize) -> String {
        let white = crate::utils::fs::sanitize_filename(self.white());
        let black = crate::utils::fs::sanitize_filename(self.black());
        let date = crate::utils::fs::sanitize_filename(self.date_str());
        format!("{:04}_{}_{}_vs_{}.pgn", index, date, white, black)
    }
}

pub fn split_pgn_games(pgn_content: &str) -> Vec<PgnGame> {
    let mut games = Vec::new();
    let mut current_tags = HashMap::new();
    let mut current_moves = Vec::new();
    let mut current_raw = String::new();
    let mut in_moves = false;

    for line in pgn_content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if in_moves && (!current_tags.is_empty() || !current_moves.is_empty()) {
                // Game finished, start new one
                games.push(PgnGame {
                    tags: std::mem::take(&mut current_tags),
                    movetext: current_moves.join("\n").trim().to_string(),
                    raw: current_raw.trim().to_string(),
                });
                current_moves.clear();
                current_raw.clear();
                in_moves = false;
            }

            current_raw.push_str(line);
            current_raw.push('\n');

            // Parse tag e.g. [White "Carlsen, Magnus"]
            if let Some(tag_content) = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                if let Some(space_idx) = tag_content.find(' ') {
                    let key = tag_content[..space_idx].trim().to_string();
                    let val = tag_content[space_idx + 1..].trim();
                    let val = val.trim_matches('"').to_string();
                    current_tags.insert(key, val);
                }
            }
        } else if !trimmed.is_empty() {
            in_moves = true;
            current_raw.push_str(line);
            current_raw.push('\n');
            current_moves.push(line);
        } else {
            // Empty line
            if in_moves {
                current_raw.push('\n');
            }
        }
    }

    if !current_tags.is_empty() || !current_moves.is_empty() || !current_raw.trim().is_empty() {
        games.push(PgnGame {
            tags: current_tags,
            movetext: current_moves.join("\n").trim().to_string(),
            raw: current_raw.trim().to_string(),
        });
    }

    games
}

pub fn count_pgn_games(pgn_content: &str) -> usize {
    let mut count = 0;
    let mut in_tags = false;

    for line in pgn_content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("[Event ") || (trimmed.starts_with('[') && !in_tags) {
            if trimmed.starts_with("[Event ") {
                count += 1;
            }
            in_tags = true;
        } else if !trimmed.starts_with('[') && !trimmed.is_empty() {
            in_tags = false;
        }
    }

    if count == 0 && !pgn_content.trim().is_empty() {
        // Fallback: split
        split_pgn_games(pgn_content).len()
    } else {
        count
    }
}

#[allow(dead_code)]
pub fn filter_games(
    games: Vec<PgnGame>,
    player_username: Option<&str>,
    since: Option<NaiveDate>,
    until: Option<NaiveDate>,
    result_filter: Option<&str>,
    color_filter: Option<&str>,
    opponent_filter: Option<&str>,
) -> Vec<PgnGame> {
    games
        .into_iter()
        .filter(|g| {
            // Date filter
            if let Some(game_date) = g.parse_date() {
                if let Some(s) = since {
                    if game_date < s {
                        return false;
                    }
                }
                if let Some(u) = until {
                    if game_date > u {
                        return false;
                    }
                }
            }

            // Opponent filter
            if let Some(opp) = opponent_filter {
                let opp_lower = opp.to_lowercase();
                let w_lower = g.white().to_lowercase();
                let b_lower = g.black().to_lowercase();
                if !w_lower.contains(&opp_lower) && !b_lower.contains(&opp_lower) {
                    return false;
                }
            }

            // Color filter for specific player
            if let Some(target_user) = player_username {
                let user_lower = target_user.to_lowercase();
                let is_white = g.white().to_lowercase() == user_lower;
                let is_black = g.black().to_lowercase() == user_lower;

                if let Some(col) = color_filter {
                    match col.to_lowercase().as_str() {
                        "white" | "w" if !is_white => return false,
                        "black" | "b" if !is_black => return false,
                        _ => {}
                    }
                }

                // Result filter: "win", "loss", "draw"
                if let Some(res) = result_filter {
                    let r = g.result();
                    let outcome = match r {
                        "1-0" => {
                            if is_white {
                                "win"
                            } else {
                                "loss"
                            }
                        }
                        "0-1" => {
                            if is_black {
                                "win"
                            } else {
                                "loss"
                            }
                        }
                        "1/2-1/2" => "draw",
                        _ => "other",
                    };
                    if outcome != res.to_lowercase() {
                        return false;
                    }
                }
            } else if let Some(res) = result_filter {
                let r = g.result();
                let matched = match res.to_lowercase().as_str() {
                    "white" | "1-0" => r == "1-0",
                    "black" | "0-1" => r == "0-1",
                    "draw" | "1/2-1/2" => r == "1/2-1/2",
                    _ => true,
                };
                if !matched {
                    return false;
                }
            }

            true
        })
        .collect()
}

pub fn save_split_games(games: &[PgnGame], output_dir: &Path) -> anyhow::Result<usize> {
    std::fs::create_dir_all(output_dir)?;
    for (i, game) in games.iter().enumerate() {
        let filename = game.default_filename(i + 1);
        let path = output_dir.join(filename);
        crate::utils::fs::save_text_to_file(&path, &game.raw)?;
    }
    Ok(games.len())
}
