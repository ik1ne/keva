use super::*;
use std::time::SystemTime;

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
            plain_text: Some(v1::TextData::Inlined("Hello, World!".to_string())),
            rich_data: vec![],
        },
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
