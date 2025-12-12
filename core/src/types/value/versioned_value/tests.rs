use super::*;
use std::time::{Duration, SystemTime};

#[test]
fn value_v1_serialization() {
    let original_value = v1::Value {
        metadata: v1::Metadata {
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
            trashed_at: None,
            lifecycle_state: v1::LifecycleState::Active,
        },
        clip_data: v1::ClipData {
            plain_text: Some("Hello, World!".to_string()),
            rich_data: vec![],
        },
    };

    let versioned_value = VersionedValue::V1(original_value.clone());
    let bytes = <VersionedValue as redb::Value>::as_bytes(&versioned_value);
    let deserialized_value = <VersionedValue as redb::Value>::from_bytes(&bytes);

    #[expect(unreachable_patterns)]
    match deserialized_value {
        VersionedValue::V1(v1_value) => {
            assert_eq!(
                v1_value.metadata.lifecycle_state,
                original_value.metadata.lifecycle_state
            );
            assert_eq!(
                v1_value.clip_data.plain_text,
                original_value.clip_data.plain_text
            );
            assert_eq!(
                v1_value.metadata.created_at,
                original_value.metadata.created_at
            );
        }
        _ => panic!("Deserialized to incorrect version"),
    }
}
