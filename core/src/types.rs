pub mod key {
    use nutype::nutype;

    #[nutype(
        sanitize(trim),
        validate(not_empty, len_char_max = 256),
        derive(Debug, Clone, Eq, PartialEq, Hash)
    )]
    pub struct Key(String);
}

pub mod value {
    use std::time::SystemTime;

    pub struct Metadata {
        pub created_at: SystemTime,
        pub updated_at: SystemTime,
        pub trashed_at: Option<SystemTime>,
    }

    pub struct ClipData {
        pub plain_text: Option<String>,
        /// Currently len is 0..=1 but might be extended in the future
        pub rich_data: Vec<RichData>,
    }

    pub enum RichData {
        Files(Vec<FileData>),
        // Image(Vec<u8>),
        // Html(String),
    }

    pub struct FileData {
        pub file_name: String,
        pub hash: u64,
    }

    pub struct Value {
        pub metadata: Metadata,
        pub clip_data: ClipData,
    }
}

pub mod config {
    use std::path::PathBuf;
    use std::time::Duration;

    pub struct Config {
        pub storage_path: PathBuf,
        pub trash_ttl: Duration,
        pub purge_ttl: Duration,
        pub large_file_threshold_bytes: u64,
    }
}

pub mod lifecycle {
    #[derive(Debug, Clone, Copy, Eq, PartialEq)]
    pub enum LifecycleState {
        Active,
        Trash,
        Purge,
    }
}
