//! Fuzzy matching using nucleo

use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

/// Fuzzy matcher wrapper
pub struct FuzzyMatcher {
    matcher: Matcher,
}

impl FuzzyMatcher {
    /// Create a new fuzzy matcher
    pub fn new() -> Self {
        Self {
            matcher: Matcher::new(Config::DEFAULT),
        }
    }

    /// Match a pattern against a haystack, returning score and matched indices
    pub fn match_with_indices(&mut self, pattern: &str, haystack: &str) -> Option<(u32, Vec<usize>)> {
        if pattern.is_empty() {
            return Some((u32::MAX, Vec::new()));
        }

        let pat = Pattern::new(
            pattern,
            CaseMatching::Smart,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );

        let mut indices = Vec::new();
        let haystack_utf32: Vec<char> = haystack.chars().collect();
        let haystack_str = Utf32Str::Unicode(&haystack_utf32);

        let score = pat.indices(haystack_str, &mut self.matcher, &mut indices)?;

        // Convert indices to usize
        let indices: Vec<usize> = indices.iter().map(|&i| i as usize).collect();

        Some((score, indices))
    }

    /// Match a pattern against a haystack, returning only the score
    pub fn match_score(&mut self, pattern: &str, haystack: &str) -> Option<u32> {
        self.match_with_indices(pattern, haystack).map(|(score, _)| score)
    }
}

impl Default for FuzzyMatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let mut matcher = FuzzyMatcher::new();
        let result = matcher.match_with_indices("hello", "hello");
        assert!(result.is_some());
        let (score, _) = result.unwrap();
        assert!(score > 0);
    }

    #[test]
    fn test_fuzzy_match() {
        let mut matcher = FuzzyMatcher::new();
        let result = matcher.match_with_indices("prj", "project");
        assert!(result.is_some());
    }

    #[test]
    fn test_path_match() {
        let mut matcher = FuzzyMatcher::new();
        let result = matcher.match_with_indices("cfg", "project/config");
        assert!(result.is_some());
    }

    #[test]
    fn test_no_match() {
        let mut matcher = FuzzyMatcher::new();
        let result = matcher.match_with_indices("xyz", "abc");
        assert!(result.is_none());
    }

    #[test]
    fn test_empty_pattern() {
        let mut matcher = FuzzyMatcher::new();
        let result = matcher.match_with_indices("", "anything");
        assert!(result.is_some());
    }
}
