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
   - **Live & finished broadcast exports**: download complete broadcast tours or specific rounds by ID or Lichess URL
   - **Broadcast listing & search**: list official broadcasts with `--query`, `--finished-only`, `--since`, and `--until` filters

3. **Chess-Results.com (Scraper, Search & PGN Exporter)**
   - **Tournament search**: search tournaments across the world by name, federation (e.g. `COL`, `GER`, `ESP`, `USA`), city, and date range
   - **Player search**: find player tournament participation history by name, FIDE ID, federation, or Elo rating
   - **Game search**: query individual pairings, results, round numbers, and tournaments
   - **PGN download**: download complete tournament PGN files or player game archives directly by TNR ID, URL, or player name

4. **Chessgames.com (Scraper & Exporter)**
   - Download single game by GID (e.g. `1000000` or page URL)
   - Download full game collections by CID (e.g. `1000001` or collection URL)
   - Download/scrape player games by PID with multi-page support
   - Search players, games, and collections via `--query`

5. **The Week in Chess (TWIC)**
   - Download specific issues by number (e.g. `--issue 1663`)
   - Download issue ranges (e.g. `--range 1650..1663`)
   - Auto-fetch the `--latest` issue
   - Auto-fetch the last N issues (`--count 5`)
   - Optional `--combine` to merge multiple issues into a single master PGN
   - Automatic extraction of `.zip` into `.pgn`

6. **PGN Mentor / PGN Master**
   - Search, list, and download historical & modern grandmaster player collections (Carlsen, Kasparov, Fischer, Capablanca, etc.)
   - Search, list, and download opening collections (Sicilian, French, Caro-Kann, King's Indian, Ruy Lopez, etc.)
   - Search, list, and download major tournament events (Candidates, Olympiads, World Championships)
   - Direct automatic decompression of zip archives into clean PGNs

7. **Interactive Wizard Mode**
   - Interactive CLI prompt (`chess-fetch interactive`) with menus, search, and guided filtering.

8. **Shell Completions**
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

# List available broadcasts (filtering for finished/played ones)
chess-fetch lichess broadcast --list --finished-only
chess-fetch lichess broadcast --list --query "Tata Steel" --finished-only

# Download all games from an entire broadcast tournament (by Tour ID or any Round URL)
# Passing any round URL will automatically resolve the parent tournament and download all rounds!
chess-fetch lichess broadcast --id T9eMnL3X -o tata_steel.pgn
chess-fetch lichess broadcast --id https://lichess.org/broadcast/xxxiii-irt-internacional-feria-de-las-flores-2026-internacional/round-1/u2sDgTxa -o feria_flores.pgn

# Download ONLY a specific round from a round URL or round ID
chess-fetch lichess broadcast --id https://lichess.org/broadcast/candidates-2024/round-14/T9eMnL3X --round-only -o round14.pgn
chess-fetch lichess broadcast --id njUPbMHT -o round_games.pgn
```

### 3. Chess-Results.com

```bash
# Search tournaments by name, federation, city, or date
chess-fetch chess-results tournaments --query "Feria de las Flores" --limit 10
chess-fetch chess-results tournaments --fed COL --city Medellin
chess-fetch chess-results tournaments --query "World Rapid" --has-games

# Search player tournament history by name, FIDE ID, or federation
chess-fetch chess-results players --last-name Carlsen --first-name Magnus --limit 10
chess-fetch chess-results players --last-name Nakamura --fed USA

# Search specific pairings / games
chess-fetch chess-results games --player Carlsen --limit 10
chess-fetch chess-results games --tournament-id 1435703 --round-from 1 --round-to 3

# Download complete tournament PGN files by TNR ID or URL
chess-fetch chess-results download --tournament-id 1435703 -o world_team_rapid.pgn
chess-fetch chess-results download --tournament-id https://chess-results.com/tnr1435703.aspx -o world_team_rapid.pgn

# Download player games from Chess-Results
chess-fetch chess-results download --player Carlsen -o carlsen_chess_results.pgn

# Save games into individual files in a directory
chess-fetch chess-results download -t 1435703 --split-files -o ./tournament_games
```

### 4. Chessgames.com

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

### 5. The Week in Chess (TWIC)

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

### 6. PGN Mentor / PGN Master

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

### 7. Interactive Wizard

```bash
# Launch interactive step-by-step menu
chess-fetch interactive
```

### 8. Shell Completions

```bash
# PowerShell
chess-fetch completions powershell | Out-String | Invoke-Expression

# Bash
chess-fetch completions bash > ~/.local/share/bash-completion/completions/chess-fetch

# Zsh
chess-fetch completions zsh > ~/.zfunc/_chess-fetch
```
