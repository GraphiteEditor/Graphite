use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize, specta::Type)]
pub struct Uuid(
	#[serde(with = "u64_string")]
	#[specta(type = String)]
	u64,
);

mod u64_string {
	use serde::{self, Deserialize, Deserializer, Serializer};
	use std::str::FromStr;

	// The signature of a serialize_with function must follow the pattern:
	//
	//    fn serialize<S>(&T, S) -> Result<S::Ok, S::Error>
	//    where
	//        S: Serializer
	//
	// although it may also be generic over the input types T.
	pub fn serialize<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		serializer.serialize_str(&value.to_string())
	}

	// The signature of a deserialize_with function must follow the pattern:
	//
	//    fn deserialize<'de, D>(D) -> Result<T, D::Error>
	//    where
	//        D: Deserializer<'de>
	//
	// although it may also be generic over the output types T.
	pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
	where
		D: Deserializer<'de>,
	{
		let s = String::deserialize(deserializer)?;
		u64::from_str(&s).map_err(serde::de::Error::custom)
	}
}
