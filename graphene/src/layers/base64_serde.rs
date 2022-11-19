//! Basic wrapper for [`serde`] for [`base64`] encoding

use serde::{Deserialize, Deserializer, Serializer};

pub fn as_base64<S>(key: &std::rc::Rc<Vec<u8>>, serializer: S) -> Result<S::Ok, S::Error>
where
	S: Serializer,
{
	serializer.serialize_str(&base64::encode(key.as_slice()))
}

pub fn from_base64<'a, D>(deserializer: D) -> Result<std::rc::Rc<Vec<u8>>, D::Error>
where
	D: Deserializer<'a>,
{
	use serde::de::Error;

	String::deserialize(deserializer)
		.and_then(|string| base64::decode(string).map_err(|err| Error::custom(err.to_string())))
		.map(std::rc::Rc::new)
		.map_err(serde::de::Error::custom)
}
