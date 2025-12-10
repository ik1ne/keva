//! Database layer using redb

use redb::{Database as RedbDatabase, ReadableTable, ReadableTableMetadata, TableDefinition};

use crate::config::Config;
use crate::error::Result;
use crate::model::{Entry, Key};

/// Table definition for entries
const ENTRIES_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("entries");

/// Database wrapper for redb
pub struct Database {
    db: RedbDatabase,
}

impl Database {
    /// Open or create the database
    pub fn open(config: &Config) -> Result<Self> {
        std::fs::create_dir_all(&config.data_dir)?;
        let db = RedbDatabase::create(config.db_path())?;

        // Ensure the entries table exists
        let write_txn = db.begin_write()?;
        {
            let _ = write_txn.open_table(ENTRIES_TABLE)?;
        }
        write_txn.commit()?;

        Ok(Self { db })
    }

    /// Get an entry by key
    pub fn get(&self, key: &Key) -> Result<Option<Entry>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(ENTRIES_TABLE)?;

        match table.get(key.as_str())? {
            Some(data) => {
                let entry: Entry = serde_json::from_slice(data.value())?;
                Ok(Some(entry))
            }
            None => Ok(None),
        }
    }

    /// Put an entry
    pub fn put(&self, entry: &Entry) -> Result<()> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(ENTRIES_TABLE)?;
            let data = serde_json::to_vec(entry)?;
            table.insert(entry.key.as_str(), data.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Delete an entry by key
    pub fn delete(&self, key: &Key) -> Result<()> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(ENTRIES_TABLE)?;
            table.remove(key.as_str())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// List all entries
    pub fn list_all(&self) -> Result<Vec<Entry>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(ENTRIES_TABLE)?;

        let mut entries = Vec::new();
        for result in table.iter()? {
            let (_, value) = result?;
            let entry: Entry = serde_json::from_slice(value.value())?;
            entries.push(entry);
        }

        Ok(entries)
    }

    /// Check if a key exists
    pub fn exists(&self, key: &Key) -> Result<bool> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(ENTRIES_TABLE)?;
        Ok(table.get(key.as_str())?.is_some())
    }

    /// Count all entries
    pub fn count(&self) -> Result<usize> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(ENTRIES_TABLE)?;
        Ok(table.len()? as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Value;
    use tempfile::TempDir;

    fn test_config(temp_dir: &TempDir) -> Config {
        Config::new(temp_dir.path().to_path_buf())
    }

    #[test]
    fn test_put_and_get() {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(&test_config(&temp_dir)).unwrap();

        let key = Key::new("test/key").unwrap();
        let entry = Entry::new(key.clone(), Value::plain_text("Hello"));

        db.put(&entry).unwrap();

        let retrieved = db.get(&key).unwrap().unwrap();
        assert_eq!(retrieved.key.as_str(), "test/key");
        assert_eq!(retrieved.value.plain_text, Some("Hello".to_string()));
    }

    #[test]
    fn test_delete() {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(&test_config(&temp_dir)).unwrap();

        let key = Key::new("to-delete").unwrap();
        let entry = Entry::new(key.clone(), Value::plain_text("Delete me"));

        db.put(&entry).unwrap();
        assert!(db.exists(&key).unwrap());

        db.delete(&key).unwrap();
        assert!(!db.exists(&key).unwrap());
    }

    #[test]
    fn test_list_all() {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(&test_config(&temp_dir)).unwrap();

        db.put(&Entry::new(Key::new("a").unwrap(), Value::plain_text("1")))
            .unwrap();
        db.put(&Entry::new(Key::new("b").unwrap(), Value::plain_text("2")))
            .unwrap();
        db.put(&Entry::new(Key::new("c").unwrap(), Value::plain_text("3")))
            .unwrap();

        let all = db.list_all().unwrap();
        assert_eq!(all.len(), 3);
    }
}
