use super::*;
use std::time::SystemTime;

#[test]
fn value_v1_empty_attachments_serialization() {
    let now = SystemTime::now();
    let original_value = v1::Value {
        metadata: v1::Metadata {
            created_at: now,
            updated_at: now,
            lifecycle_state: v1::LifecycleState::Active { last_accessed: now },
        },
        attachments: vec![],
        thumb_version: 0,
    };

    let versioned_value = VersionedValue::V1(original_value.clone());
    let bytes = <VersionedValue as redb::Value>::as_bytes(&versioned_value);
    let deserialized_value = <VersionedValue as redb::Value>::from_bytes(&bytes);

    #[expect(unreachable_patterns)]
    match deserialized_value {
        VersionedValue::V1(v1_value) => {
            assert_eq!(v1_value, original_value);
        }
        _ => panic!("Deserialized to incorrect version"),
    }
}

#[test]
fn value_v1_with_attachments_serialization() {
    let now = SystemTime::now();
    let original_value = v1::Value {
        metadata: v1::Metadata {
            created_at: now,
            updated_at: now,
            lifecycle_state: v1::LifecycleState::Active { last_accessed: now },
        },
        attachments: vec![
            v1::Attachment {
                filename: "test.txt".to_string(),
                size: 1024,
            },
            v1::Attachment {
                filename: "image.png".to_string(),
                size: 2048,
            },
        ],
        thumb_version: 1,
    };

    let versioned_value = VersionedValue::V1(original_value.clone());
    let bytes = <VersionedValue as redb::Value>::as_bytes(&versioned_value);
    let deserialized_value = <VersionedValue as redb::Value>::from_bytes(&bytes);

    #[expect(unreachable_patterns)]
    match deserialized_value {
        VersionedValue::V1(v1_value) => {
            assert_eq!(v1_value, original_value);
        }
        _ => panic!("Deserialized to incorrect version"),
    }
}

#[test]
fn value_v1_trashed_serialization() {
    let now = SystemTime::now();
    let original_value = v1::Value {
        metadata: v1::Metadata {
            created_at: now,
            updated_at: now,
            lifecycle_state: v1::LifecycleState::Trash { trashed_at: now },
        },
        attachments: vec![],
        thumb_version: 0,
    };

    let versioned_value = VersionedValue::V1(original_value.clone());
    let bytes = <VersionedValue as redb::Value>::as_bytes(&versioned_value);
    let deserialized_value = <VersionedValue as redb::Value>::from_bytes(&bytes);

    #[expect(unreachable_patterns)]
    match deserialized_value {
        VersionedValue::V1(v1_value) => {
            assert_eq!(v1_value, original_value);
        }
        _ => panic!("Deserialized to incorrect version"),
    }
}
