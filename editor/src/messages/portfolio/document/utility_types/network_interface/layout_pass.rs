use super::node_parts::{NodeShape, PartTarget, PlacedNode};
use super::*;
use glam::UVec2;
use graph_layout::{Drag, Graph, Layout, Node, NodeIndex, NodeKind, Placement, Wire};
use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};

const REACH_MARGIN: f64 = 16.;

/// Where each node of a network is drawn and which layer's chain it is drawn in, as of one revision of the interface.
#[derive(Debug, Clone)]
pub struct NetworkLayout {
	revision: u64,
	generation: u64,
	node_ids: Vec<NodeId>,
	indices: HashMap<NodeId, NodeIndex>,
	graph: Graph,
	shapes: Vec<NodeShape>,
	layout: Layout,
	bounds: [DVec2; 2],
	clearance: ImportExportClearance,
}

/// Counts every layout made, so whatever is derived from one can tell when it was replaced, even without a new revision.
static LAYOUT_GENERATIONS: AtomicU64 = AtomicU64::new(0);

impl NetworkLayout {
	fn new(revision: u64, laid_out: LaidOutNodes, graph: Graph, layout: Layout) -> Self {
		let LaidOutNodes { node_ids, indices, shapes } = laid_out;
		let mut network_layout = Self {
			revision,
			generation: LAYOUT_GENERATIONS.fetch_add(1, Ordering::Relaxed),
			node_ids,
			indices,
			graph,
			shapes,
			layout,
			bounds: [DVec2::ZERO; 2],
			clearance: ImportExportClearance::default(),
		};
		network_layout.measure_bounds_and_clearance();
		network_layout
	}

	fn into_parts(self) -> (LaidOutNodes, Graph, Layout) {
		let Self {
			node_ids,
			indices,
			shapes,
			graph,
			layout,
			..
		} = self;
		(LaidOutNodes { node_ids, indices, shapes }, graph, layout)
	}
.
	pub fn generation(&self) -> u64 {
		self.generation
	}

	pub fn position(&self, node_id: &NodeId) -> Option<IVec2> {
		Some(self.node(node_id)?.position)
	}

	/// The layer whose chain the node is drawn in.
	pub fn chain_layer(&self, node_id: &NodeId) -> Option<NodeId> {
		Some(self.node_id(self.node(node_id)?.chain_layer?))
	}

	/// The reader the node is fitted with and drawn level with, its highest reader as stored.
	pub fn owner(&self, node_id: &NodeId) -> Option<NodeId> {
		Some(self.node_id(self.node(node_id)?.owner?))
	}

	/// The point the node's stored offset is taken from: the left of its readers, level with its owner.
	pub fn anchor(&self, node_id: &NodeId) -> Option<IVec2> {
		Some(self.node(node_id)?.anchor)
	}

	/// How far the connection to the node's owner is drawn longer than stored.
	pub fn elongation(&self, node_id: &NodeId) -> IVec2 {
		self.node(node_id).map_or(IVec2::ZERO, |node| node.elongation)
	}

	/// How far the node is drawn from where its stored offsets alone place it, which adds up the elongations of every connection to its root.
	pub fn drawn_away_from_stored(&self, node_id: &NodeId) -> IVec2 {
		let first = self.indices.get(node_id).copied();
		std::iter::successors(first, |index| self.layout.node(*index).owner)
			.map(|index| self.layout.node(index).elongation)
			.sum()
	}

	/// How many nodes the layer's chain holds.
	pub fn chain_length(&self, layer: &NodeId) -> u32 {
		self.node(layer).map_or(0, |node| node.chain_length)
	}

	/// The combined bounds of every node's body in node graph space, or a point at the origin for an empty network.
	pub fn bounds(&self) -> [DVec2; 2] {
		self.bounds
	}

	/// What the imports and exports keep their distance from horizontally.
	pub(crate) fn clearance(&self) -> ImportExportClearance {
		self.clearance
	}

	/// The node's body where it sits in its grid cell, in node graph space.
	pub fn body(&self, node_id: &NodeId) -> Option<PartTarget> {
		Some(self.placed(*self.indices.get(node_id)?).body())
	}

	/// The first of the node's parts, its body last, that `find` picks something from.
	pub fn find_part<R>(&self, node_id: &NodeId, mut find: impl FnMut(&PartTarget) -> Option<R>) -> Option<R> {
		let placed = self.placed(*self.indices.get(node_id)?);
		let mut found = None;
		placed.parts(|target| {
			if found.is_none() {
				found = find(&target);
			}
		});
		found
	}

	/// Whether `point`, in node graph space, lies within the box around everything the node draws.
	pub fn reaches(&self, node_id: &NodeId, point: DVec2) -> bool {
		let Some(index) = self.indices.get(node_id) else { return false };
		let reach = self.layout.reach(&self.graph, *index);
		let min = reach.min.as_dvec2() * GRID_SIZE as f64 - REACH_MARGIN;
		let max = reach.max.as_dvec2() * GRID_SIZE as f64 + REACH_MARGIN;
		point.cmpge(min).all() && point.cmplt(max).all()
	}

	fn placed(&self, index: NodeIndex) -> PlacedNode<'_> {
		let node_layout = self.layout.node(index);
		PlacedNode {
			node: &self.graph[index],
			shape: &self.shapes[index.index()],
			position: node_layout.position,
			chain_length: node_layout.chain_length,
		}
	}

	/// Measures the combined bounds of the node bodies, and the horizontal extents the imports and exports keep clear of, with redirect dots measured by their centers.
	fn measure_bounds_and_clearance(&mut self) {
		let widen = |span: Option<[f64; 2]>, left: f64, right: f64| Some(span.map_or([left, right], |[min, max]| [min.min(left), max.max(right)]));

		let mut bounds: Option<[DVec2; 2]> = None;
		let mut clearance = ImportExportClearance::default();
		for index in (0..self.graph.nodes.len()).map(|index| NodeIndex::from(index as u32)) {
			let body = self.placed(index).body();
			bounds = Some(bounds.map_or([body.min, body.max], |[min, max]| [min.min(body.min), max.max(body.max)]));

			if self.graph[index].kind == NodeKind::Waypoint {
				let center = (body.min.x + body.max.x) / 2.;
				clearance.dot_centers = widen(clearance.dot_centers, center, center);
			} else {
				clearance.node_sides = widen(clearance.node_sides, body.min.x, body.max.x);
			}
		}

		self.bounds = bounds.unwrap_or_default();
		self.clearance = clearance;
	}

	fn node(&self, node_id: &NodeId) -> Option<graph_layout::NodeLayout> {
		Some(self.layout.node(*self.indices.get(node_id)?))
	}

	fn node_id(&self, index: NodeIndex) -> NodeId {
		self.node_ids[index.index()]
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

#[derive(Debug, Clone)]
struct DragOrigin {
	revision: u64,
	keep_upstream: bool,
	node_ids: Vec<NodeId>,
	indices: HashMap<NodeId, NodeIndex>,
	shapes: Vec<NodeShape>,
	start: Graph,
	drag: Drag,
}

impl DragOrigin {
	fn new(laid_out: LaidOutNodes, start: Graph, start_layout: Layout, dragged: &[NodeId], keep_upstream: bool, revision: u64) -> Self {
		let LaidOutNodes { node_ids, indices, shapes } = laid_out;
		let dragged = dragged.iter().filter_map(|node_id| indices.get(node_id).copied()).collect();
		Self {
			revision,
			keep_upstream,
			drag: Drag::new(start.clone(), start_layout, dragged, keep_upstream),
			node_ids,
			indices,
			shapes,
			start,
		}
	}
}

impl NodeNetworkInterface {
	pub(crate) fn with_layout<R>(&self, network_path: &[NodeId], read: impl FnOnce(&NetworkLayout) -> R) -> Option<R> {
		let cache = &self.network_metadata(network_path)?.transient_metadata.layout;
		let revision = store::revision();
		if cache.with_loaded(|layout| layout.revision != revision) == Some(true) {
			cache.unload();
		}

		cache.with_loaded_or(|| self.lay_out(network_path, revision), read)
	}

	pub(crate) fn nodes_reaching(&self, point: DVec2, network_path: &[NodeId]) -> Vec<NodeId> {
		let glide_offsets = self.glide_offsets(network_path);
		self.with_layout(network_path, |layout| {
			layout.node_ids.iter().filter(|node_id| layout.reaches(node_id, point - glide_offsets.node(node_id))).copied().collect()
		})
		.unwrap_or_default()
	}

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
		let finished = self.with_drag_origin(&drag, network_path, store::revision(), |origin| {
			let moved = origin.drag.released(drag.offset);
			let moved_placements: Vec<(NodeId, Placement)> = origin
				.start
				.nodes
				.iter()
				.zip(&moved.nodes)
				.zip(&origin.node_ids)
				.filter(|((before, after), _)| before.placement != after.placement)
				.map(|((_, after), node_id)| (*node_id, after.placement))
				.collect();
			let laid_out = LaidOutNodes {
				node_ids: std::mem::take(&mut origin.node_ids),
				indices: std::mem::take(&mut origin.indices),
				shapes: std::mem::take(&mut origin.shapes),
			};
			(moved_placements, laid_out, moved)
		});
		let Some((moved_placements, laid_out, graph)) = finished else { return };

		let mut stored_all = true;
		for (node_id, placement) in moved_placements {
			stored_all &= self.store_placement(&node_id, placement, network_path);
		}

		if stored_all && let Some(network_metadata) = self.network_metadata(network_path) {
			let layout = Layout::new(&graph);
			network_metadata.transient_metadata.layout.store(NetworkLayout::new(store::revision(), laid_out, graph, layout));
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
		Some(drag)
	}

	fn with_drag_origin<R>(&self, drag: &DragPreview, network_path: &[NodeId], revision: u64, read: impl FnOnce(&mut DragOrigin) -> R) -> Option<R> {
		let mut origin = drag.origin.borrow_mut();
		if origin.as_ref().is_none_or(|origin| (origin.revision, origin.keep_upstream) != (revision, drag.keep_upstream)) {
			let (laid_out, start) = self.layout_graph(network_path)?;
			let start_layout = Layout::new(&start);
			*origin = Some(DragOrigin::new(laid_out, start, start_layout, &drag.dragged, drag.keep_upstream, revision));
		}
		origin.as_mut().map(read)
	}

	fn drag_origin_from_cached_layout(&self, dragged: &[NodeId], keep_upstream: bool, network_path: &[NodeId]) -> Option<DragOrigin> {
		let revision = store::revision();
		let cache = &self.network_metadata(network_path)?.transient_metadata.layout;
		let cached = cache.with_loaded(|cached| (cached.revision == revision).then(|| cached.clone())).flatten()?;

		let (laid_out, graph, layout) = cached.into_parts();
		Some(DragOrigin::new(laid_out, graph, layout, dragged, keep_upstream, revision))
	}

	fn store_placement(&mut self, node_id: &NodeId, placement: Placement, network_path: &[NodeId]) -> bool {
		let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, network_path)) else {
			log::error!("Could not get node_metadata for node {node_id}");
			return false;
		};
		let node_type = match (node.is_layer(), placement) {
			(true, Placement::Absolute(position)) => NodeTypePersistentMetadata::layer(position),
			(true, Placement::Relative(offset)) => NodeTypePersistentMetadata::Layer(LayerPersistentMetadata {
				position: LayerPosition::Relative(offset),
			}),
			(false, Placement::Absolute(position)) => NodeTypePersistentMetadata::node(position),
			(false, Placement::Relative(offset)) => NodeTypePersistentMetadata::Node(NodePersistentMetadata::new(NodePosition::Relative(offset))),
		};
		if node.set_node_type(node_type) {
			self.transaction_modified();
		}
		true
	}

	fn lay_out(&self, network_path: &[NodeId], revision: u64) -> Option<NetworkLayout> {
		let (laid_out, graph, layout) = match &self.network_metadata(network_path)?.transient_metadata.drag {
			Some(drag) => self.with_drag_origin(drag, network_path, revision, |origin| {
				let (graph, mut layout) = origin.drag.moved(drag.offset);
				layout.snap_chains(&graph);
				let laid_out = LaidOutNodes {
					node_ids: origin.node_ids.clone(),
					indices: origin.indices.clone(),
					shapes: origin.shapes.clone(),
				};
				(laid_out, graph, layout)
			})?,
			None => {
				let (laid_out, graph) = self.layout_graph(network_path)?;
				let layout = Layout::new(&graph);
				(laid_out, graph, layout)
			}
		};

		Some(NetworkLayout::new(revision, laid_out, graph, layout))
	}

	pub(crate) fn layout_graph(&self, network_path: &[NodeId]) -> Option<(LaidOutNodes, Graph)> {
		let network = self.nested_network(network_path)?;
		let node_ids: Vec<NodeId> = network.nodes.keys().copied().collect();
		let indices: HashMap<NodeId, NodeIndex> = node_ids.iter().enumerate().map(|(index, node_id)| (*node_id, NodeIndex::from(index as u32))).collect();

		let (nodes, shapes) = node_ids.iter().map(|node_id| self.layout_node(node_id, network_path)).collect::<Option<(Vec<_>, Vec<_>)>>()?;
		let wires = node_ids
			.iter()
			.enumerate()
			.flat_map(|(target, node_id)| {
				let indices = &indices;
				network.nodes[node_id].inputs.iter().enumerate().filter_map(move |(input, node_input)| match node_input {
					NodeInput::Node { node_id, output_index, .. } => Some(Wire {
						source: *indices.get(node_id)?,
						output: *output_index as u32,
						target: NodeIndex::from(target as u32),
						input: input as u32,
					}),
					_ => None,
				})
			})
			.collect();

		Some((LaidOutNodes { node_ids, indices, shapes }, Graph { nodes, wires }))
	}

	fn layout_node(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<(Node, NodeShape)> {
		let metadata = self.node_metadata(node_id, network_path)?;
		let (placement, is_layer) = match &metadata.persistent_metadata.node_type_metadata {
			NodeTypePersistentMetadata::Layer(layer) => match layer.position {
				LayerPosition::Absolute(position) => (Placement::Absolute(position), true),
				LayerPosition::Relative(offset) => (Placement::Relative(offset), true),
			},
			NodeTypePersistentMetadata::Node(node) => match *node.position() {
				NodePosition::Absolute(position) => (Placement::Absolute(position), false),
				NodePosition::Relative(offset) => (Placement::Relative(offset), false),
			},
		};

		let is_redirect_dot = self.is_redirect_dot(node_id, network_path);
		let (layer_width, name_width) = if is_layer {
			self.layer_width_and_name_width(node_id, network_path).unwrap_or_default()
		} else {
			(0, 0.)
		};
		let (kind, size) = match (is_layer, is_redirect_dot) {
			(true, _) => (NodeKind::Layer, UVec2::new(layer_width, LAYER_GRID_HEIGHT)),
			(false, true) => (NodeKind::Waypoint, UVec2::new(5, self.node_height(node_id, network_path))),
			(false, false) => (NodeKind::Node, UVec2::new(5, self.node_height(node_id, network_path))),
		};

		let primary_output_hidden = self.hidden_primary_output(node_id, network_path);
		let node = Node {
			placement,
			kind,
			size: size.as_ivec2(),
			chainable: !is_redirect_dot && !primary_output_hidden,
			held: false,
		};
		let shape = NodeShape {
			inputs: self.number_of_displayed_inputs(node_id, network_path) as u32,
			outputs: self.number_of_outputs(node_id, network_path) as u32,
			primary_input_hidden: self.document_node(node_id, network_path)?.inputs.first().is_some_and(|input| !input.is_exposed()),
			primary_output_hidden,
			name_width,
			locked: self.is_locked(node_id, network_path),
		};
		Some((node, shape))
	}
}

/// The nodes of a network in the order the layout indexes them, with what each draws beyond its size in the layout.
pub(crate) struct LaidOutNodes {
	pub(crate) node_ids: Vec<NodeId>,
	pub(crate) indices: HashMap<NodeId, NodeIndex>,
	shapes: Vec<NodeShape>,
}
