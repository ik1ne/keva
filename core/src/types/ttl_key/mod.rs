use crate::types::key::Key;
use redb::TypeName;
use std::cmp::Ordering;
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TtlKey {
    /// The base timestamp for TTL calculation.
    /// For trashed table: this is `updated_at` (Active → Trash threshold).
    /// For purged table: this is `trashed_at` (Trash → Purge threshold).
    pub timestamp: SystemTime,
    pub key: Key,
}

fn extract_duration(data: &[u8]) -> (Duration, &[u8]) {
    let (secs, data) = data.split_first_chunk::<8>().unwrap();
    let secs = u64::from_be_bytes(*secs);
    let (nanos, data) = data.split_first_chunk::<4>().unwrap();
    let nanos = u32::from_be_bytes(*nanos);

    let expires_at_since_epoch = Duration::new(secs, nanos);
    (expires_at_since_epoch, data)
}

impl redb::Key for TtlKey {
    fn compare(data1: &[u8], data2: &[u8]) -> Ordering {
        let (data1_duration, data1) = extract_duration(data1);
        let (data2_duration, data2) = extract_duration(data2);

        data1_duration
            .cmp(&data2_duration)
            .then_with(|| <Key as redb::Key>::compare(data1, data2))
    }
}

impl redb::Value for TtlKey {
    type SelfType<'a> = TtlKey;
    type AsBytes<'a> = Vec<u8>;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        let (timestamp_since_epoch, data) = extract_duration(data);
        let key = Key::from_bytes(data);

        TtlKey {
            timestamp: SystemTime::UNIX_EPOCH + timestamp_since_epoch,
            key,
        }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        let mut bytes = Vec::new();
        let duration_since_epoch = value
            .timestamp
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        bytes.extend_from_slice(&duration_since_epoch.as_secs().to_be_bytes());
        bytes.extend_from_slice(&duration_since_epoch.subsec_nanos().to_be_bytes());
        bytes.extend_from_slice(<Key as redb::Value>::as_bytes(&value.key));
        bytes
    }

    fn type_name() -> TypeName {
        TypeName::new("keva::TtlKey")
    }
}

#[cfg(test)]
mod tests;
