//! Format step 4 to 5: every node another node reads is stored as an offset from the top left corner of its readers, which draws it where it was.

use crate::v4::{AttributeDelta, AttributeValue, Attributes, Delta, Node, NodeField, NodeId, NodeInput, Prior, Registry, RegistryDelta, Rev, TimeStamp, Value};
use crate::{v4, v5};
use migration_core::{DocumentPayloads, MigrationError, Payload, Upgrade, format_migration};
use std::collections::{HashMap, HashSet};
use std::num::NonZeroU128;

format_migration! {
	4 => 5,
	registry: v4::Registry,
	delta: v4::Delta,
	hot_op: v4::HotOp,
	document: relative_to_readers,
}

const POSITION: &str = "ui::position";
const INTERACTION_END: &str = "interaction_end";
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

fn relative_to_readers(document: &mut DocumentPayloads) -> Result<(), MigrationError> {
	let Some(registry_record) = document.registry.clone() else { return Ok(()) };
	if !document.hot_log.is_empty() {
		return Ok(());
	}

	let mut registry: v5::Registry = registry_record.decode()?;
	let mut conversions = StoredPositions::new(&registry).relative_to_readers();
	conversions.sort_by_key(|(id, _)| *id);

	if document.history.is_empty() {
		for (id, position) in conversions {
			if let Some(entry) = registry.node_instances.get_mut(&id).and_then(|node| node.attributes.entries.get_mut(POSITION)) {
				entry.value = position.into_value();
			}
		}
	} else if !append_as_one_interaction(document, &mut registry, &conversions)? {
		return Ok(());
	}

	document.registry = Some(registry_record.encode_as(&registry)?);
	Ok(())
}

fn append_as_one_interaction(document: &mut DocumentPayloads, registry: &mut v5::Registry, conversions: &[(NodeId, Position)]) -> Result<bool, MigrationError> {
	let (Some(session_record), Some(template)) = (document.session.clone(), document.history.first().cloned()) else {
		return Ok(false);
	};
	let mut session: v5::SessionState = session_record.decode()?;
	let history: Vec<v5::Delta> = document.history.iter().map(Payload::decode).collect::<Result<_, _>>()?;

	let mut counter = history.iter().map(|delta| delta.timestamp.counter).fold(session.clock_counter, u64::max);
	let taken: HashSet<Rev> = history.iter().map(|delta| delta.id).collect();
	let mut placeholders = (1..=u128::MAX).rev().filter_map(NonZeroU128::new).map(Rev).filter(|rev| !taken.contains(rev));
	let retired_at_ms = history.iter().find(|delta| Some(delta.id) == session.head_rev).map_or(0, |delta| delta.retired_at_ms);

	let mut parent = session.head_rev;
	for (index, (id, position)) in conversions.iter().enumerate() {
		let Some(node) = registry.node_instances.get_mut(id) else { continue };
		counter += 1;
		let timestamp = TimeStamp { counter, peer: session.peer_id };
		let value = position.into_value();
		let previous = node.attributes.entries.insert(
			POSITION.to_string(),
			AttributeValue {
				value: value.clone(),
				timestamp,
				deleted: false,
			},
		);

		let mut attributes = Attributes::default();
		if index + 1 == conversions.len() {
			attributes.entries.insert(
				INTERACTION_END.to_string(),
				AttributeValue {
					value: Value::Bool(true),
					timestamp,
					deleted: false,
				},
			);
		}
		let rev = placeholders.next().ok_or_else(|| MigrationError::Encode("no placeholder revision is left".to_string()))?;
		let delta = v5::Delta {
			id: rev,
			parent,
			author: session.peer_id,
			timestamp,
			kind: RegistryDelta::ChangeNodeAttribute {
				id: *id,
				delta: AttributeDelta {
					key: POSITION.to_string(),
					value: Some(value),
				},
			},
			reverse: vec![Prior::NodeField {
				id: *id,
				presence: node.presence,
				field: NodeField::Attribute { key: POSITION.to_string(), previous },
			}],
			attributes,
			retired_at_ms,
		};
		document.history.push(template.encode_as(&delta)?);
		parent = Some(rev);
	}

	session.head_rev = parent;
	session.redo_stack.clear();
	session.clock_counter = counter;
	document.session = Some(session_record.encode_as(&session)?);
	Ok(true)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Position {
	Absolute([i128; 2]),
	Relative([i128; 2]),
}

impl Position {
	fn from_value(value: &Value) -> Option<Self> {
		let Value::Object(fields) = value else { return None };
		let [(variant, Value::Array(coordinates))] = fields.as_slice() else { return None };
		let [Value::Int(x), Value::Int(y)] = coordinates.as_slice() else { return None };
		match variant.as_str() {
			"Absolute" => Some(Self::Absolute([*x, *y])),
			"Relative" => Some(Self::Relative([*x, *y])),
			_ => None,
		}
	}

	fn into_value(self) -> Value {
		let (variant, [x, y]) = match self {
			Self::Absolute(position) => ("Absolute", position),
			Self::Relative(offset) => ("Relative", offset),
		};
		Value::Object(vec![(variant.to_string(), Value::Array(vec![Value::Int(x), Value::Int(y)]))])
	}
}

struct StoredPositions {
	stored: HashMap<NodeId, Position>,
	readers: HashMap<NodeId, Vec<NodeId>>,
	drawn: HashMap<NodeId, [i128; 2]>,
	visiting: HashSet<NodeId>,
}

impl StoredPositions {
	fn new(registry: &v5::Registry) -> Self {
		let stored = registry
			.node_instances
			.iter()
			.filter_map(|(id, node)| Some((*id, Position::from_value(&node.attributes.entries.get(POSITION)?.value)?)))
			.collect();

		let mut readers: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
		for (reader, node) in &registry.node_instances {
			let read_in_the_same_network = node.inputs.iter().filter_map(|slot| match slot.input {
				NodeInput::Node { id, .. } => registry.node_instances.get(&id).is_some_and(|source| source.network == node.network).then_some(id),
				_ => None,
			});
			for source in read_in_the_same_network {
				readers.entry(source).or_default().push(*reader);
			}
		}

		Self {
			stored,
			readers,
			drawn: HashMap::new(),
			visiting: HashSet::new(),
		}
	}

	fn relative_to_readers(mut self) -> Vec<(NodeId, Position)> {
		let nodes: Vec<(NodeId, Position)> = self.stored.iter().map(|(id, position)| (*id, *position)).collect();
		nodes
			.into_iter()
			.filter_map(|(id, stored)| match (stored, self.readers_top_left(id)) {
				(Position::Absolute([x, y]), Some([left, top])) => Some((id, Position::Relative([x - left, y - top]))),
				(Position::Relative(_), None) => Some((id, Position::Absolute([0, 0]))),
				_ => None,
			})
			.collect()
	}

	fn readers_top_left(&mut self, id: NodeId) -> Option<[i128; 2]> {
		let readers = self.readers.get(&id)?.clone();
		readers.into_iter().map(|reader| self.drawn(reader)).reduce(|[left, top], [x, y]| [left.min(x), top.min(y)])
	}

	fn drawn(&mut self, id: NodeId) -> [i128; 2] {
		if let Some(position) = self.drawn.get(&id) {
			return *position;
		}
		if !self.visiting.insert(id) {
			return [0, 0];
		}

		let position = match self.stored.get(&id).copied() {
			Some(Position::Absolute(position)) => position,
			Some(Position::Relative([x, y])) => self.readers_top_left(id).map_or([0, 0], |[left, top]| [left + x, top + y]),
			None => [0, 0],
		};
		self.visiting.remove(&id);
		self.drawn.insert(id, position);
		position
	}
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
