use nutype::nutype;
use redb::TypeName;
use std::cmp::Ordering;
use std::str;

pub const MAX_KEY_LENGTH: usize = 256;

#[nutype(
    new_unchecked,
    sanitize(trim),
    validate(not_empty, len_char_max = MAX_KEY_LENGTH),
    derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        AsRef,
        Deref,
        TryFrom,
        Into,
        Hash,
        Borrow,
        Display,
        Serialize,
        Deserialize,
    )
)]
pub struct Key(String);

impl redb::Key for Key {
    fn compare(data1: &[u8], data2: &[u8]) -> Ordering {
        let s1 = str::from_utf8(data1).expect("invalid UTF-8 in key");
        let s2 = str::from_utf8(data2).expect("invalid UTF-8 in key");

        s1.cmp(s2)
    }
}

impl redb::Value for Key {
    type SelfType<'a> = Self;
    type AsBytes<'a> = &'a [u8];

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        let s = str::from_utf8(data).expect("invalid UTF-8 in key");
        Self::try_from(s).unwrap()
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        value.as_bytes()
    }

    fn type_name() -> TypeName {
        TypeName::new("keva::Key")
    }
}
