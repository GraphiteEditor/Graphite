use document_graph_storage::{NodeInput, Value, from_value, to_value};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Encode a frozen mirror value as the self-describing `Value` input slots store.
pub fn encode<T: Serialize>(value: &T) -> Value {
	to_value(value).expect("frozen mirror values serialize to JSON")
}

/// An unexposed literal input holding `value`.
pub fn literal<T: Serialize>(value: T) -> NodeInput {
	NodeInput::Value {
		value: encode(&value),
		exposed: false,
	}
}

/// The literal `input` holds, decoded as `T`. `None` for wires and for literals of other variants.
pub fn literal_of<T: DeserializeOwned>(input: &NodeInput) -> Option<T> {
	match input {
		NodeInput::Value { value, .. } => from_value(value).ok(),
		_ => None,
	}
}
