# ♞ chess-fetch

A blazing-fast, feature-rich command-line interface (CLI) written in Rust to query, search, filter, and download chess games and PGN archives across all major chess platforms and databases.

---

## 🚀 Supported Sources

1. **Chess.com (Official REST API)**
   - Download user games with date range filters (`--since`, `--until`, `--year`, `--month`)
   - Speed/time class filtering (`bullet`, `blitz`, `rapid`, `daily`)
   - Outcome filtering (`win`, `loss`, `draw`), color (`white`, `black`), rated/unrated, opponent (`--vs`)
   - Split downloaded games into individual PGN files or combine into one
   - Inspect monthly archive URLs
   - Download single games by ID or URL

2. **Lichess.org (Official API)**
   - Download user games with date range (`--since`, `--until`), limit (`--max`), and speed/variant (`bullet`, `blitz`, `rapid`, `classical`, `chess960`, etc.)
   - Optional inclusions: clock times (`--clocks`), computer evals (`--evals`), opening names & ECO (`--opening`)
   - Opponent filtering (`--vs`), color filtering (`--color`), rated/casual
   - Single game export by ID/URL
   - Tournament games export by tournament ID/URL

3. **Chessgames.com (Scraper & Exporter)**
   - Download single game by GID (e.g. `1000000` or page URL)
   - Download full game collections by CID (e.g. `1000001` or collection URL)
   - Download/scrape player games by PID with multi-page support
   - Search players, games, and collections via `--query`

4. **The Week in Chess (TWIC)**
   - Download specific issues by number (e.g. `--issue 1663`)
   - Download issue ranges (e.g. `--range 1650..1663`)
   - Auto-fetch the `--latest` issue
   - Auto-fetch the last N issues (`--count 5`)
   - Optional `--combine` to merge multiple issues into a single master PGN
   - Automatic extraction of `.zip` into `.pgn`

5. **PGN Mentor / PGN Master**
   - Search, list, and download historical & modern grandmaster player collections (Carlsen, Kasparov, Fischer, Capablanca, etc.)
   - Search, list, and download opening collections (Sicilian, French, Caro-Kann, King's Indian, Ruy Lopez, etc.)
   - Search, list, and download major tournament events (Candidates, Olympiads, World Championships)
   - Direct automatic decompression of zip archives into clean PGNs

6. **Interactive Wizard Mode**
   - Interactive CLI prompt (`chess-fetch interactive`) with menus, search, and guided filtering.

7. **Shell Completions**
   - Generate completion scripts for `bash`, `zsh`, `fish`, `powershell`, `elvish`.

---

## 📦 Installation & Build

```bash
# Build optimized release binary
cargo build --release

# Binary will be located at:
# ./target/release/chess-fetch (or chess-fetch.exe on Windows)
```

---

## 📖 Usage & Examples

### 1. Chess.com

```bash
# Download blitz games from January 2024 for a player
chess-fetch chess-com download --username hikaru --year 2024 --month 1 --time-class blitz --output hikaru_jan2024.pgn

# Download games in a specific date range with limit
chess-fetch chess-com download -u hikaru --since 2023-01-01 --until 2023-06-30 --limit 50 -o hikaru_2023.pgn

# Save each game into separate PGN files in a folder
chess-fetch chess-com download -u magnuscarlsen --year 2024 --month 1 --split-files -o ./carlsen_games

# List available archive months for a user
chess-fetch chess-com archives --username magnuscarlsen

# Download a single game by ID or URL
chess-fetch chess-com game --id 123456789 -o my_game.pgn
```

### 2. Lichess.org

```bash
# Download recent games for a player
chess-fetch lichess download --username thibault --max 20 --perf-type blitz -o thibault_blitz.pgn

# Download with date filters and include engine evaluations & clocks
chess-fetch lichess download -u thibault --since 2024-01-01 --evals --clocks --opening -o thibault_evals.pgn

# Download a single game by ID or URL
chess-fetch lichess game --id 1IB3wwbL -o lichess_game.pgn

# Download all games from a tournament
chess-fetch lichess tournament --id AbCdEfGh -o tournament.pgn
```

### 3. Chessgames.com

```bash
# Search for games, players, or collections
chess-fetch chessgames search --query "Morphy"

# Download a single game by GID or URL
chess-fetch chessgames game --id 1000000 -o morphy_game.pgn

# Download an entire collection
chess-fetch chessgames collection --id 1000001 -o collection.pgn

# Scrape and download games of a player by PID
chess-fetch chessgames player --id 10240 --max-pages 5 -o player_games.pgn
```

### 4. The Week in Chess (TWIC)

```bash
# List recent TWIC issues
chess-fetch twic list --count 10

# Download the latest available issue
chess-fetch twic download --latest --output-dir ./twic_games

# Download a specific issue
chess-fetch twic download --issue 1663 -o ./twic_games

# Download an issue range and combine into a single PGN
chess-fetch twic download --range 1650..1663 --combine master_database.pgn
```

### 5. PGN Mentor / PGN Master

```bash
# Search and list openings
chess-fetch pgn-mentor openings --search Sicilian --list

# Download an opening collection
chess-fetch pgn-mentor openings --download SicilianDragon --output-dir ./openings

# Search and download player collections
chess-fetch pgn-mentor players --search Carlsen --list
chess-fetch pgn-mentor players --download Capablanca --output-dir ./players

# Search and download event tournaments
chess-fetch pgn-mentor events --search Candidates --list
chess-fetch pgn-mentor events --download candidates2024 --output-dir ./events
```

### 6. Interactive Wizard

```bash
# Launch interactive step-by-step menu
chess-fetch interactive
```

### 7. Shell Completions

```bash
# PowerShell
chess-fetch completions powershell | Out-String | Invoke-Expression

# Bash
chess-fetch completions bash > ~/.local/share/bash-completion/completions/chess-fetch

# Zsh
chess-fetch completions zsh > ~/.zfunc/_chess-fetch
```
