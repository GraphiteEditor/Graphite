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

impl From<serde_json::Value> for Value {
	fn from(value: serde_json::Value) -> Self {
		use serde_json::Value as Json;
		match value {
			Json::Null => Value::None,
			Json::Bool(boolean) => Value::Bool(boolean),
			Json::Number(number) => {
				if let Some(int) = number.as_i64() {
					Value::Int(int.into())
				} else if let Some(int) = number.as_u64() {
					Value::Int(int.into())
				} else {
					Value::Float(number.as_f64().unwrap_or(f64::NAN))
				}
			}
			Json::String(string) => Value::Str(string),
			Json::Array(array) => Value::Array(array.into_iter().map(Into::into).collect()),
			Json::Object(object) => Value::Object(object.into_iter().map(|(key, value)| (key, value.into())).collect()),
		}
	}
}

impl From<Value> for serde_json::Value {
	fn from(value: Value) -> Self {
		use serde_json::Value as Json;
		match value {
			Value::None => Json::Null,
			Value::Bool(boolean) => Json::Bool(boolean),
			Value::Int(int) => match (i64::try_from(int), u64::try_from(int)) {
				(Ok(int), _) => int.into(),
				(_, Ok(int)) => int.into(),
				(Err(_), Err(_)) => (int as f64).into(),
			},
			Value::Float(float) => float.into(),
			Value::Str(string) => Json::String(string),
			Value::Array(array) => Json::Array(array.into_iter().map(Into::into).collect()),
			Value::Object(object) => Json::Object(object.into_iter().map(|(key, value)| (key, value.into())).collect()),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn round_trips_through_postcard() {
		let value = serde_json::json!({
			"null": null,
			"bools": [true, false],
			"ints": [0, -1, i64::MIN, u64::MAX],
			"floats": [1.5, -0.25, 1e300],
			"str": "text",
			"empty": { "array": [], "object": {} },
			"nested": { "b": [{ "c": 1 }], "a": "z" },
		});

		let bytes = postcard::to_stdvec(&Value::from(value.clone())).unwrap();
		let decoded: Value = postcard::from_bytes(&bytes).unwrap();
		assert_eq!(serde_json::Value::from(decoded), value);
	}

	#[test]
	fn round_trips_through_json_untagged() {
		let json = serde_json::json!({ "a": [1, -2, 2.5, "s", null, true], "b": {}, "c": [] });
		let value = Value::from(json.clone());
		let text = serde_json::to_string(&value).unwrap();
		assert_eq!(text, serde_json::to_string(&json).unwrap());
		assert_eq!(serde_json::to_value(&value).unwrap(), json);
		assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), value);
	}

	#[test]
	fn json_object_keys_sort_and_last_duplicate_wins() {
		let value: Value = serde_json::from_str(r#"{"b": 1, "a": 2, "b": 3}"#).unwrap();
		assert_eq!(value, Value::Object(vec![("a".into(), Value::Int(2)), ("b".into(), Value::Int(3))]));
	}

	#[test]
	fn number_variants_map_to_serde_json_numbers() {
		let json = |value: Value| serde_json::Value::from(value);
		assert_eq!(json(Value::Int(u64::MAX.into())), serde_json::json!(u64::MAX));
		assert_eq!(json(Value::Int(-1)), serde_json::json!(-1));
		assert_eq!(json(Value::Int(i128::MAX)), serde_json::json!(i128::MAX as f64));
		assert_eq!(json(Value::Float(2.)), serde_json::json!(2.));
		assert_eq!(Value::from(serde_json::json!(2.)), Value::Float(2.));
		assert_eq!(Value::from(serde_json::json!(2)), Value::Int(2));
	}

	#[test]
	fn postcard_tags_follow_declaration_order() {
		let encode = |value: Value| postcard::to_stdvec(&value).unwrap();
		assert_eq!(encode(Value::None), [0]);
		assert_eq!(encode(Value::Bool(true)), [1, 1]);
		assert_eq!(encode(Value::Int(-1)), [2, 1]);
		assert_eq!(encode(Value::Float(0.)), [3, 0, 0, 0, 0, 0, 0, 0, 0]);
		assert_eq!(encode(Value::Str("ab".into())), [4, 2, b'a', b'b']);
		assert_eq!(encode(Value::Array(vec![Value::None])), [5, 1, 0]);
		assert_eq!(encode(Value::Object(vec![("k".into(), Value::None)])), [6, 1, 1, b'k', 0]);
	}
}
