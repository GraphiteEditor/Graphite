#![allow(unused)]
use std::{
	borrow::Cow,
	collections::{BTreeMap, HashMap},
	sync::Arc,
};

mod runtime_translation;

#[derive(Clone, Debug)]
struct Registry {
	node_declarations: BTreeMap<DeclarationId, ProtoNode>,
	node_instances: BTreeMap<NodeId, Node>,
	networks: BTreeMap<NetworkId, Network>,
	exported_nodes: Vec<NodeId>,
}
#[derive(Clone, Debug)]
struct Document {
	registry: Registry,
	history: HashMap<Rev, Delta>,
	head: Rev,
}

type DeclarationId = u64; // content based hash
type NodeId = u64;
type NetworkId = u64;
type ProtoNodeId = String;
type TimeStamp = u64;
type Rev = u64; // Use merkle tree hash?
type Value = (serde_json::Value, TimeStamp);

type Attributes = HashMap<String, Value>;

#[derive(Clone, Debug)]
struct Node {
	implementation: Implementation,
	inputs: Vec<NodeInput>,
	inputs_attributes: Vec<Attributes>,
	attributes: Attributes,
	network: NetworkId,
}

struct NodeAttributes {
	name: Option<String>,
}

#[derive(Clone, Debug)]
enum NodeInput {
	Node { node_id: NodeId, output_index: usize },
	Value { raw_value: Arc<[u8]>, exposed: bool },
	Scope(Cow<'static, str>),
	Import { import_idx: usize },
}

#[derive(Clone, Debug)]
enum Implementation {
	ProtoNode(DeclarationId),
	Network(NetworkId),
}

#[derive(Clone, Debug)]
struct Network {
	exports: Vec<NodeId>,
}

#[derive(Clone, Debug)]
struct ProtoNode {
	identifier: ProtoNodeId,
	code: Option<String>,
	wasm: Option<Vec<u8>>,
}

#[derive(Clone, Debug)]
struct Delta {
	timestamp: TimeStamp,
	predecessor: Option<Rev>,
	id: Rev,
	delta_type: RegistryDelta,
	reverse: RegistryDelta,
}

#[derive(Clone, Debug)]
enum RegistryDelta {
	AddNode { node_id: NodeId, node: Node },
	RemoveNode { node_id: NodeId },
	ChangeNodeInput { node_id: NodeId, input_idx: usize, new_input: NodeInput },
	ChangeNodeAttribute { node_id: NodeId, delta: AttributeDelta },
	ChangeNodeInputAttribute { node_id: NodeId, input_idx: usize, delta: AttributeDelta },
	SetNetwork { network: NetworkId, network_output_nodes: Vec<NodeId> },
	RemoveNetwork { network: NetworkId },
}

#[derive(Clone, Debug)]
enum AttributeDelta {
	Set { key: String, value: Value },
	Remove { key: String },
}

impl Document {
	pub fn restore_node_from_history(&mut self, old_node_id: NodeId) -> Result<(), CrdtError> {
		for delta in self.history_iter() {
			if let RegistryDelta::AddNode { node_id, .. } = delta.reverse
				&& old_node_id == node_id
			{
				return self.revert_delta(delta.clone());
			}
		}
		Err(CrdtError::NotFoundInHistory)
	}
	pub fn restore_network_from_history(&mut self, network_id: NetworkId) -> Result<(), CrdtError> {
		for delta in self.history_iter() {
			if let RegistryDelta::SetNetwork { network, .. } = delta.reverse
				&& network == network_id
			{
				return self.revert_delta(delta.clone());
			}
		}
		Err(CrdtError::NotFoundInHistory)
	}
	pub fn revert_delta(&mut self, mut delta: Delta) -> Result<(), CrdtError> {
		std::mem::swap(&mut delta.delta_type, &mut delta.reverse);
		self.apply_delta(delta)
	}

	pub fn apply_delta(&mut self, delta: Delta) -> Result<(), CrdtError> {
		if let Some(pred) = delta.predecessor {
			assert!(self.history.contains_key(&pred));
		}

		match delta.delta_type {
			RegistryDelta::AddNode { node_id, node } => {
				if self.registry.node_instances.contains_key(&node_id) {
					return Err(CrdtError::NodeAlreadyExists);
				}
				self.registry.node_instances.insert(node_id, node);
			}
			RegistryDelta::RemoveNode { node_id } => {
				self.registry.node_instances.remove(&node_id);
			}
			RegistryDelta::ChangeNodeInput { node_id, input_idx, new_input } => {
				if let NodeInput::Node { node_id, output_index } = new_input {
					self.ensure_node_exists(node_id)?;
				}
				// These operations have to be modeled via node add / remove operation to avoid potential conflicts
				assert!(!matches!(new_input, NodeInput::Node { .. }));
				assert!(!matches!(new_input, NodeInput::Scope { .. }));

				let node = self.registry.node_instances.get_mut(&node_id).ok_or(CrdtError::TargetNodeDoesNotExist)?;
				let input = node.inputs.get_mut(input_idx).ok_or(CrdtError::InputIndexOutOfBounds)?;
				*input = new_input;
			}
			RegistryDelta::ChangeNodeAttribute { node_id, delta } => {
				self.ensure_node_exists(node_id)?;

				let node = self.registry.node_instances.get_mut(&node_id).ok_or(CrdtError::TargetNodeDoesNotExist)?;
				apply_attribute_delta(delta, &mut node.attributes);
			}
			RegistryDelta::ChangeNodeInputAttribute { node_id, input_idx, delta } => {
				self.ensure_node_exists(node_id)?;
				let node = self.registry.node_instances.get_mut(&node_id).ok_or(CrdtError::TargetNodeDoesNotExist)?;
				let input_attributes = node.inputs_attributes.get_mut(input_idx).ok_or(CrdtError::InputIndexOutOfBounds)?;
				apply_attribute_delta(delta, input_attributes);
			}
			RegistryDelta::SetNetwork { network, network_output_nodes } => {
				self.registry.networks.entry(network).and_modify(|net| net.exports = network_output_nodes);
			}
			RegistryDelta::RemoveNetwork { network } => {
				self.registry.networks.remove(&network);
			}
		}
		Ok(())
	}

	fn ensure_node_exists(&mut self, node_id: u64) -> Result<(), CrdtError> {
		if !self.registry.node_instances.contains_key(&node_id) {
			self.restore_node_from_history(node_id)?;
		}
		Ok(())
	}

	fn compute_reverse_delta(&self, delta: &RegistryDelta) -> Result<RegistryDelta, CrdtError> {
		let reverse_delta = match delta {
			&RegistryDelta::AddNode { node_id, .. } => RegistryDelta::RemoveNode { node_id },
			&RegistryDelta::RemoveNode { node_id } => {
				let node = self.registry.node_instances.get(&node_id).ok_or(CrdtError::TargetNodeDoesNotExist)?.clone();
				RegistryDelta::AddNode { node_id, node }
			}
			&RegistryDelta::ChangeNodeInput { node_id, input_idx, .. } => {
				let node = self.registry.node_instances.get(&node_id).ok_or(CrdtError::TargetNodeDoesNotExist)?;
				let input = node.inputs.get(input_idx).ok_or(CrdtError::InputIndexOutOfBounds)?;

				RegistryDelta::ChangeNodeInput {
					node_id,
					input_idx,
					new_input: input.clone(),
				}
			}
			&RegistryDelta::ChangeNodeAttribute { node_id, ref delta } => {
				let node = self.registry.node_instances.get(&node_id).ok_or(CrdtError::TargetNodeDoesNotExist)?;

				RegistryDelta::ChangeNodeAttribute {
					node_id,
					delta: reverse_attribute_delta(delta, &node.attributes),
				}
			}
			&RegistryDelta::ChangeNodeInputAttribute { node_id, input_idx, ref delta } => {
				let node = self.registry.node_instances.get(&node_id).ok_or(CrdtError::TargetNodeDoesNotExist)?;
				let input_attributes = node.inputs_attributes.get(input_idx).ok_or(CrdtError::InputIndexOutOfBounds)?;
				RegistryDelta::ChangeNodeInputAttribute {
					node_id,
					delta: reverse_attribute_delta(delta, input_attributes),
					input_idx,
				}
			}
			&RegistryDelta::SetNetwork { network, .. } | &RegistryDelta::RemoveNetwork { network } => match self.registry.networks.get(&network) {
				Some(old_network) => RegistryDelta::SetNetwork {
					network,
					network_output_nodes: old_network.exports.clone(),
				},
				None => RegistryDelta::RemoveNetwork { network },
			},
		};
		Ok(reverse_delta)
	}

	fn history_iter(&self) -> HistoryIter<'_> {
		HistoryIter {
			document: self,
			parent_rev: self.head,
		}
	}

	fn find_delta(&mut self, check_fn: impl Fn(&Delta) -> bool) -> Result<&Delta, CrdtError> {
		for delta in self.history_iter() {
			if check_fn(delta) {
				return Ok(delta);
			}
		}
		Err(CrdtError::NotFoundInHistory)
	}
}

fn reverse_attribute_delta(delta: &AttributeDelta, attributes: &Attributes) -> AttributeDelta {
	let current_value = attributes.get(delta.key());
	let key = delta.key().to_string();
	match current_value {
		None => AttributeDelta::Remove { key },
		Some(previous) => AttributeDelta::Set { key, value: previous.clone() },
	}
}

impl AttributeDelta {
	fn key(&self) -> &str {
		match self {
			AttributeDelta::Set { key, .. } => key,
			AttributeDelta::Remove { key } => key,
		}
	}
}

fn apply_attribute_delta(delta: AttributeDelta, attributes: &mut Attributes) {
	match delta {
		AttributeDelta::Set { key, value } => {
			attributes.entry(key).and_modify(|x| {
				if value.1 > x.1 {
					*x = value
				}
			});
		}
		AttributeDelta::Remove { key } => {
			attributes.remove(&key);
		}
	}
}

struct HistoryIter<'a> {
	document: &'a Document,
	parent_rev: Rev,
}

impl<'a> Iterator for HistoryIter<'a> {
	type Item = &'a Delta;

	fn next(&mut self) -> Option<Self::Item> {
		let delta = self.document.history.get(&self.parent_rev)?;
		self.parent_rev = delta.predecessor?;
		Some(delta)
	}
}

enum CrdtError {
	TargetNodeDoesNotExist,
	InputIndexOutOfBounds,
	NotFoundInHistory,
	NodeAlreadyExists,
}
