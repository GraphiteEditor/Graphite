use crate::Value;
use serde::Serialize;
use serde::de::DeserializeOwned;

pub fn to_value<T: Serialize + ?Sized>(value: &T) -> Result<Value, ValueError> {
	serde_json::to_value(value).map(Value::from).map_err(ValueError)
}

pub fn from_value<T: DeserializeOwned>(value: &Value) -> Result<T, ValueError> {
	serde_json::from_value(value.clone().into()).map_err(ValueError)
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

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct ValueError(serde_json::Error);

#[cfg(test)]
mod tests {
	use super::*;
	use serde::Deserialize;

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

	#[derive(Debug, PartialEq, Serialize, Deserialize)]
	enum Sample {
		Unit,
		Newtype(u8),
		Tuple(i64, String),
		Struct { flag: bool, nested: Option<Box<Sample>> },
	}

	#[derive(Debug, PartialEq, Serialize, Deserialize)]
	struct Record {
		samples: Vec<Sample>,
		by_id: std::collections::BTreeMap<u32, f64>,
		bytes: Vec<u8>,
		letter: char,
		unit: (),
		big: u64,
	}

	fn record() -> Record {
		Record {
			samples: vec![
				Sample::Unit,
				Sample::Newtype(7),
				Sample::Tuple(-3, "t".into()),
				Sample::Struct {
					flag: true,
					nested: Some(Box::new(Sample::Unit)),
				},
				Sample::Struct { flag: false, nested: None },
			],
			by_id: [(2, 0.5), (10, -1.)].into(),
			bytes: vec![0, 255],
			letter: 'x',
			unit: (),
			big: u64::MAX,
		}
	}

	#[test]
	fn from_value_round_trips_serde_types() {
		assert_eq!(from_value::<Record>(&to_value(&record()).unwrap()).unwrap(), record());
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
