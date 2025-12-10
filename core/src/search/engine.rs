//! Search engine implementation

use std::path::Path;
use regex::Regex;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Schema, STORED, TEXT, Field, Value as TantivyValue};
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument};

use crate::config::Config;
use crate::error::Result;
use crate::model::{Key, Lifecycle};
use crate::storage::Database;

use super::fuzzy::FuzzyMatcher;
use super::{SearchMode, SearchResult, SearchScope};

/// Search engine combining fuzzy matching and full-text search
pub struct SearchEngine {
    index: Index,
    reader: IndexReader,
    key_field: Field,
    content_field: Field,
    fuzzy_matcher: FuzzyMatcher,
}

impl SearchEngine {
    /// Open or create the search engine
    pub fn open(config: &Config) -> Result<Self> {
        let index_dir = config.index_dir();
        std::fs::create_dir_all(&index_dir)?;

        let (index, key_field, content_field) = Self::open_or_create_index(&index_dir)?;

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        Ok(Self {
            index,
            reader,
            key_field,
            content_field,
            fuzzy_matcher: FuzzyMatcher::new(),
        })
    }

    fn open_or_create_index(path: &Path) -> Result<(Index, Field, Field)> {
        let mut schema_builder = Schema::builder();
        let key_field = schema_builder.add_text_field("key", TEXT | STORED);
        let content_field = schema_builder.add_text_field("content", TEXT);
        let schema = schema_builder.build();

        let index = if path.join("meta.json").exists() {
            Index::open_in_dir(path)?
        } else {
            Index::create_in_dir(path, schema)?
        };

        Ok((index, key_field, content_field))
    }

    /// Index an entry
    pub fn index_entry(&self, key: &Key, content: Option<&str>) -> Result<()> {
        let mut writer: IndexWriter<TantivyDocument> = self.index.writer(50_000_000)?;

        // Remove existing document for this key
        let term = tantivy::Term::from_field_text(self.key_field, key.as_str());
        writer.delete_term(term);

        // Add new document
        let mut doc = TantivyDocument::new();
        doc.add_text(self.key_field, key.as_str());
        if let Some(content) = content {
            doc.add_text(self.content_field, content);
        }
        writer.add_document(doc)?;

        writer.commit()?;
        Ok(())
    }

    /// Remove an entry from the index
    pub fn remove_entry(&self, key: &Key) -> Result<()> {
        let mut writer: IndexWriter<TantivyDocument> = self.index.writer(50_000_000)?;
        let term = tantivy::Term::from_field_text(self.key_field, key.as_str());
        writer.delete_term(term);
        writer.commit()?;
        Ok(())
    }

    /// Search for entries
    pub fn search(
        &mut self,
        query: &str,
        scope: SearchScope,
        include_trash: bool,
        db: &Database,
    ) -> Result<Vec<SearchResult>> {
        if query.is_empty() {
            return self.list_all_as_results(include_trash, db);
        }

        let mode = SearchMode::detect(query);

        match mode {
            SearchMode::Fuzzy => self.search_fuzzy(query, scope, include_trash, db),
            SearchMode::Regex => self.search_regex(query, scope, include_trash, db),
        }
    }

    /// Fuzzy search
    fn search_fuzzy(
        &mut self,
        query: &str,
        scope: SearchScope,
        include_trash: bool,
        db: &Database,
    ) -> Result<Vec<SearchResult>> {
        let entries = db.list_all()?;
        let mut results = Vec::new();

        for entry in entries {
            let lifecycle = entry.lifecycle();

            // Skip purged entries
            if lifecycle == Lifecycle::Purged {
                continue;
            }

            // Skip trash unless requested
            if lifecycle == Lifecycle::Trash && !include_trash {
                continue;
            }

            // Match against key
            let key_match = self.fuzzy_matcher.match_with_indices(query, entry.key.as_str());

            // Match against content if scope includes it
            let content_match = if scope == SearchScope::KeysAndContent {
                entry.value.plain_text.as_ref().and_then(|text| {
                    self.fuzzy_matcher.match_score(query, text)
                })
            } else {
                None
            };

            // Use the best match
            let (score, indices) = match (key_match, content_match) {
                (Some((key_score, indices)), Some(content_score)) => {
                    if key_score >= content_score {
                        (key_score, indices)
                    } else {
                        (content_score, Vec::new())
                    }
                }
                (Some((score, indices)), None) => (score, indices),
                (None, Some(score)) => (score, Vec::new()),
                (None, None) => continue,
            };

            let mut result = SearchResult::new(entry.key, score, lifecycle);
            result.matched_indices = indices;
            results.push(result);
        }

        // Sort results: active first, then by score descending
        results.sort_by(|a, b| {
            // Trash items go to the bottom
            match (a.is_trash, b.is_trash) {
                (true, false) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                _ => b.score.cmp(&a.score), // Higher score first
            }
        });

        Ok(results)
    }

    /// Regex search
    fn search_regex(
        &self,
        query: &str,
        scope: SearchScope,
        include_trash: bool,
        db: &Database,
    ) -> Result<Vec<SearchResult>> {
        let regex = Regex::new(query)?;
        let entries = db.list_all()?;
        let mut results = Vec::new();

        for entry in entries {
            let lifecycle = entry.lifecycle();

            // Skip purged entries
            if lifecycle == Lifecycle::Purged {
                continue;
            }

            // Skip trash unless requested
            if lifecycle == Lifecycle::Trash && !include_trash {
                continue;
            }

            // Match against key
            let key_match = regex.find(entry.key.as_str());

            // Match against content if scope includes it
            let content_match = if scope == SearchScope::KeysAndContent {
                entry.value.plain_text.as_ref().and_then(|text| regex.find(text))
            } else {
                None
            };

            // Use the shortest match (prefer more specific matches)
            let match_len = match (key_match, content_match) {
                (Some(km), Some(cm)) => Some(km.len().min(cm.len())),
                (Some(km), None) => Some(km.len()),
                (None, Some(cm)) => Some(cm.len()),
                (None, None) => None,
            };

            if let Some(len) = match_len {
                // Score is inverse of match length (shorter = better)
                let score = (u32::MAX - len as u32).max(1);
                let result = SearchResult::new(entry.key, score, lifecycle);
                results.push(result);
            }
        }

        // Sort results: active first, then by score descending (shorter matches first)
        results.sort_by(|a, b| {
            match (a.is_trash, b.is_trash) {
                (true, false) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                _ => b.score.cmp(&a.score),
            }
        });

        Ok(results)
    }

    /// List all entries as search results (for empty query)
    fn list_all_as_results(
        &self,
        include_trash: bool,
        db: &Database,
    ) -> Result<Vec<SearchResult>> {
        let entries = db.list_all()?;
        let mut results: Vec<SearchResult> = entries
            .into_iter()
            .filter_map(|entry| {
                let lifecycle = entry.lifecycle();

                // Skip purged entries
                if lifecycle == Lifecycle::Purged {
                    return None;
                }

                // Skip trash unless requested
                if lifecycle == Lifecycle::Trash && !include_trash {
                    return None;
                }

                Some(SearchResult::new(entry.key, u32::MAX, lifecycle))
            })
            .collect();

        // Sort: active first, then alphabetically
        results.sort_by(|a, b| {
            match (a.is_trash, b.is_trash) {
                (true, false) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                _ => a.key.as_str().cmp(b.key.as_str()),
            }
        });

        Ok(results)
    }

    /// Full-text search using Tantivy (for content search)
    #[allow(dead_code)]
    fn search_fulltext(&self, query: &str, limit: usize) -> Result<Vec<String>> {
        let searcher = self.reader.searcher();
        let query_parser = QueryParser::for_index(&self.index, vec![self.key_field, self.content_field]);
        let parsed_query = query_parser.parse_query(query)
            .map_err(|e| crate::Error::Config(format!("Query parse error: {}", e)))?;

        let top_docs = searcher.search(&parsed_query, &TopDocs::with_limit(limit))?;

        let mut results = Vec::new();
        for (_score, doc_address) in top_docs {
            let doc: TantivyDocument = searcher.doc(doc_address)?;
            if let Some(key) = doc.get_first(self.key_field) {
                if let Some(key_str) = key.as_str() {
                    results.push(key_str.to_string());
                }
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Entry, Value};
    use tempfile::TempDir;

    fn test_config(temp_dir: &TempDir) -> Config {
        Config::new(temp_dir.path().to_path_buf())
    }

    #[test]
    fn test_index_and_search() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(&temp_dir);
        let db = Database::open(&config).unwrap();
        let mut engine = SearchEngine::open(&config).unwrap();

        // Add some entries
        let key1 = Key::new("project/config").unwrap();
        let key2 = Key::new("project/readme").unwrap();

        db.put(&Entry::new(key1.clone(), Value::plain_text("Configuration settings"))).unwrap();
        db.put(&Entry::new(key2.clone(), Value::plain_text("Read me first"))).unwrap();

        engine.index_entry(&key1, Some("Configuration settings")).unwrap();
        engine.index_entry(&key2, Some("Read me first")).unwrap();

        // Search
        let results = engine.search("config", SearchScope::Keys, false, &db).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].key.as_str(), "project/config");
    }
}
