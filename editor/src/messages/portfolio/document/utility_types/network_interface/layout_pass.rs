use super::*;
use graph_layout::{Graph, Layout, Node, Placement, Wire, layout};

/// Where each node of a network is drawn and which layer's chain it is drawn in, as of one revision of the interface.
#[derive(Debug, Clone)]
pub struct NetworkLayout {
	revision: u64,
	node_ids: Vec<NodeId>,
	indices: HashMap<NodeId, u32>,
	layout: Layout,
}

impl NetworkLayout {
	pub fn position(&self, node_id: &NodeId) -> Option<IVec2> {
		self.indices.get(node_id).map(|index| self.layout.positions[*index as usize])
	}

	/// The layer whose chain the node is drawn in.
	pub fn chain_layer(&self, node_id: &NodeId) -> Option<NodeId> {
		let layer = self.layout.chain_layers[*self.indices.get(node_id)? as usize]?;
		Some(self.node_ids[layer as usize])
	}

	/// How many nodes the layer's chain holds.
	pub fn chain_length(&self, layer: &NodeId) -> u32 {
		self.indices.get(layer).map_or(0, |index| self.layout.chain_lengths[*index as usize])
	}
}

impl NodeNetworkInterface {
	/// Reads the layout of the network at `network_path`, laying it out again first if anything was written since.
	pub(crate) fn with_layout<R>(&self, network_path: &[NodeId], read: impl FnOnce(&NetworkLayout) -> R) -> Option<R> {
		let cache = &self.network_metadata(network_path)?.transient_metadata.layout;
		let revision = store::revision();
		if cache.with_loaded(|layout| layout.revision != revision) == Some(true) {
			cache.unload();
		}

		cache.with_loaded_or(|| self.lay_out(network_path, revision), read)
	}

	fn lay_out(&self, network_path: &[NodeId], revision: u64) -> Option<NetworkLayout> {
		let network = self.nested_network(network_path)?;
		let node_ids: Vec<NodeId> = network.nodes.keys().copied().collect();
		let indices: HashMap<NodeId, u32> = node_ids.iter().enumerate().map(|(index, node_id)| (*node_id, index as u32)).collect();

		let nodes = node_ids.iter().map(|node_id| self.layout_node(node_id, network_path)).collect::<Option<Vec<_>>>()?;
		let wires = node_ids
			.iter()
			.enumerate()
			.flat_map(|(target, node_id)| {
				let indices = &indices;
				network.nodes[node_id].inputs.iter().enumerate().filter_map(move |(input, node_input)| match node_input {
					NodeInput::Node { node_id, output_index, .. } => Some(Wire {
						source: *indices.get(node_id)?,
						output: *output_index as u32,
						target: target as u32,
						input: input as u32,
					}),
					_ => None,
				})
			})
			.collect();

		Some(NetworkLayout {
			revision,
			layout: layout(&Graph { nodes, wires }),
			node_ids,
			indices,
		})
	}

	/// The node as the layout pass reads it, which only places it, so its size and appearance are left out.
	fn layout_node(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<Node> {
		let metadata = self.node_metadata(node_id, network_path)?;
		let (placement, is_layer) = match &metadata.persistent_metadata.node_type_metadata {
			NodeTypePersistentMetadata::Layer(layer) => match layer.position {
				LayerPosition::Absolute(position) => (Placement::Absolute(position), true),
				LayerPosition::Stack(gap) => (Placement::Stack(gap), true),
				LayerPosition::Relative(offset) => (Placement::Relative(offset), true),
			},
			NodeTypePersistentMetadata::Node(node) => match *node.position() {
				NodePosition::Absolute(position) => (Placement::Absolute(position), false),
				NodePosition::Chain => (Placement::Chain, false),
				NodePosition::Relative(offset) => (Placement::Relative(offset), false),
			},
		};

		Some(Node {
			placement,
			is_layer,
			rows: self.node_height(node_id, network_path).saturating_sub(1),
			width: 0,
			inputs: 0,
			outputs: 0,
			chainable: !self.is_redirect_dot(node_id, network_path) && !self.hidden_primary_output(node_id, network_path),
			name_width: 0.,
			locked: false,
		})
	}
}
