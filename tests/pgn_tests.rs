#[cfg(test)]
mod tests {
    use chess_fetch::utils::fs::sanitize_filename;
    use chess_fetch::utils::pgn::{count_pgn_games, filter_games, split_pgn_games};
    use chrono::NaiveDate;

    const SAMPLE_PGN: &str = r#"
[Event "FIDE World Championship 2021"]
[Site "Dubai UAE"]
[Date "2021.11.26"]
[Round "1"]
[White "Nepomniachtchi, Ian"]
[Black "Carlsen, Magnus"]
[Result "1/2-1/2"]
[WhiteElo "2782"]
[BlackElo "2855"]
[ECO "C88"]

1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7 6. Re1 b5 7. Bb3 O-O 8. h3 Bb7 9. d3 d6 10. a3 Na5 11. Ba2 c5 12. Nbd2 Nc6 13. Nf1 Bc8 14. Ne3 Be6 15. Nd5 Nd4 16. Ng5 Bxd5 17. exd5 h6 18. Ne4 Nxe4 19. dxe4 f5 20. c3 fxe4 21. cxd4 exd4 22. Rxe4 Bf6 23. Re6 c4 24. Bf4 Be5 25. Bxe5 dxe5 26. Rxe5 Qf6 27. Qxd4 Rad8 28. Rd1 Rd6 29. Bb1 g6 30. Qe3 h5 31. Re6 Rxe6 32. dxe6 Re8 33. Rd6 Qxb2 34. Bxg6 Qa1+ 35. Kh2 Qf6 36. Bf7+ Kf8 37. Bxe8 Kxe8 38. Rxa6 Qe5+ 39. Qxe5 Ke7 40. Qd6+ Kf6 41. e7+ Kf7 42. e8=Q+ Kxe8 43. Ra7 c3 44. Qe7# 1-0

[Event "Rated Blitz game"]
[Site "https://lichess.org/test1234"]
[Date "2024.01.15"]
[White "Carlsen, Magnus"]
[Black "Nakamura, Hikaru"]
[Result "1-0"]
[ECO "B06"]

1. e4 g6 2. d4 Bg7 3. Nc3 d6 4. f4 a6 5. Nf3 b5 6. Bd3 Bb7 7. e5 c5 8. dxc5 dxc5 9. Qe2 e6 10. Ne4 Bxe4 11. Bxe4 Ra7 12. Be3 Nd7 13. O-O-O Qc7 14. Rd6 Ne7 15. Rhd1 1-0
"#;

    #[test]
    fn test_split_and_count_pgn() {
        let games = split_pgn_games(SAMPLE_PGN);
        assert_eq!(games.len(), 2);
        assert_eq!(count_pgn_games(SAMPLE_PGN), 2);

        assert_eq!(games[0].white(), "Nepomniachtchi, Ian");
        assert_eq!(games[0].black(), "Carlsen, Magnus");
        assert_eq!(games[0].result(), "1/2-1/2");
        assert_eq!(games[0].parse_date(), NaiveDate::from_ymd_opt(2021, 11, 26));

        assert_eq!(games[1].white(), "Carlsen, Magnus");
        assert_eq!(games[1].black(), "Nakamura, Hikaru");
        assert_eq!(games[1].result(), "1-0");
        assert_eq!(games[1].parse_date(), NaiveDate::from_ymd_opt(2024, 1, 15));
    }

    #[test]
    fn test_filter_games_by_date() {
        let games = split_pgn_games(SAMPLE_PGN);
        let filtered = filter_games(
            games,
            None,
            Some(NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
            None,
            None,
            None,
            None,
        );
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].date_str(), "2024.01.15");
    }

    #[test]
    fn test_filter_games_by_opponent() {
        let games = split_pgn_games(SAMPLE_PGN);
        let filtered = filter_games(games, None, None, None, None, None, Some("Nakamura"));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].black(), "Nakamura, Hikaru");
    }

    #[test]
    fn test_filter_games_by_color_and_result() {
        let games = split_pgn_games(SAMPLE_PGN);
        let filtered = filter_games(
            games,
            Some("Carlsen, Magnus"),
            None,
            None,
            Some("win"),
            Some("white"),
            None,
        );
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].white(), "Carlsen, Magnus");
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(
            sanitize_filename("Carlsen, Magnus: 2024/01?"),
            "Carlsen, Magnus_ 2024_01_"
        );
        assert_eq!(sanitize_filename("Game <1> *test*"), "Game _1_ _test_");
    }
}
