use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self")]
pub enum Value {
	None,
	Bool(bool),
	Int(i128),
	Float(f64),
	Str(String),
	Array(Vec<Value>),
	Object(Vec<(String, Value)>),
}

// Using `Serializer::is_human_readable` to determine whether to serialize tagged.
// We assume that any human-readable format is self describing and does not need tags.
impl Serialize for Value {
	fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		if !serializer.is_human_readable() {
			return Value::serialize(self, serializer);
		}
		match self {
			Value::None => serializer.serialize_unit(),
			Value::Bool(boolean) => serializer.serialize_bool(*boolean),
			Value::Int(int) => serializer.serialize_i128(*int),
			Value::Float(float) => serializer.serialize_f64(*float),
			Value::Str(string) => serializer.serialize_str(string),
			Value::Array(array) => serializer.collect_seq(array),
			Value::Object(object) => serializer.collect_map(object.iter().map(|(key, value)| (key, value))),
		}
	}
}

// Using `Deserialize::is_human_readable` to determine whether to deserialize tagged.
// We assume that any human-readable format is self describing and does not need tags.
impl<'de> Deserialize<'de> for Value {
	fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		if !deserializer.is_human_readable() {
			return Value::deserialize(deserializer);
		}

		struct Untagged;
		impl<'de> Visitor<'de> for Untagged {
			type Value = Value;
			fn visit_unit<E>(self) -> Result<Value, E> {
				Ok(Value::None)
			}
			fn visit_none<E>(self) -> Result<Value, E> {
				Ok(Value::None)
			}
			fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
				Deserialize::deserialize(deserializer)
			}
			fn visit_bool<E>(self, value: bool) -> Result<Value, E> {
				Ok(Value::Bool(value))
			}
			fn visit_i64<E>(self, value: i64) -> Result<Value, E> {
				Ok(Value::Int(value.into()))
			}
			fn visit_u64<E>(self, value: u64) -> Result<Value, E> {
				Ok(Value::Int(value.into()))
			}
			fn visit_i128<E>(self, value: i128) -> Result<Value, E> {
				Ok(Value::Int(value))
			}
			fn visit_u128<E: de::Error>(self, value: u128) -> Result<Value, E> {
				i128::try_from(value).map(Value::Int).map_err(E::custom)
			}
			fn visit_f64<E>(self, value: f64) -> Result<Value, E> {
				Ok(Value::Float(value))
			}
			fn visit_str<E>(self, value: &str) -> Result<Value, E> {
				Ok(Value::Str(value.to_owned()))
			}
			fn visit_string<E>(self, value: String) -> Result<Value, E> {
				Ok(Value::Str(value))
			}
			fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
				let mut array = Vec::with_capacity(seq.size_hint().unwrap_or(0));
				while let Some(value) = seq.next_element()? {
					array.push(value);
				}
				Ok(Value::Array(array))
			}
			fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
				let mut object: Vec<(String, Value)> = Vec::with_capacity(map.size_hint().unwrap_or(0));
				while let Some(entry) = map.next_entry()? {
					object.push(entry);
				}
				object.sort_by(|a, b| a.0.cmp(&b.0));
				object.dedup_by(|b, a| {
					let dedup = b.0 == a.0;
					if dedup {
						std::mem::swap(&mut a.1, &mut b.1);
					}
					dedup
				});
				Ok(Value::Object(object))
			}
			fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
				formatter.write_str("a valid Value")
			}
		}
		deserializer.deserialize_any(Untagged)
	}
}
