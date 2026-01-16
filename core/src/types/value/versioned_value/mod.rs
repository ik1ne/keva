use redb::TypeName;
pub use v1 as latest_value;

pub mod v1;

pub trait ValueVariant {
    const VERSION: u8;
}

#[derive(Debug, Clone)]
pub enum VersionedValue {
    V1(v1::Value),
}

impl redb::Value for VersionedValue {
    type SelfType<'a> = VersionedValue;
    type AsBytes<'a> = Vec<u8>;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        let (version, data) = data.split_first().expect("empty data");
        match *version {
            v1::Value::VERSION => {
                let v1 = postcard::from_bytes::<v1::Value>(data).expect("invalid value");
                VersionedValue::V1(v1)
            }
            version => panic!("unsupported version: {}", version),
        }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        match value {
            VersionedValue::V1(v1) => postcard::to_extend(v1, vec![v1::Value::VERSION]).unwrap(),
        }
    }

    fn type_name() -> TypeName {
        TypeName::new("keva::Value")
    }
}

#[cfg(test)]
mod tests;
