#![allow(unused)]
use std::{
	borrow::Cow,
	collections::{BTreeMap, HashMap},
};

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
	history: HashMap<DeltaId, Delta>,
	head: DeltaId,
}

type DeclarationId = u64; // content based hash
type NodeId = u64;
type NetworkId = u64;
type ProtoNodeId = String;
type TimeStamp = u64;
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
	Value { raw_value: &'static [u8], exposed: bool },
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

type DeltaId = u64;

#[derive(Clone, Debug)]
struct Delta {
	timestamp: TimeStamp,
	predecessor: Option<DeltaId>,
	id: DeltaId,
	delta_type: RegistryDelta,
	reverse: RegistryDelta,
}

#[derive(Clone, Debug)]
enum RegistryDelta {
	AddNode { node_id: NodeId, node: Node },
	RemoveNode { node_id: NodeId },
	ChangeNodeInput { node_id: NodeId, input_idx: usize, new_node: NodeInput },
	ChangeNodeAttribute { node_id: NodeId, delta: AttributeDelta },
	ChangeNodeInputAttribute { node_id: NodeId, input_idx: usize, delta: AttributeDelta },
	SetNetworkExport { network: NetworkId, network_output_nodes: Vec<NodeId> },
	RemoveNetwork { network: NetworkId },
}

#[derive(Clone, Debug)]
enum AttributeDelta {
	Set { key: String, value: Value },
	Remove { key: String },
}

impl Document {
	pub fn restore_node_from_history(&mut self, node_id: NodeId) -> Result<(), CrdtError> {
		for delta in self.history_iter().reverse() {
			if let RegistryDelta::AddNode { .. } = delta.delta_type {
				return self.apply_delta(delta.clone());
			}
		}
		Err(CrdtError::NodeNotFoundInHistory)
	}

	pub fn apply_delta(&mut self, delta: Delta) -> Result<(), CrdtError> {
		if let Some(pred) = delta.predecessor {
			assert!(self.history.contains_key(&pred));
		}

		match delta.delta_type {
			RegistryDelta::AddNode { node_id, node } => {
				self.registry.node_instances.insert(node_id, node);
			}
			RegistryDelta::RemoveNode { node_id } => {
				self.registry.node_instances.remove(&node_id);
			}
			RegistryDelta::ChangeNodeInput { node_id, input_idx, new_node } => {
				if let NodeInput::Node { node_id, output_index } = new_node
					&& !self.registry.node_instances.contains_key(&node_id)
				{
					self.restore_node_from_history(node_id)?;
				}
				let node = self.registry.node_instances.get_mut(&node_id).ok_or(CrdtError::TargetNodeDoesNotExist)?;
				let input = node.inputs.get_mut(input_idx).ok_or(CrdtError::InputIndexOutOfBounds)?;
				*input = new_node;
			}
			RegistryDelta::ChangeNodeAttribute { node_id, delta } => {
				let node = self.registry.node_instances.get_mut(&node_id).ok_or(CrdtError::TargetNodeDoesNotExist)?;
				apply_attribute_delta(delta, &mut node.attributes);
			}
			RegistryDelta::ChangeNodeInputAttribute { node_id, input_idx, delta } => {
				let node = self.registry.node_instances.get_mut(&node_id).ok_or(CrdtError::TargetNodeDoesNotExist)?;
				let input_attributes = node.inputs_attributes.get_mut(input_idx).ok_or(CrdtError::InputIndexOutOfBounds)?;
				apply_attribute_delta(delta, input_attributes);
			}
			RegistryDelta::SetNetworkExport { network, network_output_nodes } => {
				self.registry.networks.entry(network).and_modify(|net| net.exports = network_output_nodes);
			}
			RegistryDelta::RemoveNetwork { network } => {
				self.registry.networks.remove(&network);
			}
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
					new_node: input.clone(),
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
			&RegistryDelta::SetNetworkExport { network, .. } | &RegistryDelta::RemoveNetwork { network } => match self.registry.networks.get(&network) {
				Some(old_network) => RegistryDelta::SetNetworkExport {
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
	parent_rev: DeltaId,
}

impl<'a> Iterator for HistoryIter<'a> {
	type Item = &'a Delta;

	fn next(&mut self) -> Option<Self::Item> {
		let delta = self.document.history.get(&self.parent_rev)?;
		self.parent_rev = delta.predecessor?;
		Some(delta)
	}
}

impl<'a> HistoryIter<'a> {
	fn reverse(self) -> HistoryIter<'a> {
		todo!()
	}
}

enum CrdtError {
	TargetNodeDoesNotExist,
	InputIndexOutOfBounds,
	NodeNotFoundInHistory,
}
