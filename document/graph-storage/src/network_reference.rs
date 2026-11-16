use crate::attr::node::ui::REFERENCE;
use crate::{Attributes, Value};

const IDENTIFIER: &str = "identifier";
const VERSION: &str = "version";

/// The built-in network definition a network was created from, stored under [`REFERENCE`] on the network's attributes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetworkReference {
	pub identifier: String,
	/// The definition's position in its lineage, like [`ProtoNode::version`](crate::ProtoNode).
	pub version: u32,
}

impl NetworkReference {
	pub fn read(attributes: &Attributes) -> Option<Self> {
		attributes.get(REFERENCE).filter(|value| !value.deleted).and_then(|value| Self::from_value(&value.value))
	}

	/// A bare identifier at version 1, so first versions keep the value they had before definitions were versioned.
	pub fn to_value(&self) -> Value {
		if self.version == 1 {
			return Value::Str(self.identifier.clone());
		}

		Value::Object(vec![
			(IDENTIFIER.to_string(), Value::Str(self.identifier.clone())),
			(VERSION.to_string(), Value::Int(self.version.into())),
		])
	}

	pub fn from_value(value: &Value) -> Option<Self> {
		match value {
			Value::Str(identifier) => Some(Self {
				identifier: identifier.clone(),
				version: 1,
			}),
			Value::Object(fields) => {
				let field = |name: &str| fields.iter().find(|(key, _)| key == name).map(|(_, value)| value);
				let (Some(Value::Str(identifier)), Some(Value::Int(version))) = (field(IDENTIFIER), field(VERSION)) else {
					return None;
				};

				Some(Self {
					identifier: identifier.clone(),
					version: (*version).try_into().ok()?,
				})
			}
			_ => None,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn round_trips_through_its_value() {
		for version in [1, 2] {
			let reference = NetworkReference {
				identifier: "Merge".to_string(),
				version,
			};
			assert_eq!(NetworkReference::from_value(&reference.to_value()), Some(reference));
		}
	}

	#[test]
	fn first_version_is_a_bare_identifier() {
		let reference = NetworkReference {
			identifier: "Merge".to_string(),
			version: 1,
		};
		assert_eq!(reference.to_value(), Value::Str("Merge".to_string()));
	}
}
