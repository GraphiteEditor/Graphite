use crate::MigrationError;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Codec of a serialized payload, which `document-format` maps its own codecs onto so migration crates don't depend on it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PayloadCodec {
	Json,
	Postcard,
}

/// One serialized record of a document (the registry, one history delta, one hot op, or the session) plus its codec.
/// `document-format` splits framed files, so migrations always see one record per payload.
#[derive(Clone, Debug)]
pub struct Payload {
	pub bytes: Vec<u8>,
	pub codec: PayloadCodec,
}

impl Payload {
	pub fn new(bytes: Vec<u8>, codec: PayloadCodec) -> Self {
		Self { bytes, codec }
	}

	/// Decode into a typed shape, usually a frozen type of `migration-formats`.
	pub fn decode<T: DeserializeOwned>(&self) -> Result<T, MigrationError> {
		match self.codec {
			PayloadCodec::Json => serde_json::from_slice(&self.bytes).map_err(|e| MigrationError::Decode(e.to_string())),
			PayloadCodec::Postcard => postcard::from_bytes(&self.bytes).map_err(|e| MigrationError::Decode(e.to_string())),
		}
	}

	/// Encode a typed shape with the given codec.
	pub fn encode<T: Serialize>(value: &T, codec: PayloadCodec) -> Result<Self, MigrationError> {
		let bytes = match codec {
			PayloadCodec::Json => serde_json::to_vec(value).map_err(|e| MigrationError::Encode(e.to_string()))?,
			PayloadCodec::Postcard => postcard::to_stdvec(value).map_err(|e| MigrationError::Encode(e.to_string()))?,
		};
		Ok(Self { bytes, codec })
	}

	/// Encode a typed shape, keeping this payload's codec.
	pub fn encode_as<T: Serialize>(&self, value: &T) -> Result<Self, MigrationError> {
		Self::encode(value, self.codec)
	}
}
