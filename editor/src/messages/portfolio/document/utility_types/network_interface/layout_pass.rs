use super::*;
use graph_layout::{Graph, Layout, Node, Placement, Wire, layout, reach};

/// Where each node of a network is drawn and which layer's chain it is drawn in, as of one revision of the interface.
#[derive(Debug, Clone)]
pub struct NetworkLayout {
	revision: u64,
	node_ids: Vec<NodeId>,
	indices: HashMap<NodeId, u32>,
	graph: Graph,
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

	/// Whether `point`, in node graph space, lies within the box around everything the node draws.
	pub fn reaches(&self, node_id: &NodeId, point: DVec2) -> bool {
		let Some(index) = self.indices.get(node_id) else { return false };
		let [min, max] = reach(&self.graph, &self.layout, *index as usize);
		point.cmpge(min).all() && point.cmplt(max).all()
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

	/// The nodes whose drawing could cover `point`, in node graph space, each offset by how far it is drawn from its grid cell while gliding into it.
	pub(crate) fn nodes_reaching(&self, point: DVec2, network_path: &[NodeId]) -> Vec<NodeId> {
		let glide_offsets = self.glide_offsets(network_path);
		self.with_layout(network_path, |layout| {
			layout.node_ids.iter().filter(|node_id| layout.reaches(node_id, point - glide_offsets.node(node_id))).copied().collect()
		})
		.unwrap_or_default()
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

		let graph = Graph { nodes, wires };
		Some(NetworkLayout {
			revision,
			layout: layout(&graph),
			graph,
			node_ids,
			indices,
		})
	}

	/// The node as the layout pass reads it, sized in grid cells but without the name width, which only exact click targets need.
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
			width: if is_layer { self.layer_width(node_id, network_path).unwrap_or_default() } else { 5 },
			inputs: self.number_of_displayed_inputs(node_id, network_path) as u32,
			outputs: self.number_of_outputs(node_id, network_path) as u32,
			chainable: !self.is_redirect_dot(node_id, network_path) && !self.hidden_primary_output(node_id, network_path),
			name_width: 0.,
			locked: self.is_locked(node_id, network_path),
		})
	}
}
