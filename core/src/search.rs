use crate::types::key::Key;

pub struct SearchIndex {
    keys: Vec<Key>,
    // nucleo matcher state
}

impl SearchIndex {
    pub fn new() -> Self {
        todo!()
    }

    pub fn load(&mut self, keys: Vec<Key>) {
        todo!()
    }

    pub fn add_key(&mut self, key: &Key) {
        todo!()
    }

    pub fn remove_key(&mut self, key: &Key) {
        todo!()
    }

    pub fn rename_key(&mut self, from: &Key, to: &Key) {
        todo!()
    }

    pub fn fuzzy_search(&self, query: &str) -> Vec<SearchResult> {
        todo!()
    }

    pub fn regex_search(&self, pattern: &str) -> Result<Vec<SearchResult>, RegexError> {
        todo!()
    }
}

pub struct SearchResult {
    pub key: Key,
    pub score: u32,
}

#[derive(Debug)]
pub struct RegexError {
    pub message: String,
}
