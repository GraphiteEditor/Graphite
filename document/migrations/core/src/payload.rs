use crate::MigrationError;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Codec of a serialized payload. Mirrors the `document-format` codec table entries relevant to
/// migrations so migration crates don't depend on `document-format`; the runner maps between the two.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PayloadCodec {
	Json,
	MessagePack,
}

/// One serialized document payload (the registry, a single history record, the session) plus its
/// codec. History framing is the runner's concern; migrations always see one record per payload.
#[derive(Clone, Debug)]
pub struct Payload {
	pub bytes: Vec<u8>,
	pub codec: PayloadCodec,
}

impl Payload {
	pub fn new(bytes: Vec<u8>, codec: PayloadCodec) -> Self {
		Self { bytes, codec }
	}

	/// Decode into a typed shape, usually a frozen mirror struct owned by the migration crate.
	pub fn decode<T: DeserializeOwned>(&self) -> Result<T, MigrationError> {
		match self.codec {
			PayloadCodec::Json => serde_json::from_slice(&self.bytes).map_err(|e| MigrationError::Decode(e.to_string())),
			PayloadCodec::MessagePack => rmp_serde::from_slice(&self.bytes).map_err(|e| MigrationError::Decode(e.to_string())),
		}
	}

	/// Encode a typed shape with the given codec.
	pub fn encode<T: Serialize>(value: &T, codec: PayloadCodec) -> Result<Self, MigrationError> {
		let bytes = match codec {
			PayloadCodec::Json => serde_json::to_vec(value).map_err(|e| MigrationError::Encode(e.to_string()))?,
			PayloadCodec::MessagePack => rmp_serde::to_vec(value).map_err(|e| MigrationError::Encode(e.to_string()))?,
		};
		Ok(Self { bytes, codec })
	}

	/// Encode a typed shape, keeping this payload's codec.
	pub fn encode_as<T: Serialize>(&self, value: &T) -> Result<Self, MigrationError> {
		Self::encode(value, self.codec)
	}
}
