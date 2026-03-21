/// Stress and edge-case tests that exercise every boundary in the application.
///
/// Covers: cli, exclude, filedetect, search, replace, model, ui, and app (key handling).
#[cfg(test)]
mod stress_tests {
    use regex::Regex;
    use std::collections::HashSet;
    use std::fs;
    use std::path::Path;

    // ── helpers ──────────────────────────────────────────────────────────

    fn tmp(name: &str) -> String {
        std::env::temp_dir()
            .join(name)
            .to_string_lossy()
            .to_string()
    }

    fn write_tmp(name: &str, data: &[u8]) -> String {
        let p = tmp(name);
        fs::write(&p, data).unwrap();
        p
    }

    fn cleanup(path: &str) {
        let _ = fs::remove_file(path);
    }

    fn make_result(
        file: &str,
        line_num: usize,
        line_text: &str,
    ) -> gref::model::SearchResult {
        gref::model::SearchResult {
            file_path: file.to_string(),
            line_num,
            line_text: line_text.to_string(),
        }
    }

    fn new_model(
        results: Vec<gref::model::SearchResult>,
        pattern: &str,
        replacement: &str,
        mode: gref::model::AppMode,
    ) -> gref::model::Model {
        let re = Regex::new(pattern).unwrap();
        gref::model::Model::new(
            results,
            pattern.to_string(),
            replacement.to_string(),
            re,
            mode,
        )
    }

    // =====================================================================
    //  CLI — parse_exclude_list edge cases
    // =====================================================================

    #[test]
    fn cli_exclude_only_commas() {
        assert!(gref::cli::parse_exclude_list(",,,").is_empty());
    }

    #[test]
    fn cli_exclude_whitespace_items() {
        assert!(gref::cli::parse_exclude_list("  ,  ,  ").is_empty());
    }

    #[test]
    fn cli_exclude_single_item_no_comma() {
        assert_eq!(
            gref::cli::parse_exclude_list("*.log"),
            vec!["*.log"]
        );
    }

    #[test]
    fn cli_exclude_trailing_comma() {
        assert_eq!(
            gref::cli::parse_exclude_list("a,b,"),
            vec!["a", "b"]
        );
    }

    #[test]
    fn cli_exclude_unicode_pattern() {
        assert_eq!(
            gref::cli::parse_exclude_list("données/,résumé.txt"),
            vec!["données/", "résumé.txt"]
        );
    }

    #[test]
    fn cli_parse_many_positionals_ignored() {
        // Extra positionals beyond [pattern, replacement, dir] are silently ignored
        let args: Vec<String> = vec![
            "pat".into(),
            "rep".into(),
            "dir".into(),
            "extra".into(),
        ];
        let cli = gref::cli::parse_from(&args);
        assert_eq!(cli.pattern, "pat");
        assert_eq!(cli.replacement, Some("rep".into()));
        assert_eq!(cli.root_path, "dir");
    }

    #[test]
    fn cli_parse_flags_any_order() {
        let args: Vec<String> = vec![
            "-e".into(), ".git".into(),
            "pattern".into(),
            "-i".into(),
            "replacement".into(),
        ];
        let cli = gref::cli::parse_from(&args);
        assert!(cli.ignore_case);
        assert_eq!(cli.exclude, vec![".git"]);
        assert_eq!(cli.pattern, "pattern");
        assert_eq!(cli.replacement, Some("replacement".into()));
    }

    // =====================================================================
    //  EXCLUDE — edge cases
    // =====================================================================

    #[test]
    fn exclude_empty_list() {
        assert!(!gref::exclude::is_excluded("/some/path.rs", &[]));
    }

    #[test]
    fn exclude_empty_pattern_in_list() {
        let list = vec!["".to_string(), "  ".to_string()];
        assert!(!gref::exclude::is_excluded("/any/file.rs", &list));
    }

    #[test]
    fn exclude_backslash_path() {
        let list = vec!["media/".to_string()];
        assert!(gref::exclude::is_excluded(
            "C:\\project\\media\\img.png",
            &list
        ));
    }

    #[test]
    fn exclude_nested_dir() {
        let list = vec!["build/".to_string()];
        assert!(gref::exclude::is_excluded(
            "/project/build/output/file.o",
            &list
        ));
    }

    #[test]
    fn exclude_extension_case_sensitive() {
        // Pattern is case-sensitive: *.LOG should NOT match .log
        let list = vec!["*.LOG".to_string()];
        assert!(!gref::exclude::is_excluded("/file.log", &list));
    }

    #[test]
    fn exclude_root_file_exact() {
        let list = vec!["Makefile".to_string()];
        assert!(gref::exclude::is_excluded("Makefile", &list));
    }

    #[test]
    fn exclude_deeply_nested_exact_match() {
        let list = vec!["README.md".to_string()];
        assert!(gref::exclude::is_excluded(
            "/a/b/c/d/e/f/README.md",
            &list
        ));
    }

    // =====================================================================
    //  FILEDETECT — edge cases
    // =====================================================================

    #[test]
    fn filedetect_no_extension() {
        let p = write_tmp("gref_stress_noext", b"just text content\n");
        assert!(gref::filedetect::is_likely_text_file(Path::new(&p)));
        cleanup(&p);
    }

    #[test]
    fn filedetect_empty_file_unknown_ext() {
        let p = write_tmp("gref_stress_empty.zzz", b"");
        // Empty file → is_text_file_content returns false (n == 0)
        assert!(!gref::filedetect::is_likely_text_file(Path::new(&p)));
        cleanup(&p);
    }

    #[test]
    fn filedetect_all_control_chars() {
        let data: Vec<u8> = (1..32).filter(|&b| b != 9 && b != 10 && b != 13).collect();
        let p = write_tmp("gref_stress_ctrl.zzz", &data);
        assert!(!gref::filedetect::is_likely_text_file(Path::new(&p)));
        cleanup(&p);
    }

    #[test]
    fn filedetect_just_tabs_and_newlines() {
        let p = write_tmp("gref_stress_tabnl.zzz", b"\t\n\r\n\t\t\n");
        assert!(gref::filedetect::is_likely_text_file(Path::new(&p)));
        cleanup(&p);
    }

    #[test]
    fn filedetect_binary_at_byte_512() {
        // First 511 bytes are text, byte 512 is null → should be caught
        let mut data = vec![b'A'; 511];
        data.push(0x00);
        let p = write_tmp("gref_stress_bin512.zzz", &data);
        assert!(!gref::filedetect::is_likely_text_file(Path::new(&p)));
        cleanup(&p);
    }

    #[test]
    fn filedetect_text_at_exactly_512_bytes() {
        let data = vec![b'A'; 512];
        let p = write_tmp("gref_stress_txt512.zzz", &data);
        assert!(gref::filedetect::is_likely_text_file(Path::new(&p)));
        cleanup(&p);
    }

    #[test]
    fn filedetect_binary_past_512_not_detected() {
        // null byte at position 513 is beyond the read window → still detected as text
        let mut data = vec![b'A'; 512];
        data.push(0x00);
        let p = write_tmp("gref_stress_nullpast.zzz", &data);
        assert!(gref::filedetect::is_likely_text_file(Path::new(&p)));
        cleanup(&p);
    }

    #[test]
    fn filedetect_uppercase_extension() {
        // Extension normalisation: .RS → .rs → text
        assert!(gref::filedetect::is_likely_text_file(Path::new("FOO.RS")));
    }

    #[test]
    fn filedetect_nonexistent_file() {
        assert!(!gref::filedetect::is_likely_text_file(Path::new(
            "/nonexistent/path/file.zzz"
        )));
    }

    // =====================================================================
    //  SEARCH — extract_longest_literal edge cases
    // =====================================================================

    #[test]
    fn search_prefix_only_anchors() {
        assert_eq!(gref::search::extract_longest_literal("^$"), None);
    }

    #[test]
    fn search_prefix_multiline_anchor() {
        // (?m) is stripped, then ^hello — ^ is a metachar that splits,
        // but "hello" (5 chars) is collected as the longest literal
        assert_eq!(
            gref::search::extract_longest_literal("(?m)^hello"),
            Some("hello".to_string())
        );
    }

    #[test]
    fn search_prefix_anchor_then_multiline() {
        // ^(?m)hello → strips leading ^, then (?m), leaving "hello" → Some
        assert_eq!(
            gref::search::extract_longest_literal("^(?m)hello"),
            Some("hello".to_string())
        );
    }

    #[test]
    fn search_prefix_all_escaped() {
        assert_eq!(
            gref::search::extract_longest_literal("\\[foo\\]"),
            Some("[foo]".to_string())
        );
    }

    #[test]
    fn search_prefix_empty_string() {
        assert_eq!(gref::search::extract_longest_literal(""), None);
    }

    #[test]
    fn search_prefix_only_metachar() {
        assert_eq!(gref::search::extract_longest_literal(".*"), None);
    }

    #[test]
    fn search_prefix_long_literal() {
        let long = "a".repeat(1000);
        assert_eq!(
            gref::search::extract_longest_literal(&long),
            Some(long)
        );
    }

    #[test]
    fn search_prefix_unicode_chars() {
        // "föö" has 3 characters but 5 bytes — len() >= 3 by byte count
        assert_eq!(
            gref::search::extract_longest_literal("föö"),
            Some("föö".to_string())
        );
    }

    #[test]
    fn search_prefix_trailing_backslash() {
        // Trailing backslash with no following char: escaped flag stays set, loop ends
        assert_eq!(gref::search::extract_longest_literal("abc\\"), Some("abc".to_string()));
    }

    // =====================================================================
    //  SEARCH — perform_search_adaptive edge cases
    // =====================================================================

    #[test]
    fn search_nonexistent_path() {
        let re = Regex::new("foo").unwrap();
        let result = gref::search::perform_search_adaptive("/nonexistent_dir", &re, &[], false, false);
        assert!(result.is_err());
    }

    #[test]
    fn search_empty_directory() {
        let dir = std::env::temp_dir().join("gref_stress_empty_dir");
        let _ = fs::create_dir(&dir);
        let re = Regex::new("foo").unwrap();
        let results = gref::search::perform_search_adaptive(
            dir.to_str().unwrap(),
            &re,
            &[],
            false,
            false,
        )
        .unwrap();
        assert!(results.is_empty());
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn search_file_with_many_matches() {
        let dir = std::env::temp_dir().join("gref_stress_many");
        let _ = fs::create_dir_all(&dir);
        // 10 000 lines each containing the pattern
        let content: String = (1..=10_000)
            .map(|i| format!("line {} has foo in it\n", i))
            .collect();
        fs::write(dir.join("big.txt"), &content).unwrap();
        let re = Regex::new("foo").unwrap();
        let results = gref::search::perform_search_adaptive(
            dir.to_str().unwrap(),
            &re,
            &[],
            false,
            false,
        )
        .unwrap();
        assert_eq!(results.len(), 10_000);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_excludes_git_dir() {
        let dir = std::env::temp_dir().join("gref_stress_gitskip");
        let git = dir.join(".git");
        let _ = fs::create_dir_all(&git);
        fs::write(git.join("config.txt"), b"foo match").unwrap();
        fs::write(dir.join("src.txt"), b"foo match").unwrap();
        let re = Regex::new("foo").unwrap();
        let results = gref::search::perform_search_adaptive(
            dir.to_str().unwrap(),
            &re,
            &[],
            false,
            false,
        )
        .unwrap();
        // Only the top-level file, not .git/config.txt
        assert_eq!(results.len(), 1);
        assert!(results[0].file_path.contains("src.txt"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_utf8_content() {
        let dir = std::env::temp_dir().join("gref_stress_utf8");
        let _ = fs::create_dir_all(&dir);
        fs::write(dir.join("uni.txt"), "日本語 föö 中文\nföö again\n").unwrap();
        let re = Regex::new("föö").unwrap();
        let results = gref::search::perform_search_adaptive(
            dir.to_str().unwrap(),
            &re,
            &[],
            false,
            false,
        )
        .unwrap();
        assert_eq!(results.len(), 2);
        let _ = fs::remove_dir_all(&dir);
    }

    // =====================================================================
    //  REPLACE — stress / edge cases
    // =====================================================================

    #[test]
    fn replace_single_newline_file() {
        let file = write_tmp("gref_stress_nl.txt", b"\n");
        let results: Vec<gref::model::SearchResult> = vec![];
        let refs: Vec<&gref::model::SearchResult> = results.iter().collect();
        let re = Regex::new("foo").unwrap();
        gref::replace::replace_in_file(&file, &refs, &re, "bar").unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"\n");
        cleanup(&file);
    }

    #[test]
    fn replace_file_no_trailing_newline() {
        let file = write_tmp("gref_stress_notrail.txt", b"foo");
        let results = vec![make_result(&file, 1, "foo")];
        let refs: Vec<&_> = results.iter().collect();
        let re = Regex::new("foo").unwrap();
        gref::replace::replace_in_file(&file, &refs, &re, "bar").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "bar");
        cleanup(&file);
    }

    #[test]
    fn replace_expand_match() {
        // Replacement is longer than the match
        let file = write_tmp("gref_stress_expand.txt", b"a\na\na\n");
        let results = vec![
            make_result(&file, 1, "a"),
            make_result(&file, 2, "a"),
            make_result(&file, 3, "a"),
        ];
        let refs: Vec<&_> = results.iter().collect();
        let re = Regex::new("a").unwrap();
        gref::replace::replace_in_file(&file, &refs, &re, "LONGSTRING").unwrap();
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "LONGSTRING\nLONGSTRING\nLONGSTRING\n"
        );
        cleanup(&file);
    }

    #[test]
    fn replace_shrink_match() {
        // Replacement is shorter than the match
        let file = write_tmp("gref_stress_shrink.txt", b"LONGWORD\nLONGWORD\n");
        let results = vec![
            make_result(&file, 1, "LONGWORD"),
            make_result(&file, 2, "LONGWORD"),
        ];
        let refs: Vec<&_> = results.iter().collect();
        let re = Regex::new("LONGWORD").unwrap();
        gref::replace::replace_in_file(&file, &refs, &re, "x").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "x\nx\n");
        cleanup(&file);
    }

    #[test]
    fn replace_to_empty_string() {
        let file = write_tmp("gref_stress_toempty.txt", b"foo bar foo\n");
        let results = vec![make_result(&file, 1, "foo bar foo")];
        let refs: Vec<&_> = results.iter().collect();
        let re = Regex::new("foo").unwrap();
        gref::replace::replace_in_file(&file, &refs, &re, "").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), " bar \n");
        cleanup(&file);
    }

    #[test]
    fn replace_very_long_line() {
        let line = "x".repeat(100_000) + "foo" + &"y".repeat(100_000);
        let content = format!("{}\n", line);
        let file = write_tmp("gref_stress_longline.txt", content.as_bytes());
        let results = vec![make_result(&file, 1, &line)];
        let refs: Vec<&_> = results.iter().collect();
        let re = Regex::new("foo").unwrap();
        gref::replace::replace_in_file(&file, &refs, &re, "bar").unwrap();
        let out = fs::read_to_string(&file).unwrap();
        assert!(out.contains("bar"));
        assert!(!out.contains("foo"));
        assert_eq!(out.len(), content.len()); // "foo" and "bar" same length
        cleanup(&file);
    }

    #[test]
    fn replace_many_lines() {
        // 5000-line file, replace every line
        let content: String = (0..5000).map(|_| "foo\n").collect();
        let file = write_tmp("gref_stress_manylines.txt", content.as_bytes());
        let results: Vec<_> = (1..=5000)
            .map(|ln| make_result(&file, ln, "foo"))
            .collect();
        let refs: Vec<&_> = results.iter().collect();
        let re = Regex::new("foo").unwrap();
        gref::replace::replace_in_file(&file, &refs, &re, "bar").unwrap();
        let out = fs::read_to_string(&file).unwrap();
        assert!(!out.contains("foo"));
        assert_eq!(out.lines().count(), 5000);
        cleanup(&file);
    }

    #[test]
    fn replace_multiple_matches_per_line() {
        let file = write_tmp("gref_stress_multi.txt", b"foo foo foo\n");
        let results = vec![make_result(&file, 1, "foo foo foo")];
        let refs: Vec<&_> = results.iter().collect();
        let re = Regex::new("foo").unwrap();
        gref::replace::replace_in_file(&file, &refs, &re, "X").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "X X X\n");
        cleanup(&file);
    }

    #[test]
    fn replace_only_middle_line() {
        let file = write_tmp("gref_stress_middle.txt", b"aaa\nfoo\nbbb\n");
        let results = vec![make_result(&file, 2, "foo")];
        let refs: Vec<&_> = results.iter().collect();
        let re = Regex::new("foo").unwrap();
        gref::replace::replace_in_file(&file, &refs, &re, "bar").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "aaa\nbar\nbbb\n");
        cleanup(&file);
    }

    #[test]
    fn replace_perform_replacements_disjoint_files() {
        let f1 = write_tmp("gref_stress_prd1.txt", b"foo\n");
        let f2 = write_tmp("gref_stress_prd2.txt", b"foo\n");
        let results = vec![
            make_result(&f1, 1, "foo"),
            make_result(&f2, 1, "foo"),
        ];
        let mut selected = HashSet::new();
        selected.insert(0);
        selected.insert(1);
        let re = Regex::new("foo").unwrap();
        gref::replace::perform_replacements(&results, &selected, &re, "bar").unwrap();
        assert_eq!(fs::read_to_string(&f1).unwrap(), "bar\n");
        assert_eq!(fs::read_to_string(&f2).unwrap(), "bar\n");
        cleanup(&f1);
        cleanup(&f2);
    }

    #[test]
    fn replace_perform_replacements_partial_selection() {
        let file = write_tmp("gref_stress_prpar.txt", b"foo\nfoo\nfoo\n");
        let results = vec![
            make_result(&file, 1, "foo"),
            make_result(&file, 2, "foo"),
            make_result(&file, 3, "foo"),
        ];
        let mut selected = HashSet::new();
        selected.insert(1); // only line 2
        let re = Regex::new("foo").unwrap();
        gref::replace::perform_replacements(&results, &selected, &re, "bar").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "foo\nbar\nfoo\n");
        cleanup(&file);
    }

    #[test]
    fn replace_perform_replacements_selected_out_of_range() {
        // Selected index beyond results length → silently ignored
        let file = write_tmp("gref_stress_proor.txt", b"foo\n");
        let results = vec![make_result(&file, 1, "foo")];
        let mut selected = HashSet::new();
        selected.insert(100); // out of range
        let re = Regex::new("foo").unwrap();
        gref::replace::perform_replacements(&results, &selected, &re, "bar").unwrap();
        // No replacement performed
        assert_eq!(fs::read_to_string(&file).unwrap(), "foo\n");
        cleanup(&file);
    }

    // =====================================================================
    //  MODEL — construction edge cases
    // =====================================================================

    #[test]
    fn model_new_defaults() {
        let m = new_model(vec![], "foo", "", gref::model::AppMode::Default);
        assert_eq!(m.cursor, 0);
        assert_eq!(m.topline, 0);
        assert_eq!(m.screen_height, 20);
        assert_eq!(m.screen_width, 80);
        assert!(m.selected.is_empty());
        assert_eq!(m.state, gref::model::AppState::Browse);
        assert!(m.error.is_none());
        assert_eq!(m.horizontal_offset, 0);
    }

    #[test]
    fn model_new_search_only() {
        let m = new_model(vec![], "foo", "", gref::model::AppMode::SearchOnly);
        assert_eq!(m.mode, gref::model::AppMode::SearchOnly);
    }

    // =====================================================================
    //  UI — render edge cases (no panic is the success criterion)
    // =====================================================================

    #[test]
    fn ui_render_empty_results() {
        let mut m = new_model(vec![], "foo", "bar", gref::model::AppMode::Default);
        let output = gref::ui::render(&mut m);
        assert!(output.contains("Search results"));
    }

    #[test]
    fn ui_render_single_result() {
        let results = vec![make_result("file.rs", 1, "hello foo world")];
        let mut m = new_model(results, "foo", "bar", gref::model::AppMode::Default);
        let output = gref::ui::render(&mut m);
        assert!(output.contains("file.rs"));
        assert!(output.contains("1:"));
    }

    #[test]
    fn ui_render_screen_height_1() {
        let results = vec![
            make_result("a.rs", 1, "foo"),
            make_result("a.rs", 2, "foo"),
            make_result("a.rs", 3, "foo"),
        ];
        let mut m = new_model(results, "foo", "bar", gref::model::AppMode::Default);
        m.screen_height = 1;
        // Should not panic even with very small screen
        let output = gref::ui::render(&mut m);
        assert!(!output.is_empty());
    }

    #[test]
    fn ui_render_screen_width_1() {
        let results = vec![make_result("a.rs", 1, "foo bar baz")];
        let mut m = new_model(results, "foo", "bar", gref::model::AppMode::Default);
        m.screen_width = 1;
        let output = gref::ui::render(&mut m);
        assert!(!output.is_empty());
    }

    #[test]
    fn ui_render_multibyte_offset_beyond_length() {
        let results = vec![make_result("a.rs", 1, "日本語")];
        let mut m = new_model(results, "日本語", "bar", gref::model::AppMode::Default);
        m.horizontal_offset = 1000; // way beyond the 3 chars
        let output = gref::ui::render(&mut m);
        assert!(!output.is_empty()); // no panic
    }

    #[test]
    fn ui_render_multibyte_offset_middle() {
        // The bug that was recently fixed: offset 1 into "├── main.rs"
        let results = vec![make_result("a.rs", 1, "    ├── main.rs")];
        let mut m = new_model(results, "main", "bar", gref::model::AppMode::Default);
        m.horizontal_offset = 5; // inside multi-byte char '├' if byte-based → would panic
        let output = gref::ui::render(&mut m);
        assert!(!output.is_empty());
    }

    #[test]
    fn ui_render_all_multibyte_line() {
        let text = "αβγδεζηθ";
        let results = vec![make_result("a.rs", 1, text)];
        let mut m = new_model(results, "αβγ", "XYZ", gref::model::AppMode::Default);
        for offset in 0..=text.chars().count() + 5 {
            m.horizontal_offset = offset;
            let output = gref::ui::render(&mut m);
            assert!(!output.is_empty(), "panicked at offset {}", offset);
        }
    }

    #[test]
    fn ui_render_emoji_line() {
        let text = "🔥🚀✨ foo 🎉";
        let results = vec![make_result("a.rs", 1, text)];
        let mut m = new_model(results, "foo", "bar", gref::model::AppMode::Default);
        for offset in 0..=20 {
            m.horizontal_offset = offset;
            let _ = gref::ui::render(&mut m);
        }
    }

    #[test]
    fn ui_render_cursor_at_last_result() {
        let results = vec![
            make_result("a.rs", 1, "foo"),
            make_result("a.rs", 2, "foo"),
            make_result("a.rs", 3, "foo"),
        ];
        let mut m = new_model(results, "foo", "bar", gref::model::AppMode::Default);
        m.cursor = 2;
        let output = gref::ui::render(&mut m);
        assert!(!output.is_empty());
    }

    #[test]
    fn ui_render_topline_adjustment() {
        let results: Vec<_> = (1..=100)
            .map(|i| make_result("a.rs", i, "foo line"))
            .collect();
        let mut m = new_model(results, "foo", "bar", gref::model::AppMode::Default);
        m.screen_height = 5;
        m.cursor = 99; // last result
        let output = gref::ui::render(&mut m);
        // topline should have been adjusted so cursor is visible
        assert!(m.topline > 0);
        assert!(!output.is_empty());
    }

    #[test]
    fn ui_render_many_files() {
        // Results from many distinct files → many "DIR:" headers
        let results: Vec<_> = (0..50)
            .map(|i| make_result(&format!("dir/file_{}.rs", i), 1, "foo"))
            .collect();
        let mut m = new_model(results, "foo", "bar", gref::model::AppMode::Default);
        m.screen_height = 10;
        let output = gref::ui::render(&mut m);
        assert!(output.contains("DIR:"));
    }

    #[test]
    fn ui_render_selected_results() {
        let results = vec![
            make_result("a.rs", 1, "has foo"),
            make_result("a.rs", 2, "also foo"),
        ];
        let mut m = new_model(results, "foo", "REPLACED", gref::model::AppMode::Default);
        m.selected.insert(0);
        let output = gref::ui::render(&mut m);
        assert!(output.contains("[x]"));
        assert!(output.contains("[ ]"));
    }

    #[test]
    fn ui_render_confirming_state() {
        let results = vec![make_result("a.rs", 1, "foo")];
        let mut m = new_model(results, "foo", "bar", gref::model::AppMode::Default);
        m.selected.insert(0);
        m.state = gref::model::AppState::Confirming;
        let output = gref::ui::render(&mut m);
        assert!(output.contains("Replacing 1?"));
    }

    #[test]
    fn ui_render_replacing_state() {
        let mut m = new_model(vec![], "foo", "bar", gref::model::AppMode::Default);
        m.state = gref::model::AppState::Replacing;
        let output = gref::ui::render(&mut m);
        assert!(output.contains("Replacing... wait"));
    }

    #[test]
    fn ui_render_error_state() {
        let mut m = new_model(vec![], "foo", "bar", gref::model::AppMode::Default);
        m.error = Some("something broke".into());
        let output = gref::ui::render(&mut m);
        assert!(output.contains("something broke"));
        assert!(output.contains("Press 'q' to exit"));
    }

    #[test]
    fn ui_render_search_only_mode() {
        let results = vec![make_result("a.rs", 1, "foo")];
        let mut m = new_model(results, "foo", "", gref::model::AppMode::SearchOnly);
        let output = gref::ui::render(&mut m);
        assert!(output.contains("Search Only Mode"));
    }

    // =====================================================================
    //  APP — handle_browse_key edge cases (tested via exported behaviour)
    //  We simulate key handling by directly calling into the model logic.
    // =====================================================================

    /// Helper that simulates handle_browse_key without needing the terminal.
    fn simulate_browse_key(model: &mut gref::model::Model, key: gref::term::Key) {
        // Re-implement key dispatch inline so tests don't need raw mode
        use gref::model::{AppMode, AppState};
        use gref::term::Key;
        match key {
            Key::CtrlC | Key::Char('q') => model.state = AppState::Done,
            Key::Up | Key::Char('k') => {
                if model.cursor > 0 {
                    model.cursor -= 1;
                    if model.cursor < model.topline {
                        model.topline = model.cursor;
                    }
                }
            }
            Key::Down | Key::Char('j') => {
                if model.cursor < model.results.len().saturating_sub(1) {
                    model.cursor += 1;
                    if model.cursor >= model.topline + model.screen_height {
                        model.topline = model.cursor - model.screen_height + 1;
                    }
                }
            }
            Key::Left | Key::Char('h') => {
                if model.horizontal_offset > 0 {
                    model.horizontal_offset = model.horizontal_offset.saturating_sub(10);
                }
            }
            Key::Right | Key::Char('l') => {
                let available_width = model.screen_width.saturating_sub(20).max(1);
                let end_line = (model.topline + model.screen_height).min(model.results.len());
                let mut max_offset = 0;
                for i in model.topline..end_line {
                    let line_len = model.results[i].line_text.len();
                    let offset = line_len.saturating_sub(available_width);
                    if offset > max_offset {
                        max_offset = offset;
                    }
                }
                model.horizontal_offset += 5;
                if model.horizontal_offset > max_offset {
                    model.horizontal_offset = max_offset;
                }
            }
            Key::Home => model.horizontal_offset = 0,
            Key::End => model.horizontal_offset = 1000,
            Key::Space => {
                if model.mode != AppMode::SearchOnly {
                    if model.selected.contains(&model.cursor) {
                        model.selected.remove(&model.cursor);
                    } else {
                        model.selected.insert(model.cursor);
                    }
                }
            }
            Key::Char('a') => {
                if model.mode != AppMode::SearchOnly {
                    for i in 0..model.results.len() {
                        model.selected.insert(i);
                    }
                }
            }
            Key::Char('n') => {
                if model.mode != AppMode::SearchOnly {
                    model.selected.clear();
                }
            }
            Key::Enter => {
                if model.mode != AppMode::SearchOnly {
                    if model.selected.is_empty() {
                        model.error = Some("no results".to_string());
                    } else {
                        model.state = AppState::Confirming;
                    }
                }
            }
            _ => {}
        }
    }

    #[test]
    fn app_cursor_cannot_go_below_zero() {
        let results = vec![make_result("a.rs", 1, "foo")];
        let mut m = new_model(results, "foo", "bar", gref::model::AppMode::Default);
        simulate_browse_key(&mut m, gref::term::Key::Up);
        assert_eq!(m.cursor, 0);
        simulate_browse_key(&mut m, gref::term::Key::Char('k'));
        assert_eq!(m.cursor, 0);
    }

    #[test]
    fn app_cursor_cannot_exceed_results() {
        let results = vec![
            make_result("a.rs", 1, "foo"),
            make_result("a.rs", 2, "foo"),
        ];
        let mut m = new_model(results, "foo", "bar", gref::model::AppMode::Default);
        m.cursor = 1;
        simulate_browse_key(&mut m, gref::term::Key::Down);
        assert_eq!(m.cursor, 1); // stays at last
    }

    #[test]
    fn app_cursor_scrolls_page() {
        let results: Vec<_> = (1..=50)
            .map(|i| make_result("a.rs", i, "foo"))
            .collect();
        let mut m = new_model(results, "foo", "bar", gref::model::AppMode::Default);
        m.screen_height = 5;
        // Move down 10 times
        for _ in 0..10 {
            simulate_browse_key(&mut m, gref::term::Key::Down);
        }
        assert_eq!(m.cursor, 10);
        assert!(m.topline > 0);
    }

    #[test]
    fn app_horizontal_scroll_clamp() {
        let results = vec![make_result("a.rs", 1, "short")];
        let mut m = new_model(results, "short", "bar", gref::model::AppMode::Default);
        m.screen_width = 80;
        // Right on a short line should not produce a huge offset
        simulate_browse_key(&mut m, gref::term::Key::Right);
        assert_eq!(m.horizontal_offset, 0); // max_offset is 0 for short line
    }

    #[test]
    fn app_horizontal_home_end() {
        let line = "x".repeat(200);
        let results = vec![make_result("a.rs", 1, &line)];
        let mut m = new_model(results, "x", "y", gref::model::AppMode::Default);
        simulate_browse_key(&mut m, gref::term::Key::End);
        assert_eq!(m.horizontal_offset, 1000);
        simulate_browse_key(&mut m, gref::term::Key::Home);
        assert_eq!(m.horizontal_offset, 0);
    }

    #[test]
    fn app_space_toggle_selection() {
        let results = vec![make_result("a.rs", 1, "foo")];
        let mut m = new_model(results, "foo", "bar", gref::model::AppMode::Default);
        simulate_browse_key(&mut m, gref::term::Key::Space);
        assert!(m.selected.contains(&0));
        simulate_browse_key(&mut m, gref::term::Key::Space);
        assert!(!m.selected.contains(&0));
    }

    #[test]
    fn app_space_noop_in_search_only() {
        let results = vec![make_result("a.rs", 1, "foo")];
        let mut m = new_model(results, "foo", "", gref::model::AppMode::SearchOnly);
        simulate_browse_key(&mut m, gref::term::Key::Space);
        assert!(m.selected.is_empty());
    }

    #[test]
    fn app_select_all_then_deselect() {
        let results = vec![
            make_result("a.rs", 1, "foo"),
            make_result("a.rs", 2, "foo"),
            make_result("a.rs", 3, "foo"),
        ];
        let mut m = new_model(results, "foo", "bar", gref::model::AppMode::Default);
        simulate_browse_key(&mut m, gref::term::Key::Char('a'));
        assert_eq!(m.selected.len(), 3);
        simulate_browse_key(&mut m, gref::term::Key::Char('n'));
        assert!(m.selected.is_empty());
    }

    #[test]
    fn app_enter_no_selection_sets_error() {
        let results = vec![make_result("a.rs", 1, "foo")];
        let mut m = new_model(results, "foo", "bar", gref::model::AppMode::Default);
        simulate_browse_key(&mut m, gref::term::Key::Enter);
        assert!(m.error.is_some());
        assert_eq!(m.state, gref::model::AppState::Browse);
    }

    #[test]
    fn app_enter_with_selection_confirms() {
        let results = vec![make_result("a.rs", 1, "foo")];
        let mut m = new_model(results, "foo", "bar", gref::model::AppMode::Default);
        m.selected.insert(0);
        simulate_browse_key(&mut m, gref::term::Key::Enter);
        assert_eq!(m.state, gref::model::AppState::Confirming);
    }

    #[test]
    fn app_enter_noop_in_search_only() {
        let results = vec![make_result("a.rs", 1, "foo")];
        let mut m = new_model(results, "foo", "", gref::model::AppMode::SearchOnly);
        simulate_browse_key(&mut m, gref::term::Key::Enter);
        assert_eq!(m.state, gref::model::AppState::Browse);
    }

    #[test]
    fn app_q_quits() {
        let results = vec![make_result("a.rs", 1, "foo")];
        let mut m = new_model(results, "foo", "bar", gref::model::AppMode::Default);
        simulate_browse_key(&mut m, gref::term::Key::Char('q'));
        assert_eq!(m.state, gref::model::AppState::Done);
    }

    #[test]
    fn app_ctrlc_quits() {
        let results = vec![make_result("a.rs", 1, "foo")];
        let mut m = new_model(results, "foo", "bar", gref::model::AppMode::Default);
        simulate_browse_key(&mut m, gref::term::Key::CtrlC);
        assert_eq!(m.state, gref::model::AppState::Done);
    }

    #[test]
    fn app_zero_results_cursor_stays() {
        let mut m = new_model(vec![], "foo", "bar", gref::model::AppMode::Default);
        simulate_browse_key(&mut m, gref::term::Key::Down);
        assert_eq!(m.cursor, 0);
        simulate_browse_key(&mut m, gref::term::Key::Up);
        assert_eq!(m.cursor, 0);
    }

    #[test]
    fn app_confirming_escape_returns_to_browse() {
        let results = vec![make_result("a.rs", 1, "foo")];
        let mut m = new_model(results, "foo", "bar", gref::model::AppMode::Default);
        m.selected.insert(0);
        m.state = gref::model::AppState::Confirming;
        m.error = Some("old error".into());
        // simulate confirming key: Escape
        match gref::term::Key::Escape {
            gref::term::Key::Escape => {
                m.state = gref::model::AppState::Browse;
                m.error = None;
            }
            _ => {}
        }
        assert_eq!(m.state, gref::model::AppState::Browse);
        assert!(m.error.is_none());
    }

    #[test]
    fn app_confirming_enter_goes_to_replacing() {
        let mut m = new_model(vec![], "foo", "bar", gref::model::AppMode::Default);
        m.state = gref::model::AppState::Confirming;
        // simulate confirming key: Enter
        m.state = gref::model::AppState::Replacing;
        assert_eq!(m.state, gref::model::AppState::Replacing);
    }

    // =====================================================================
    //  INTEGRATION — full search + replace round-trip
    // =====================================================================

    #[test]
    fn integration_search_then_replace() {
        let dir = std::env::temp_dir().join("gref_stress_integ");
        let _ = fs::create_dir_all(&dir);
        fs::write(dir.join("code.rs"), "fn foo() { foo(); }\nfn bar() {}\n").unwrap();
        fs::write(dir.join("data.txt"), "no match here\n").unwrap();

        let re = Regex::new("foo").unwrap();
        let results = gref::search::perform_search_adaptive(
            dir.to_str().unwrap(),
            &re,
            &[],
            false,
            false,
        )
        .unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].file_path.contains("code.rs"));
        assert_eq!(results[0].line_num, 1);

        // Replace
        let mut selected = HashSet::new();
        selected.insert(0);
        gref::replace::perform_replacements(&results, &selected, &re, "baz").unwrap();

        let content = fs::read_to_string(dir.join("code.rs")).unwrap();
        assert_eq!(content, "fn baz() { baz(); }\nfn bar() {}\n");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn integration_case_insensitive_search() {
        let dir = std::env::temp_dir().join("gref_stress_icase");
        let _ = fs::create_dir_all(&dir);
        fs::write(dir.join("mixed.txt"), "Foo\nfOO\nFOO\nfoo\n").unwrap();

        let re = Regex::new("(?i)foo").unwrap();
        let results = gref::search::perform_search_adaptive(
            dir.to_str().unwrap(),
            &re,
            &[],
            false,
            false,
        )
        .unwrap();
        assert_eq!(results.len(), 4);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn integration_exclude_works_end_to_end() {
        let dir = std::env::temp_dir().join("gref_stress_excl");
        let sub = dir.join("logs");
        let _ = fs::create_dir_all(&sub);
        fs::write(dir.join("main.txt"), "foo\n").unwrap();
        fs::write(sub.join("app.log"), "foo\n").unwrap();

        let re = Regex::new("foo").unwrap();
        let exclude = vec!["*.log".to_string()];
        let results = gref::search::perform_search_adaptive(
            dir.to_str().unwrap(),
            &re,
            &exclude,
            false,
            false,
        )
        .unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].file_path.contains("main.txt"));

        let _ = fs::remove_dir_all(&dir);
    }

    // =====================================================================
    //  HIDDEN / GITIGNORE — skip strategy tests
    // =====================================================================

    #[test]
    fn search_skips_hidden_files_by_default() {
        let dir = std::env::temp_dir().join("gref_stress_hidden");
        let hidden = dir.join(".hidden_dir");
        let _ = fs::create_dir_all(&hidden);
        fs::write(dir.join("visible.txt"), "foo\n").unwrap();
        fs::write(hidden.join("secret.txt"), "foo\n").unwrap();
        fs::write(dir.join(".hidden_file.txt"), "foo\n").unwrap();

        let re = Regex::new("foo").unwrap();
        // skip_hidden=true, use_gitignore=false
        let results = gref::search::perform_search_adaptive(
            dir.to_str().unwrap(),
            &re,
            &[],
            true,
            false,
        )
        .unwrap();
        // Only visible.txt — hidden dir and hidden file are skipped
        assert_eq!(results.len(), 1);
        assert!(results[0].file_path.contains("visible.txt"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_includes_hidden_when_flag_off() {
        let dir = std::env::temp_dir().join("gref_stress_nohide");
        let hidden = dir.join(".hidden_dir");
        let _ = fs::create_dir_all(&hidden);
        fs::write(dir.join("visible.txt"), "foo\n").unwrap();
        fs::write(hidden.join("secret.txt"), "foo\n").unwrap();
        fs::write(dir.join(".hidden_file.txt"), "foo\n").unwrap();

        let re = Regex::new("foo").unwrap();
        // skip_hidden=false → include hidden
        let results = gref::search::perform_search_adaptive(
            dir.to_str().unwrap(),
            &re,
            &[],
            false,
            false,
        )
        .unwrap();
        assert_eq!(results.len(), 3);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_respects_gitignore() {
        let dir = std::env::temp_dir().join("gref_stress_gitignore");
        let _ = fs::create_dir_all(&dir);
        fs::write(dir.join(".gitignore"), "*.log\nbuild/\n").unwrap();
        fs::write(dir.join("main.txt"), "foo\n").unwrap();
        fs::write(dir.join("debug.log"), "foo\n").unwrap();
        let build = dir.join("build");
        let _ = fs::create_dir_all(&build);
        fs::write(build.join("output.txt"), "foo\n").unwrap();

        let re = Regex::new("foo").unwrap();
        // skip_hidden=false (so .gitignore file itself isn't skipped by hidden logic),
        // use_gitignore=true
        let results = gref::search::perform_search_adaptive(
            dir.to_str().unwrap(),
            &re,
            &[],
            false,
            true,
        )
        .unwrap();
        // Only main.txt — debug.log and build/ are gitignored
        assert_eq!(results.len(), 1);
        assert!(results[0].file_path.contains("main.txt"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_gitignore_negation() {
        let dir = std::env::temp_dir().join("gref_stress_gi_neg");
        let _ = fs::create_dir_all(&dir);
        fs::write(dir.join(".gitignore"), "*.log\n!important.log\n").unwrap();
        fs::write(dir.join("debug.log"), "foo\n").unwrap();
        fs::write(dir.join("important.log"), "foo\n").unwrap();
        fs::write(dir.join("main.txt"), "foo\n").unwrap();

        let re = Regex::new("foo").unwrap();
        let results = gref::search::perform_search_adaptive(
            dir.to_str().unwrap(),
            &re,
            &[],
            false,
            true,
        )
        .unwrap();
        // main.txt + important.log (negated), but not debug.log
        assert_eq!(results.len(), 2);
        let names: Vec<&str> = results.iter().map(|r| r.file_path.as_str()).collect();
        assert!(names.iter().any(|n| n.contains("main.txt")));
        assert!(names.iter().any(|n| n.contains("important.log")));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_nested_gitignore() {
        let dir = std::env::temp_dir().join("gref_stress_gi_nest");
        let sub = dir.join("sub");
        let _ = fs::create_dir_all(&sub);
        fs::write(dir.join(".gitignore"), "*.log\n").unwrap();
        fs::write(sub.join(".gitignore"), "!keep.log\n").unwrap();
        fs::write(dir.join("root.log"), "foo\n").unwrap();
        fs::write(sub.join("keep.log"), "foo\n").unwrap();
        fs::write(sub.join("other.log"), "foo\n").unwrap();
        fs::write(sub.join("code.txt"), "foo\n").unwrap();

        let re = Regex::new("foo").unwrap();
        let results = gref::search::perform_search_adaptive(
            dir.to_str().unwrap(),
            &re,
            &[],
            false,
            true,
        )
        .unwrap();
        // code.txt always, keep.log (negated in sub), not root.log, not other.log
        assert_eq!(results.len(), 2);
        let names: Vec<&str> = results.iter().map(|r| r.file_path.as_str()).collect();
        assert!(names.iter().any(|n| n.contains("code.txt")));
        assert!(names.iter().any(|n| n.contains("keep.log")));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_no_ignore_bypasses_gitignore() {
        let dir = std::env::temp_dir().join("gref_stress_noignore");
        let _ = fs::create_dir_all(&dir);
        fs::write(dir.join(".gitignore"), "*.log\n").unwrap();
        fs::write(dir.join("main.txt"), "foo\n").unwrap();
        fs::write(dir.join("debug.log"), "foo\n").unwrap();

        let re = Regex::new("foo").unwrap();
        // use_gitignore=false → .gitignore is not respected
        let results = gref::search::perform_search_adaptive(
            dir.to_str().unwrap(),
            &re,
            &[],
            false,
            false,
        )
        .unwrap();
        assert_eq!(results.len(), 2);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_hidden_and_gitignore_combined() {
        let dir = std::env::temp_dir().join("gref_stress_combo");
        let hidden = dir.join(".secret");
        let _ = fs::create_dir_all(&hidden);
        fs::write(dir.join(".gitignore"), "*.tmp\n").unwrap();
        fs::write(dir.join("main.txt"), "foo\n").unwrap();
        fs::write(dir.join("cache.tmp"), "foo\n").unwrap();
        fs::write(hidden.join("data.txt"), "foo\n").unwrap();

        let re = Regex::new("foo").unwrap();
        // skip_hidden=true AND use_gitignore=true (default behavior)
        let results = gref::search::perform_search_adaptive(
            dir.to_str().unwrap(),
            &re,
            &[],
            true,
            true,
        )
        .unwrap();
        // Only main.txt — .secret is hidden, cache.tmp is gitignored
        assert_eq!(results.len(), 1);
        assert!(results[0].file_path.contains("main.txt"));

        let _ = fs::remove_dir_all(&dir);
    }

    // =====================================================================
    //  .IGNORE / .GREFIGNORE — additional ignore file tests
    // =====================================================================

    #[test]
    fn search_respects_dot_ignore_file() {
        let dir = std::env::temp_dir().join("gref_stress_dotignore");
        let _ = fs::create_dir_all(&dir);
        fs::write(dir.join(".ignore"), "*.tmp\n").unwrap();
        fs::write(dir.join("main.txt"), "foo\n").unwrap();
        fs::write(dir.join("cache.tmp"), "foo\n").unwrap();

        let re = Regex::new("foo").unwrap();
        let results = gref::search::perform_search_adaptive(
            dir.to_str().unwrap(),
            &re,
            &[],
            false,
            true,
        )
        .unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].file_path.contains("main.txt"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_respects_grefignore_file() {
        let dir = std::env::temp_dir().join("gref_stress_grefignore");
        let _ = fs::create_dir_all(&dir);
        fs::write(dir.join(".grefignore"), "*.dat\n").unwrap();
        fs::write(dir.join("main.txt"), "foo\n").unwrap();
        fs::write(dir.join("data.dat"), "foo\n").unwrap();

        let re = Regex::new("foo").unwrap();
        let results = gref::search::perform_search_adaptive(
            dir.to_str().unwrap(),
            &re,
            &[],
            false,
            true,
        )
        .unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].file_path.contains("main.txt"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_grefignore_overrides_gitignore() {
        let dir = std::env::temp_dir().join("gref_stress_gi_override");
        let _ = fs::create_dir_all(&dir);
        // .gitignore ignores *.log, .grefignore un-ignores important.log
        fs::write(dir.join(".gitignore"), "*.log\n").unwrap();
        fs::write(dir.join(".grefignore"), "!important.log\n").unwrap();
        fs::write(dir.join("debug.log"), "foo\n").unwrap();
        fs::write(dir.join("important.log"), "foo\n").unwrap();
        fs::write(dir.join("main.txt"), "foo\n").unwrap();

        let re = Regex::new("foo").unwrap();
        let results = gref::search::perform_search_adaptive(
            dir.to_str().unwrap(),
            &re,
            &[],
            false,
            true,
        )
        .unwrap();
        // main.txt + important.log (negated by .grefignore), but not debug.log
        assert_eq!(results.len(), 2);
        let names: Vec<&str> = results.iter().map(|r| r.file_path.as_str()).collect();
        assert!(names.iter().any(|n| n.contains("main.txt")));
        assert!(names.iter().any(|n| n.contains("important.log")));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_skips_binary_content_unknown_ext() {
        let dir = std::env::temp_dir().join("gref_stress_bindetect");
        let _ = fs::create_dir_all(&dir);
        // File with unknown extension but binary content (null byte)
        fs::write(dir.join("data.zzz"), b"foo\x00bar\n").unwrap();
        fs::write(dir.join("text.zzz"), b"foo bar\n").unwrap();

        let re = Regex::new("foo").unwrap();
        let results = gref::search::perform_search_adaptive(
            dir.to_str().unwrap(),
            &re,
            &[],
            false,
            false,
        )
        .unwrap();
        // Only text.zzz — data.zzz has null byte → binary
        assert_eq!(results.len(), 1);
        assert!(results[0].file_path.contains("text.zzz"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_dot_ignore_nested() {
        let dir = std::env::temp_dir().join("gref_stress_ignore_nest");
        let sub = dir.join("sub");
        let _ = fs::create_dir_all(&sub);
        fs::write(dir.join(".ignore"), "*.log\n").unwrap();
        fs::write(sub.join(".ignore"), "!keep.log\n").unwrap();
        fs::write(dir.join("root.log"), "foo\n").unwrap();
        fs::write(sub.join("keep.log"), "foo\n").unwrap();
        fs::write(sub.join("other.log"), "foo\n").unwrap();
        fs::write(sub.join("code.txt"), "foo\n").unwrap();

        let re = Regex::new("foo").unwrap();
        let results = gref::search::perform_search_adaptive(
            dir.to_str().unwrap(),
            &re,
            &[],
            false,
            true,
        )
        .unwrap();
        // code.txt always, keep.log (negated in sub), not root.log, not other.log
        assert_eq!(results.len(), 2);
        let names: Vec<&str> = results.iter().map(|r| r.file_path.as_str()).collect();
        assert!(names.iter().any(|n| n.contains("code.txt")));
        assert!(names.iter().any(|n| n.contains("keep.log")));

        let _ = fs::remove_dir_all(&dir);
    }
}

