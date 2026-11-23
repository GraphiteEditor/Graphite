//! Format step 4 to 5: positions in a chain or a stack become offsets relative to the nodes reading them, which draw them where they were.

use crate::v4::{Attributes, Delta, Node, NodeField, Prior, Registry, RegistryDelta, Value};
use crate::{v4, v5};
use migration_core::{MigrationError, Upgrade, format_migration};

format_migration! {
	4 => 5,
	registry: v4::Registry,
	delta: v4::Delta,
	hot_op: v4::HotOp,
}

const POSITION: &str = "ui::position";
const CHAIN_WIDTH: i128 = 7;
const LAYER_HEIGHT: i128 = 2;
const LAYER_BOTTOM_GAP: i128 = 1;

impl Upgrade for v4::Registry {
	type Next = v5::Registry;
	fn upgrade(mut self) -> Result<Self::Next, MigrationError> {
		convert_registry(&mut self);
		Ok(self)
	}
}

impl Upgrade for v4::Delta {
	type Next = v5::Delta;
	fn upgrade(mut self) -> Result<Self::Next, MigrationError> {
		convert_delta(&mut self);
		Ok(self)
	}
}

impl Upgrade for v4::HotOp {
	type Next = v5::HotOp;
	fn upgrade(mut self) -> Result<Self::Next, MigrationError> {
		convert_operation(&mut self.op);
		Ok(self)
	}
}

fn convert_registry(registry: &mut Registry) {
	for node in registry.node_instances.values_mut() {
		convert_node(node);
	}
	for tombstone in registry.removed_nodes.values_mut() {
		convert_node(&mut tombstone.content);
	}
}

fn convert_delta(delta: &mut Delta) {
	convert_operation(&mut delta.kind);
	for prior in &mut delta.reverse {
		convert_prior(prior);
	}
}

fn convert_operation(operation: &mut RegistryDelta) {
	match operation {
		RegistryDelta::AddNode { node, .. } | RegistryDelta::RemoveNode { snapshot: node, .. } => convert_node(node),
		RegistryDelta::ChangeNodeAttribute { delta, .. } if delta.key == POSITION => {
			if let Some(position) = &mut delta.value {
				convert_position(position);
			}
		}
		_ => {}
	}
}

fn convert_prior(prior: &mut Prior) {
	match prior {
		Prior::Node { node, .. } => convert_node(node),
		Prior::RemovedNode { tombstone, .. } => convert_node(&mut tombstone.content),
		Prior::NodeField {
			field: NodeField::Attribute { key, previous: Some(previous) },
			..
		} if key == POSITION => convert_position(&mut previous.value),
		_ => {}
	}
}

fn convert_node(node: &mut Node) {
	convert_attributes(&mut node.attributes);
}

fn convert_attributes(attributes: &mut Attributes) {
	if let Some(entry) = attributes.entries.get_mut(POSITION) {
		convert_position(&mut entry.value);
	}
}

fn convert_position(position: &mut Value) {
	let offset = match position {
		Value::Str(variant) if variant == "Chain" => [-CHAIN_WIDTH, 0],
		Value::Object(fields) => match fields.as_slice() {
			[(variant, Value::Int(gap))] if variant == "Stack" => [0, LAYER_HEIGHT + LAYER_BOTTOM_GAP + gap],
			_ => return,
		},
		_ => return,
	};
	*position = Value::Object(vec![("Relative".to_string(), Value::Array(offset.map(Value::Int).to_vec()))]);
}

#[cfg(test)]
mod tests {
	use super::*;

	fn relative(x: i128, y: i128) -> Value {
		Value::Object(vec![("Relative".to_string(), Value::Array(vec![Value::Int(x), Value::Int(y)]))])
	}

	#[test]
	fn a_chained_position_becomes_one_chain_width_left_of_its_reader() {
		let mut position = Value::Str("Chain".to_string());
		convert_position(&mut position);
		assert_eq!(position, relative(-CHAIN_WIDTH, 0));
	}

	#[test]
	fn a_stacked_position_becomes_its_gap_below_the_layer_reading_it() {
		let mut position = Value::Object(vec![("Stack".to_string(), Value::Int(2))]);
		convert_position(&mut position);
		assert_eq!(position, relative(0, 5));
	}

	#[test]
	fn absolute_and_relative_positions_stay_as_they_are() {
		for stored in [Value::Object(vec![("Absolute".to_string(), Value::Array(vec![Value::Int(3), Value::Int(4)]))]), relative(-7, 3)] {
			let mut position = stored.clone();
			convert_position(&mut position);
			assert_eq!(position, stored);
		}
	}
}
