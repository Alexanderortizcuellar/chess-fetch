use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "chess-fetch",
    author = "Chess Fetch CLI Developer",
    version,
    about = "A high-performance CLI to query and download chess games and PGNs from online databases (Chess.com, Lichess, Chessgames.com, The Week in Chess, PGN Mentor)",
    long_about = "Chess-Fetch is an all-in-one command-line tool to query, filter, and download PGN collections and games from top online chess databases and websites:
- Chess.com (via official REST API with date ranges, time classes, ratings, results)
- Lichess.org (via official API with filters, evaluations, clocks, tournaments)
- Chessgames.com (game, player, collection scraper & search)
- The Week in Chess (TWIC weekly master game archives by issue or range)
- PGN Mentor / PGN Master (players, openings, and events libraries)
- Interactive CLI wizard mode"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[arg(short, long, global = true, help = "Enable verbose logging")]
    pub verbose: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(
        name = "chess-com",
        alias = "chesscom",
        about = "Query and download games from Chess.com API"
    )]
    ChessCom(ChessComArgs),

    #[command(
        name = "lichess",
        about = "Query and download games from Lichess.org API"
    )]
    Lichess(LichessArgs),

    #[command(
        name = "chessgames",
        about = "Scrape and download games, collections, and player databases from Chessgames.com"
    )]
    Chessgames(ChessgamesArgs),

    #[command(
        name = "twic",
        alias = "theweekinchess",
        about = "Download weekly master tournament PGNs from The Week in Chess (TWIC)"
    )]
    Twic(TwicArgs),

    #[command(
        name = "pgn-mentor",
        aliases = ["pgnmentor", "pgn-master", "pgnmaster"],
        about = "Search and download opening, player, and event PGN archives from PGN Mentor"
    )]
    PgnMentor(PgnMentorArgs),

    #[command(
        name = "interactive",
        alias = "wizard",
        about = "Launch an interactive prompt wizard to search and download games"
    )]
    Interactive,

    #[command(name = "completions", about = "Generate shell completion scripts")]
    Completions(CompletionsArgs),
}

// ---------------- Chess.com ----------------
#[derive(Args, Debug)]
pub struct ChessComArgs {
    #[command(subcommand)]
    pub command: ChessComSubcommand,
}

#[derive(Subcommand, Debug)]
pub enum ChessComSubcommand {
    #[command(about = "Download games for a Chess.com player")]
    Download {
        #[arg(short, long, help = "Chess.com username", required = true)]
        username: String,

        #[arg(
            short,
            long,
            help = "Filter games played on or after date (YYYY-MM-DD or YYYY-MM)"
        )]
        since: Option<String>,

        #[arg(
            short = 'U',
            long,
            help = "Filter games played on or before date (YYYY-MM-DD or YYYY-MM)"
        )]
        until: Option<String>,

        #[arg(short, long, help = "Filter games from a specific year (e.g. 2024)")]
        year: Option<i32>,

        #[arg(short, long, help = "Filter games from a specific month (1-12)")]
        month: Option<u32>,

        #[arg(short, long, help = "Time class filter: bullet, blitz, rapid, daily")]
        time_class: Option<String>,

        #[arg(
            long,
            help = "Only download rated games",
            conflicts_with = "unrated_only"
        )]
        rated_only: bool,

        #[arg(
            long,
            help = "Only download unrated/casual games",
            conflicts_with = "rated_only"
        )]
        unrated_only: bool,

        #[arg(short, long, help = "Filter by player color: white, black")]
        color: Option<String>,

        #[arg(short, long, help = "Filter by player outcome: win, loss, draw")]
        result: Option<String>,

        #[arg(long, help = "Filter by opponent username")]
        vs: Option<String>,

        #[arg(short, long, help = "Maximum number of games to download")]
        limit: Option<usize>,

        #[arg(short, long, help = "Output PGN file or directory")]
        output: Option<PathBuf>,

        #[arg(
            long,
            help = "Save each game into a separate PGN file inside output directory"
        )]
        split_files: bool,

        #[arg(
            long,
            help = "Fast mode: fetch raw monthly PGNs directly without per-game filtering"
        )]
        pgn_bulk: bool,
    },

    #[command(about = "List available monthly archives for a user")]
    Archives {
        #[arg(short, long, help = "Chess.com username", required = true)]
        username: String,
    },

    #[command(about = "Download a single Chess.com game by ID or URL")]
    Game {
        #[arg(
            short,
            long,
            help = "Game ID or URL (e.g., 123456789 or https://www.chess.com/game/live/123456789)"
        )]
        id: String,

        #[arg(short, long, help = "Output PGN file path")]
        output: Option<PathBuf>,
    },
}

// ---------------- Lichess ----------------
#[derive(Args, Debug)]
pub struct LichessArgs {
    #[command(subcommand)]
    pub command: LichessSubcommand,
}

#[derive(Subcommand, Debug)]
pub enum LichessSubcommand {
    #[command(about = "Download games for a Lichess user")]
    Download {
        #[arg(short, long, help = "Lichess username", required = true)]
        username: String,

        #[arg(short, long, help = "Filter games played since date (YYYY-MM-DD)")]
        since: Option<String>,

        #[arg(
            short = 'U',
            long,
            help = "Filter games played until date (YYYY-MM-DD)"
        )]
        until: Option<String>,

        #[arg(short = 'n', long, help = "Maximum number of games to download")]
        max: Option<usize>,

        #[arg(
            short = 't',
            long,
            help = "Variant/speed: ultraBullet, bullet, blitz, rapid, classical, correspondence, chess960, crazyhouse, etc."
        )]
        perf_type: Option<String>,

        #[arg(
            long,
            help = "Only download rated games",
            conflicts_with = "casual_only"
        )]
        rated_only: bool,

        #[arg(
            long,
            help = "Only download casual games",
            conflicts_with = "rated_only"
        )]
        casual_only: bool,

        #[arg(short, long, help = "Filter by player color: white, black")]
        color: Option<String>,

        #[arg(long, default_value_t = true, action = clap::ArgAction::Set, help = "Include clock times in PGN")]
        clocks: bool,

        #[arg(long, default_value_t = true, action = clap::ArgAction::Set, help = "Include engine evaluations if available")]
        evals: bool,

        #[arg(long, default_value_t = true, action = clap::ArgAction::Set, help = "Include opening name and ECO tag in PGN")]
        opening: bool,

        #[arg(long, help = "Filter by opponent username")]
        vs: Option<String>,

        #[arg(short, long, help = "Output PGN file or directory")]
        output: Option<PathBuf>,

        #[arg(
            long,
            help = "Save each game into a separate PGN file inside output directory"
        )]
        split_files: bool,
    },

    #[command(about = "Download a single Lichess game by ID or URL")]
    Game {
        #[arg(
            short,
            long,
            help = "Game ID or URL (e.g., q7ZvsdUF or https://lichess.org/q7ZvsdUF)"
        )]
        id: String,

        #[arg(long, default_value_t = true, action = clap::ArgAction::Set, help = "Include clock times")]
        clocks: bool,

        #[arg(long, default_value_t = true, action = clap::ArgAction::Set, help = "Include engine evaluations")]
        evals: bool,

        #[arg(long, default_value_t = true, action = clap::ArgAction::Set, help = "Include opening name and ECO")]
        opening: bool,

        #[arg(short, long, help = "Output PGN file path")]
        output: Option<PathBuf>,
    },

    #[command(about = "Download all games from a Lichess tournament")]
    Tournament {
        #[arg(short, long, help = "Tournament ID or URL")]
        id: String,

        #[arg(short, long, help = "Output PGN file path")]
        output: Option<PathBuf>,
    },
}

// ---------------- Chessgames.com ----------------
#[derive(Args, Debug)]
pub struct ChessgamesArgs {
    #[command(subcommand)]
    pub command: ChessgamesSubcommand,
}

#[derive(Subcommand, Debug)]
pub enum ChessgamesSubcommand {
    #[command(about = "Download a single game from Chessgames.com by GID or URL")]
    Game {
        #[arg(short, long, help = "Chessgames Game ID (gid) or URL (e.g. 1234567)")]
        id: String,

        #[arg(short, long, help = "Output PGN file path")]
        output: Option<PathBuf>,
    },

    #[command(about = "Download a game collection from Chessgames.com by CID or URL")]
    Collection {
        #[arg(
            short,
            long,
            help = "Chessgames Collection ID (cid) or URL (e.g. 1000001)"
        )]
        id: String,

        #[arg(short, long, help = "Output PGN file path")]
        output: Option<PathBuf>,
    },

    #[command(about = "Scrape and download games for a player from Chessgames.com by PID")]
    Player {
        #[arg(short, long, help = "Chessgames Player ID (pid) or URL (e.g. 10240)")]
        id: String,

        #[arg(
            short = 'p',
            long,
            default_value_t = 10,
            help = "Maximum pages of games to scrape"
        )]
        max_pages: usize,

        #[arg(short, long, help = "Output PGN file path")]
        output: Option<PathBuf>,
    },

    #[command(about = "Search Chessgames.com for players, games, and collections")]
    Search {
        #[arg(short, long, help = "Search query string")]
        query: String,
    },
}

// ---------------- The Week in Chess (TWIC) ----------------
#[derive(Args, Debug)]
pub struct TwicArgs {
    #[command(subcommand)]
    pub command: TwicSubcommand,
}

#[derive(Subcommand, Debug)]
pub enum TwicSubcommand {
    #[command(about = "Download TWIC issue(s) in PGN format")]
    Download {
        #[arg(short, long, help = "Specific issue number (e.g. 1550)")]
        issue: Option<u32>,

        #[arg(short, long, help = "Issue range to download (e.g. 1540..1550)")]
        range: Option<String>,

        #[arg(long, help = "Download the latest available issue")]
        latest: bool,

        #[arg(short = 'n', long, help = "Download the latest N issues")]
        count: Option<u32>,

        #[arg(
            short,
            long,
            default_value = ".",
            help = "Output directory for downloaded PGN files"
        )]
        output_dir: PathBuf,

        #[arg(
            long,
            help = "Combine multiple downloaded issues into a single PGN file"
        )]
        combine: Option<PathBuf>,

        #[arg(long, help = "Keep the downloaded .zip files")]
        keep_zip: bool,
    },

    #[command(about = "List recent issues available on The Week in Chess")]
    List {
        #[arg(
            short = 'n',
            long,
            default_value_t = 25,
            help = "Number of recent issues to list"
        )]
        count: usize,
    },
}

// ---------------- PGN Mentor / PGN Master ----------------
#[derive(Args, Debug)]
pub struct PgnMentorArgs {
    #[command(subcommand)]
    pub command: PgnMentorSubcommand,
}

#[derive(Subcommand, Debug)]
pub enum PgnMentorSubcommand {
    #[command(about = "Search, list, and download historical and modern grandmaster collections")]
    Players {
        #[arg(
            short,
            long,
            help = "Search player name (e.g. Carlsen, Kasparov, Fischer)"
        )]
        search: Option<String>,

        #[arg(short, long, help = "List all available player collections")]
        list: bool,

        #[arg(short, long, help = "Download specific player collection by name")]
        download: Option<String>,

        #[arg(long, help = "Download all matching/available player collections")]
        all: bool,

        #[arg(
            short,
            long,
            default_value = ".",
            help = "Output directory for PGN files"
        )]
        output_dir: PathBuf,

        #[arg(long, help = "Keep downloaded .zip archive")]
        keep_zip: bool,
    },

    #[command(
        about = "Search, list, and download opening collections (Sicilian, French, King's Indian, etc.)"
    )]
    Openings {
        #[arg(
            short,
            long,
            help = "Search opening name (e.g. Sicilian, French, Caro-Kann, Ruy Lopez)"
        )]
        search: Option<String>,

        #[arg(short, long, help = "List all available opening collections")]
        list: bool,

        #[arg(short, long, help = "Download specific opening collection by name")]
        download: Option<String>,

        #[arg(long, help = "Download all matching/available opening collections")]
        all: bool,

        #[arg(
            short,
            long,
            default_value = ".",
            help = "Output directory for PGN files"
        )]
        output_dir: PathBuf,

        #[arg(long, help = "Keep downloaded .zip archive")]
        keep_zip: bool,
    },

    #[command(
        about = "Search, list, and download event tournament collections (Candidates, Olympiads, World Championships)"
    )]
    Events {
        #[arg(
            short,
            long,
            help = "Search event name (e.g. Candidates, Wijk aan Zee, World Championship)"
        )]
        search: Option<String>,

        #[arg(short, long, help = "List all available event collections")]
        list: bool,

        #[arg(short, long, help = "Download specific event collection by name")]
        download: Option<String>,

        #[arg(long, help = "Download all matching/available event collections")]
        all: bool,

        #[arg(
            short,
            long,
            default_value = ".",
            help = "Output directory for PGN files"
        )]
        output_dir: PathBuf,

        #[arg(long, help = "Keep downloaded .zip archive")]
        keep_zip: bool,
    },
}

// ---------------- Completions ----------------
#[derive(Args, Debug)]
pub struct CompletionsArgs {
    #[arg(value_enum, help = "Shell to generate completions for")]
    pub shell: ShellType,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum ShellType {
    Bash,
    Elvish,
    Fish,
    Powershell,
    Zsh,
}
