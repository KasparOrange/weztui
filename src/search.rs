use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

use crate::model::WezWindow;

#[derive(Debug, Clone, PartialEq)]
pub struct SearchEntry {
    pub pane_id: u64,
    pub tab_id: u64,
    pub window_id: u64,
    pub match_text: String,
    pub display: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub entry_index: usize,
    pub score: i64,
    pub match_indices: Vec<usize>,
}

pub fn build_search_entries(windows: &[WezWindow]) -> Vec<SearchEntry> {
    let mut entries = Vec::new();
    for window in windows {
        let win_title = window.title.as_deref().unwrap_or("");
        for tab in &window.tabs {
            let tab_title = tab.title.as_deref().unwrap_or("");
            for pane in &tab.panes {
                let cwd = pane.cwd.as_deref().unwrap_or("");
                let cwd_short = cwd.rsplit('/').next().unwrap_or(cwd);
                let match_text = format!("{} {} {} {}", win_title, tab_title, pane.title, cwd);
                let display = format!(
                    "{}  >  {}  >  {}  ({})",
                    window
                        .title
                        .as_deref()
                        .unwrap_or(&format!("Window {}", window.window_id)),
                    tab.title
                        .as_deref()
                        .unwrap_or(&format!("Tab {}", tab.tab_id)),
                    pane.title,
                    cwd_short,
                );
                entries.push(SearchEntry {
                    pane_id: pane.pane_id,
                    tab_id: tab.tab_id,
                    window_id: window.window_id,
                    match_text,
                    display,
                });
            }
        }
    }
    entries
}

pub fn filter(entries: &[SearchEntry], query: &str) -> Vec<SearchResult> {
    if query.is_empty() {
        return entries
            .iter()
            .enumerate()
            .map(|(i, _)| SearchResult {
                entry_index: i,
                score: 0,
                match_indices: vec![],
            })
            .collect();
    }

    let matcher = SkimMatcherV2::default();
    let mut results: Vec<SearchResult> = entries
        .iter()
        .enumerate()
        .filter_map(|(i, entry)| {
            matcher
                .fuzzy_indices(&entry.match_text, query)
                .map(|(score, indices)| SearchResult {
                    entry_index: i,
                    score,
                    match_indices: indices,
                })
        })
        .collect();

    results.sort_by(|a, b| b.score.cmp(&a.score));
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{WezPane, WezTab, WezWindow};

    fn make_window(id: u64, title: &str, tabs: Vec<WezTab>) -> WezWindow {
        WezWindow {
            window_id: id,
            title: Some(title.to_string()),
            tabs,
        }
    }

    fn make_tab(id: u64, title: &str, panes: Vec<WezPane>) -> WezTab {
        WezTab {
            tab_id: id,
            title: Some(title.to_string()),
            panes,
        }
    }

    fn make_pane(id: u64, title: &str, cwd: &str) -> WezPane {
        WezPane {
            pane_id: id,
            title: title.to_string(),
            cwd: Some(cwd.to_string()),
            is_active: false,
            left: 0,
            top: 0,
            width: 80,
            height: 24,
        }
    }

    fn sample_windows() -> Vec<WezWindow> {
        vec![
            make_window(1, "Dev", vec![
                make_tab(10, "editor", vec![
                    make_pane(100, "nvim", "/home/user/code"),
                    make_pane(101, "zsh", "/home/user/code"),
                ]),
                make_tab(11, "logs", vec![
                    make_pane(102, "tail", "/var/log"),
                ]),
            ]),
            make_window(2, "Browser", vec![
                make_tab(20, "http", vec![
                    make_pane(200, "curl", "/tmp"),
                ]),
            ]),
        ]
    }

    #[test]
    fn build_entries_count_matches_panes() {
        let entries = build_search_entries(&sample_windows());
        assert_eq!(entries.len(), 4); // 2 + 1 + 1 panes
    }

    #[test]
    fn build_entries_fields_populated() {
        let entries = build_search_entries(&sample_windows());
        let nvim = entries.iter().find(|e| e.pane_id == 100).unwrap();
        assert_eq!(nvim.window_id, 1);
        assert_eq!(nvim.tab_id, 10);
        assert!(nvim.match_text.contains("Dev"));
        assert!(nvim.match_text.contains("editor"));
        assert!(nvim.match_text.contains("nvim"));
        assert!(nvim.display.contains("Dev"));
        assert!(nvim.display.contains("editor"));
        assert!(nvim.display.contains("nvim"));
    }

    #[test]
    fn filter_empty_query_returns_all() {
        let entries = build_search_entries(&sample_windows());
        let results = filter(&entries, "");
        assert_eq!(results.len(), entries.len());
        // All scores should be 0
        assert!(results.iter().all(|r| r.score == 0));
    }

    #[test]
    fn filter_exact_match() {
        let entries = build_search_entries(&sample_windows());
        let results = filter(&entries, "nvim");
        assert!(!results.is_empty());
        let best = &results[0];
        assert_eq!(entries[best.entry_index].pane_id, 100);
    }

    #[test]
    fn filter_fuzzy_match() {
        let entries = build_search_entries(&sample_windows());
        let results = filter(&entries, "nvm");
        assert!(!results.is_empty(), "fuzzy match for 'nvm' should find 'nvim'");
    }

    #[test]
    fn filter_no_matches() {
        let entries = build_search_entries(&sample_windows());
        let results = filter(&entries, "zzzzzzzzz");
        assert!(results.is_empty());
    }

    #[test]
    fn filter_results_sorted_by_score() {
        let entries = build_search_entries(&sample_windows());
        let results = filter(&entries, "ed");
        if results.len() >= 2 {
            assert!(results[0].score >= results[1].score);
        }
    }

    #[test]
    fn filter_matches_across_fields() {
        let entries = build_search_entries(&sample_windows());
        // "Dev nvim" spans window title + pane title
        let results = filter(&entries, "Dev nvim");
        assert!(!results.is_empty());
        let best = &results[0];
        assert_eq!(entries[best.entry_index].pane_id, 100);
    }

    #[test]
    fn build_entries_empty_windows() {
        let entries = build_search_entries(&[]);
        assert!(entries.is_empty());
    }
}
