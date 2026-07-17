use serde::{Deserialize, Serialize};
#[cfg(feature = "editor")]
use serde_json::Error;
use wasm_bindgen::JsValue;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) enum WasmValue {
	Undefined,
	Bool(bool),
	Number(f64),
	NonFinite(NonFinite),
	Int(i64),
	UInt(u64),
	String(String),
	Bytes(Vec<u8>),
	Seq(Vec<WasmValue>),
	Object(Vec<(String, WasmValue)>),
	Map(Vec<(WasmValue, WasmValue)>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum NonFinite {
	Nan,
	PositiveInfinity,
	NegativeInfinity,
}

impl From<NonFinite> for f64 {
	fn from(value: NonFinite) -> Self {
		match value {
			NonFinite::Nan => f64::NAN,
			NonFinite::PositiveInfinity => f64::INFINITY,
			NonFinite::NegativeInfinity => f64::NEG_INFINITY,
		}
	}
}

impl From<WasmValue> for JsValue {
	fn from(value: WasmValue) -> Self {
		match value {
			WasmValue::Undefined => JsValue::UNDEFINED,
			WasmValue::Bool(value) => JsValue::from_bool(value),
			WasmValue::Number(value) => JsValue::from_f64(value),
			WasmValue::NonFinite(value) => JsValue::from_f64(value.into()),
			WasmValue::Int(value) => JsValue::from(value),
			WasmValue::UInt(value) => JsValue::from(value),
			WasmValue::String(value) => JsValue::from_str(&value),
			WasmValue::Bytes(bytes) => js_sys::Uint8Array::from(bytes.as_slice()).into(),
			WasmValue::Seq(values) => {
				let array = js_sys::Array::new();
				for value in values {
					array.push(&value.into());
				}
				array.into()
			}
			WasmValue::Object(fields) => {
				let object = js_sys::Object::new();
				for (key, value) in fields {
					let _ = js_sys::Reflect::set(&object, &JsValue::from_str(&key), &value.into());
				}
				object.into()
			}
			WasmValue::Map(entries) => {
				let map = js_sys::Map::new();
				for (key, value) in entries {
					map.set(&key.into(), &value.into());
				}
				map.into()
			}
		}
	}
}

#[cfg(feature = "editor")]
pub(crate) fn encode<T: Serialize + ?Sized>(value: &T) -> Result<String, serde_json::Error> {
	serde_json::to_string(&ser::to_value(value)?)
}

#[cfg(feature = "editor")]
mod ser {
	use super::*;
	use serde::ser::{self, Error as _};

	pub(super) fn to_value<T: Serialize + ?Sized>(value: &T) -> Result<WasmValue, Error> {
		value.serialize(ValueSerializer)
	}

	struct ValueSerializer;

	impl ser::Serializer for ValueSerializer {
		type Ok = WasmValue;
		type Error = Error;
		type SerializeSeq = SeqSerializer;
		type SerializeTuple = SeqSerializer;
		type SerializeTupleStruct = SeqSerializer;
		type SerializeTupleVariant = VariantSeqSerializer;
		type SerializeMap = MapSerializer;
		type SerializeStruct = ObjectSerializer;
		type SerializeStructVariant = VariantObjectSerializer;

		fn serialize_bool(self, v: bool) -> Result<WasmValue, Error> {
			Ok(WasmValue::Bool(v))
		}

		fn serialize_i8(self, v: i8) -> Result<WasmValue, Error> {
			Ok(WasmValue::Number(v as f64))
		}

		fn serialize_i16(self, v: i16) -> Result<WasmValue, Error> {
			Ok(WasmValue::Number(v as f64))
		}

		fn serialize_i32(self, v: i32) -> Result<WasmValue, Error> {
			Ok(WasmValue::Number(v as f64))
		}

		fn serialize_i64(self, v: i64) -> Result<WasmValue, Error> {
			Ok(WasmValue::Int(v))
		}

		fn serialize_i128(self, v: i128) -> Result<WasmValue, Error> {
			if let Ok(v) = i64::try_from(v) {
				Ok(WasmValue::Int(v))
			} else {
				u64::try_from(v)
					.map(WasmValue::UInt)
					.map_err(|_| Error::custom(format!("i128 value {v} exceeds the wire BigInt range")))
			}
		}

		fn serialize_u8(self, v: u8) -> Result<WasmValue, Error> {
			Ok(WasmValue::Number(v as f64))
		}

		fn serialize_u16(self, v: u16) -> Result<WasmValue, Error> {
			Ok(WasmValue::Number(v as f64))
		}

		fn serialize_u32(self, v: u32) -> Result<WasmValue, Error> {
			Ok(WasmValue::Number(v as f64))
		}

		fn serialize_u64(self, v: u64) -> Result<WasmValue, Error> {
			Ok(WasmValue::UInt(v))
		}

		fn serialize_u128(self, v: u128) -> Result<WasmValue, Error> {
			u64::try_from(v)
				.map(WasmValue::UInt)
				.map_err(|_| Error::custom(format!("u128 value {v} exceeds the wire BigInt range")))
		}

		fn serialize_f32(self, v: f32) -> Result<WasmValue, Error> {
			self.serialize_f64(v as f64)
		}

		fn serialize_f64(self, v: f64) -> Result<WasmValue, Error> {
			Ok(if v.is_finite() {
				WasmValue::Number(v)
			} else if v.is_nan() {
				WasmValue::NonFinite(NonFinite::Nan)
			} else if v > 0. {
				WasmValue::NonFinite(NonFinite::PositiveInfinity)
			} else {
				WasmValue::NonFinite(NonFinite::NegativeInfinity)
			})
		}

		fn serialize_char(self, v: char) -> Result<WasmValue, Error> {
			Ok(WasmValue::String(v.to_string()))
		}

		fn serialize_str(self, v: &str) -> Result<WasmValue, Error> {
			Ok(WasmValue::String(v.to_string()))
		}

		fn serialize_bytes(self, v: &[u8]) -> Result<WasmValue, Error> {
			Ok(WasmValue::Bytes(v.to_vec()))
		}

		fn serialize_none(self) -> Result<WasmValue, Error> {
			Ok(WasmValue::Undefined)
		}

		fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<WasmValue, Error> {
			value.serialize(self)
		}

		fn serialize_unit(self) -> Result<WasmValue, Error> {
			Ok(WasmValue::Undefined)
		}

		fn serialize_unit_struct(self, _name: &'static str) -> Result<WasmValue, Error> {
			Ok(WasmValue::Undefined)
		}

		fn serialize_unit_variant(self, _name: &'static str, _variant_index: u32, variant: &'static str) -> Result<WasmValue, Error> {
			Ok(WasmValue::String(variant.to_string()))
		}

		fn serialize_newtype_struct<T: Serialize + ?Sized>(self, _name: &'static str, value: &T) -> Result<WasmValue, Error> {
			value.serialize(self)
		}

		fn serialize_newtype_variant<T: Serialize + ?Sized>(self, _name: &'static str, _variant_index: u32, variant: &'static str, value: &T) -> Result<WasmValue, Error> {
			Ok(WasmValue::Object(vec![(variant.to_string(), value.serialize(ValueSerializer)?)]))
		}

		fn serialize_seq(self, len: Option<usize>) -> Result<SeqSerializer, Error> {
			Ok(SeqSerializer(Vec::with_capacity(len.unwrap_or_default())))
		}

		fn serialize_tuple(self, len: usize) -> Result<SeqSerializer, Error> {
			self.serialize_seq(Some(len))
		}

		fn serialize_tuple_struct(self, _name: &'static str, len: usize) -> Result<SeqSerializer, Error> {
			self.serialize_seq(Some(len))
		}

		fn serialize_tuple_variant(self, _name: &'static str, _variant_index: u32, variant: &'static str, len: usize) -> Result<VariantSeqSerializer, Error> {
			Ok(VariantSeqSerializer {
				variant,
				elements: Vec::with_capacity(len),
			})
		}

		fn serialize_map(self, len: Option<usize>) -> Result<MapSerializer, Error> {
			Ok(MapSerializer {
				entries: Vec::with_capacity(len.unwrap_or_default()),
				pending_key: None,
			})
		}

		fn serialize_struct(self, _name: &'static str, len: usize) -> Result<ObjectSerializer, Error> {
			Ok(ObjectSerializer(Vec::with_capacity(len)))
		}

		fn serialize_struct_variant(self, _name: &'static str, _variant_index: u32, variant: &'static str, len: usize) -> Result<VariantObjectSerializer, Error> {
			Ok(VariantObjectSerializer {
				variant,
				fields: Vec::with_capacity(len),
			})
		}
	}

	struct SeqSerializer(Vec<WasmValue>);

	impl ser::SerializeSeq for SeqSerializer {
		type Ok = WasmValue;
		type Error = Error;

		fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
			self.0.push(value.serialize(ValueSerializer)?);
			Ok(())
		}

		fn end(self) -> Result<WasmValue, Error> {
			Ok(WasmValue::Seq(self.0))
		}
	}

	impl ser::SerializeTuple for SeqSerializer {
		type Ok = WasmValue;
		type Error = Error;

		fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
			ser::SerializeSeq::serialize_element(self, value)
		}

		fn end(self) -> Result<WasmValue, Error> {
			ser::SerializeSeq::end(self)
		}
	}

	impl ser::SerializeTupleStruct for SeqSerializer {
		type Ok = WasmValue;
		type Error = Error;

		fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
			ser::SerializeSeq::serialize_element(self, value)
		}

		fn end(self) -> Result<WasmValue, Error> {
			ser::SerializeSeq::end(self)
		}
	}

	struct VariantSeqSerializer {
		variant: &'static str,
		elements: Vec<WasmValue>,
	}

	impl ser::SerializeTupleVariant for VariantSeqSerializer {
		type Ok = WasmValue;
		type Error = Error;

		fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
			self.elements.push(value.serialize(ValueSerializer)?);
			Ok(())
		}

		fn end(self) -> Result<WasmValue, Error> {
			Ok(WasmValue::Object(vec![(self.variant.to_string(), WasmValue::Seq(self.elements))]))
		}
	}

	struct MapSerializer {
		entries: Vec<(WasmValue, WasmValue)>,
		pending_key: Option<WasmValue>,
	}

	impl ser::SerializeMap for MapSerializer {
		type Ok = WasmValue;
		type Error = Error;

		fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Error> {
			self.pending_key = Some(key.serialize(ValueSerializer)?);
			Ok(())
		}

		fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
			let key = self.pending_key.take().ok_or_else(|| Error::custom("Map value serialized without a key"))?;
			self.entries.push((key, value.serialize(ValueSerializer)?));
			Ok(())
		}

		fn end(self) -> Result<WasmValue, Error> {
			Ok(WasmValue::Map(self.entries))
		}
	}

	struct ObjectSerializer(Vec<(String, WasmValue)>);

	impl ser::SerializeStruct for ObjectSerializer {
		type Ok = WasmValue;
		type Error = Error;

		fn serialize_field<T: Serialize + ?Sized>(&mut self, key: &'static str, value: &T) -> Result<(), Error> {
			self.0.push((key.to_string(), value.serialize(ValueSerializer)?));
			Ok(())
		}

		fn end(self) -> Result<WasmValue, Error> {
			Ok(WasmValue::Object(self.0))
		}
	}

	struct VariantObjectSerializer {
		variant: &'static str,
		fields: Vec<(String, WasmValue)>,
	}

	impl ser::SerializeStructVariant for VariantObjectSerializer {
		type Ok = WasmValue;
		type Error = Error;

		fn serialize_field<T: Serialize + ?Sized>(&mut self, key: &'static str, value: &T) -> Result<(), Error> {
			self.fields.push((key.to_string(), value.serialize(ValueSerializer)?));
			Ok(())
		}

		fn end(self) -> Result<WasmValue, Error> {
			Ok(WasmValue::Object(vec![(self.variant.to_string(), WasmValue::Object(self.fields))]))
		}
	}

	#[cfg(test)]
	mod tests {
		use super::*;
		use crate::wasm_value::encode;
		use serde::Serialize;

		#[derive(Serialize)]
		#[serde(rename_all = "camelCase")]
		enum TestMessage {
			UnitVariant,
			StructVariant {
				#[serde(rename = "nodeId")]
				node_id: u64,
				width: u32,
				scale: f64,
				min: f64,
				max: f64,
				label: Option<String>,
				widths: std::collections::BTreeMap<u64, u32>,
				flags: Vec<bool>,
				bytes: serde_bytes::ByteBuf,
			},
		}

		#[test]
		fn mirrors_serde_wasm_bindgen_representations() {
			assert_eq!(to_value(&TestMessage::UnitVariant).unwrap(), WasmValue::String("unitVariant".into()));

			let message = TestMessage::StructVariant {
				node_id: u64::MAX,
				width: 8,
				scale: f64::NAN,
				min: f64::NEG_INFINITY,
				max: f64::INFINITY,
				label: None,
				widths: [(42, 7)].into(),
				flags: vec![true, false],
				bytes: serde_bytes::ByteBuf::from(vec![0, 255]),
			};
			let expected = WasmValue::Object(vec![(
				"structVariant".into(),
				WasmValue::Object(vec![
					("nodeId".into(), WasmValue::UInt(u64::MAX)),
					("width".into(), WasmValue::Number(8.)),
					("scale".into(), WasmValue::NonFinite(NonFinite::Nan)),
					("min".into(), WasmValue::NonFinite(NonFinite::NegativeInfinity)),
					("max".into(), WasmValue::NonFinite(NonFinite::PositiveInfinity)),
					("label".into(), WasmValue::Undefined),
					("widths".into(), WasmValue::Map(vec![(WasmValue::UInt(42), WasmValue::Number(7.))])),
					("flags".into(), WasmValue::Seq(vec![WasmValue::Bool(true), WasmValue::Bool(false)])),
					("bytes".into(), WasmValue::Bytes(vec![0, 255])),
				]),
			)]);
			assert_eq!(to_value(&message).unwrap(), expected);
		}

		#[test]
		fn encode_decode_roundtrip() {
			let message = TestMessage::StructVariant {
				node_id: 1,
				width: 2,
				scale: f64::NAN,
				min: f64::NEG_INFINITY,
				max: 1.5,
				label: Some("hi".into()),
				widths: [(3, 4)].into(),
				flags: vec![true],
				bytes: serde_bytes::ByteBuf::from(vec![9]),
			};

			let json: &str = &encode(&message).unwrap();
			let decoded = serde_json::from_str(json).unwrap();

			assert_eq!(decoded, to_value(&message).unwrap());
		}
	}
}
