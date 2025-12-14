use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fmt::Debug;

pub trait FileHasher: Clone + Debug {
    type Output: Serialize + DeserializeOwned + Clone + Debug + Eq;

    fn new() -> Self;
    fn update(&mut self, data: &[u8]);
    fn finalize(&self) -> Self::Output;
}

impl FileHasher for blake3_v1::Hasher {
    type Output = blake3_v1::Hash;

    fn new() -> Self {
        blake3_v1::Hasher::new()
    }

    fn update(&mut self, data: &[u8]) {
        self.update(data);
    }

    fn finalize(&self) -> Self::Output {
        blake3_v1::Hasher::finalize(&self)
    }
}
