use super::*;
use graph_layout::{Drag, Graph, Layout, Node, Placement, Wire, layout, reach};
use std::cell::RefCell;

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

	/// The reader the node is fitted with and drawn level with, its highest reader as stored.
	pub fn owner(&self, node_id: &NodeId) -> Option<NodeId> {
		let owner = self.layout.owners[*self.indices.get(node_id)? as usize]?;
		Some(self.node_ids[owner as usize])
	}

	/// The point the node's stored offset is taken from: the left of its readers, level with its owner.
	pub fn anchor(&self, node_id: &NodeId) -> Option<IVec2> {
		self.indices.get(node_id).map(|index| self.layout.anchors[*index as usize])
	}

	/// How far the connection to the node's owner is drawn longer than stored.
	pub fn elongation(&self, node_id: &NodeId) -> IVec2 {
		self.indices.get(node_id).map_or(IVec2::ZERO, |index| self.layout.elongations[*index as usize])
	}

	/// How far the node is drawn from where its stored offsets alone place it, which adds up the elongations of every connection to its root.
	pub fn drawn_away_from_stored(&self, node_id: &NodeId) -> IVec2 {
		let mut total = IVec2::ZERO;
		let mut current = self.indices.get(node_id).copied();
		while let Some(index) = current {
			total += self.layout.elongations[index as usize];
			current = self.layout.owners[index as usize];
		}
		total
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

/// The selected nodes being dragged and how far, which the layout draws them moved by until the drag is stored or cancelled.
#[derive(Debug, Clone)]
pub struct DragPreview {
	dragged: Vec<NodeId>,
	offset: IVec2,
	keep_upstream: bool,
	origin: RefCell<Option<DragOrigin>>,
}

/// The network as stored that a drag moves the nodes from, as of one revision, with the rows it has moved up so far.
#[derive(Debug, Clone)]
struct DragOrigin {
	revision: u64,
	keep_upstream: bool,
	node_ids: Vec<NodeId>,
	indices: HashMap<NodeId, u32>,
	start: Graph,
	drag: Drag,
}

impl DragOrigin {
	fn new(node_ids: Vec<NodeId>, indices: HashMap<NodeId, u32>, start: Graph, start_layout: Layout, dragged: &[NodeId], keep_upstream: bool, revision: u64) -> Self {
		let dragged = dragged.iter().filter_map(|node_id| indices.get(node_id).copied()).collect();
		Self {
			revision,
			keep_upstream,
			drag: Drag::with_layout(start.clone(), start_layout, dragged, keep_upstream, layout),
			node_ids,
			indices,
			start,
		}
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

	/// How far the selected nodes are drawn dragged from where the drag started, and whether the nodes they carry along stay in place.
	pub(crate) fn drag_preview(&self, network_path: &[NodeId]) -> (IVec2, bool) {
		self.network_metadata(network_path)
			.and_then(|network_metadata| network_metadata.transient_metadata.drag.as_ref())
			.map_or((IVec2::ZERO, false), |drag| (drag.offset, drag.keep_upstream))
	}

	/// Draws the selected nodes `offset` grid cells from where they were drawn when the drag started, without storing anything until it finishes.
	/// With `keep_upstream`, each node only carried along by them stays where it is drawn.
	pub fn drag_selected_nodes(&mut self, offset: IVec2, keep_upstream: bool, network_path: &[NodeId]) {
		let selected_nodes: Vec<NodeId> = self
			.selected_nodes_in_nested_network(network_path)
			.map(|selected_nodes| selected_nodes.selected_nodes().copied().collect())
			.unwrap_or_default();

		let starting = self.network_metadata(network_path).is_some_and(|network_metadata| network_metadata.transient_metadata.drag.is_none());
		let origin = starting.then(|| self.drag_origin_from_cached_layout(&selected_nodes, keep_upstream, network_path)).flatten();

		let Some(transient) = self.network_transient_mut(network_path) else { return };
		match &mut transient.drag {
			Some(drag) if (drag.offset, drag.keep_upstream) == (offset, keep_upstream) => return,
			Some(drag) => (drag.offset, drag.keep_upstream) = (offset, keep_upstream),
			None => {
				transient.drag = Some(DragPreview {
					dragged: selected_nodes,
					offset,
					keep_upstream,
					origin: RefCell::new(origin),
				})
			}
		}
		transient.layout.unload();
		self.invalidate_positions(network_path);
	}

	/// Moves the selected nodes `offset` grid cells from where they are drawn, the same way as a drag.
	pub fn shift_selected_nodes(&mut self, offset: IVec2, keep_upstream: bool, network_path: &[NodeId]) {
		self.cancel_drag(network_path);
		self.drag_selected_nodes(offset, keep_upstream, network_path);
		self.finish_drag(network_path);
	}

	/// Stores where the drag has moved each node.
	pub fn finish_drag(&mut self, network_path: &[NodeId]) {
		let Some(drag) = self.end_drag(network_path) else { return };
		let moved_placements = self
			.with_drag_origin(&drag, network_path, store::revision(), |origin| {
				let (moved, _) = origin.drag.moved(drag.offset);
				origin
					.start
					.nodes
					.iter()
					.zip(&moved.nodes)
					.zip(&origin.node_ids)
					.filter(|((before, after), _)| before.placement != after.placement)
					.map(|((_, after), node_id)| (*node_id, after.placement))
					.collect::<Vec<_>>()
			})
			.unwrap_or_default();

		for (node_id, placement) in moved_placements {
			self.store_placement(&node_id, placement, network_path);
		}
	}

	/// Draws the nodes where they are stored again, as they were before the drag.
	pub fn cancel_drag(&mut self, network_path: &[NodeId]) {
		self.end_drag(network_path);
	}

	fn end_drag(&mut self, network_path: &[NodeId]) -> Option<DragPreview> {
		let transient = self.network_transient_mut(network_path)?;
		let drag = transient.drag.take()?;
		transient.layout.unload();
		self.invalidate_positions(network_path);
		Some(drag)
	}

	/// Reads where the drag moves the nodes from, starting over from the network as stored if anything was written or changed how it is dragged since.
	fn with_drag_origin<R>(&self, drag: &DragPreview, network_path: &[NodeId], revision: u64, read: impl FnOnce(&mut DragOrigin) -> R) -> Option<R> {
		let mut origin = drag.origin.borrow_mut();
		if origin.as_ref().is_none_or(|origin| (origin.revision, origin.keep_upstream) != (revision, drag.keep_upstream)) {
			let (node_ids, indices, start) = self.layout_graph(network_path)?;
			let start_layout = layout(&start);
			*origin = Some(DragOrigin::new(node_ids, indices, start, start_layout, &drag.dragged, drag.keep_upstream, revision));
		}
		origin.as_mut().map(read)
	}

	/// Where a drag starting now moves the nodes from, taken from the layout cached for the network as stored if it is current.
	fn drag_origin_from_cached_layout(&self, dragged: &[NodeId], keep_upstream: bool, network_path: &[NodeId]) -> Option<DragOrigin> {
		let revision = store::revision();
		let cache = &self.network_metadata(network_path)?.transient_metadata.layout;
		let cached = cache.with_loaded(|cached| (cached.revision == revision).then(|| cached.clone())).flatten()?;

		let NetworkLayout { node_ids, indices, graph, layout, .. } = cached;
		Some(DragOrigin::new(node_ids, indices, graph, layout, dragged, keep_upstream, revision))
	}

	fn store_placement(&mut self, node_id: &NodeId, placement: Placement, network_path: &[NodeId]) {
		let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, network_path)) else {
			log::error!("Could not get node_metadata for node {node_id}");
			return;
		};
		let node_type = match (node.is_layer(), placement) {
			(true, Placement::Absolute(position)) => NodeTypePersistentMetadata::layer(position),
			(true, Placement::Relative(offset)) => NodeTypePersistentMetadata::Layer(LayerPersistentMetadata {
				position: LayerPosition::Relative(offset),
			}),
			(true, Placement::Stack(gap)) => NodeTypePersistentMetadata::Layer(LayerPersistentMetadata {
				position: LayerPosition::stacked(gap),
			}),
			(false, Placement::Absolute(position)) => NodeTypePersistentMetadata::node(position),
			(false, Placement::Relative(offset)) => NodeTypePersistentMetadata::Node(NodePersistentMetadata::new(NodePosition::Relative(offset))),
			(false, Placement::Chain) => NodeTypePersistentMetadata::Node(NodePersistentMetadata::new(NodePosition::Chain)),
			(true, Placement::Chain) | (false, Placement::Stack(_)) => {
				log::error!("Node {node_id} cannot be stored as {placement:?}");
				return;
			}
		};
		if node.set_node_type(node_type) {
			self.transaction_modified();
			self.invalidate_positions(network_path);
		}
	}

	fn lay_out(&self, network_path: &[NodeId], revision: u64) -> Option<NetworkLayout> {
		let (node_ids, indices, graph, layout) = match &self.network_metadata(network_path)?.transient_metadata.drag {
			Some(drag) => self.with_drag_origin(drag, network_path, revision, |origin| {
				let (graph, layout) = origin.drag.moved(drag.offset);
				(origin.node_ids.clone(), origin.indices.clone(), graph, layout)
			})?,
			None => {
				let (node_ids, indices, graph) = self.layout_graph(network_path)?;
				let layout = layout(&graph);
				(node_ids, indices, graph, layout)
			}
		};

		Some(NetworkLayout {
			revision,
			layout,
			graph,
			node_ids,
			indices,
		})
	}

	/// The network as the layout pass reads it, with the ID of each node and the index of each ID.
	pub(crate) fn layout_graph(&self, network_path: &[NodeId]) -> Option<(Vec<NodeId>, HashMap<NodeId, u32>, Graph)> {
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

		Some((node_ids, indices, Graph { nodes, wires }))
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

		let is_redirect_dot = self.is_redirect_dot(node_id, network_path);
		Some(Node {
			placement,
			is_layer,
			rows: self.node_height(node_id, network_path).saturating_sub(1),
			width: if is_layer { self.layer_width(node_id, network_path).unwrap_or_default() } else { 5 },
			inputs: self.number_of_displayed_inputs(node_id, network_path) as u32,
			outputs: self.number_of_outputs(node_id, network_path) as u32,
			chainable: !is_redirect_dot && !self.hidden_primary_output(node_id, network_path),
			waypoint: is_redirect_dot,
			held: false,
			name_width: 0.,
			locked: self.is_locked(node_id, network_path),
		})
	}
}
