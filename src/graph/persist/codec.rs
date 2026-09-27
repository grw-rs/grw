use bincode::Options;

pub(crate) fn decode<V: ::serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<V, bincode::Error> {
    bincode::DefaultOptions::new().with_fixint_encoding().reject_trailing_bytes().deserialize(bytes)
}

pub(crate) fn encode<V: ::serde::Serialize + ?Sized>(value: &V) -> Result<Vec<u8>, bincode::Error> {
    bincode::serialize(value)
}
