use super::*;
use graphene_std::ParameterRef;
use graphene_std::vector::misc::dvec2_to_point;
use kurbo::{DEFAULT_ACCURACY, Shape};

/// Axis-aligned rectangle spanning the two opposite corners, used for node graph chrome click targets.
pub fn rectangle_path(corner1: DVec2, corner2: DVec2) -> BezPath {
	kurbo::Rect::from_points(dvec2_to_point(corner1), dvec2_to_point(corner2)).to_path(DEFAULT_ACCURACY)
}

/// Same as [`rectangle_path`] but with per-corner radii ordered top left, top right, bottom right, bottom left.
pub fn rounded_rectangle_path(corner1: DVec2, corner2: DVec2, radii: [f64; 4]) -> BezPath {
	let rect = kurbo::Rect::from_points(dvec2_to_point(corner1), dvec2_to_point(corner2));
	if radii.iter().all(|radius| *radius == 0.) {
		return rect.to_path(DEFAULT_ACCURACY);
	}

	let [top_left, top_right, bottom_right, bottom_left] = radii;
	kurbo::RoundedRect::from_rect(rect, kurbo::RoundedRectRadii::new(top_left, top_right, bottom_right, bottom_left)).to_path(DEFAULT_ACCURACY)
}

/// Ellipse inscribed in the box spanning the two opposite corners.
pub fn ellipse_path(corner1: DVec2, corner2: DVec2) -> BezPath {
	let rect = kurbo::Rect::from_points(dvec2_to_point(corner1), dvec2_to_point(corner2));
	let mut path = kurbo::Ellipse::new(rect.center(), (rect.width() / 2., rect.height() / 2.), 0.).to_path(DEFAULT_ACCURACY);

	// Kurbo emits the ellipse as an unclosed 360 degree arc, but hit testing only fills explicitly closed contours
	path.close_path();
	path
}

/// A copy of a node from a layer's chain, which can be inserted into a chain in this or another document.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ChainNodeCopy {
	/// The node that was copied.
	pub source: NodeId,
	pub template: NodeTemplate,
	/// The value of each input wired to another node, which the copy takes where that node is missing.
	pub fallback_values: Vec<(usize, NodeInput)>,
}

impl ChainNodeCopy {
	/// Whether the copied node is a redirect dot, which comes along only to bend a wire between the other copies.
	pub fn is_redirect_dot(&self) -> bool {
		let passthrough = matches!(&self.template.implementation, NodeTemplateImplementation::ProtoNode(identifier) if *identifier == graphene_std::ops::passthrough::IDENTIFIER);
		passthrough && matches!(self.template.node_type_metadata, NodeTypePersistentMetadata::Node(_))
	}

	/// The copied node, placed offset from its original, for pasting outside any flow along with the copies of the nodes in `pasted`.
	/// Its inputs stay wired to those copies, and each of its other wired inputs takes its fallback value.
	pub fn into_loose_template(self, pasted: &HashSet<NodeId>) -> NodeTemplate {
		let mut template = self.template;
		for (index, fallback_value) in self.fallback_values {
			if let Some(input) = template.inputs.get_mut(index)
				&& !input.as_node().is_some_and(|node_id| pasted.contains(&node_id))
			{
				*input = fallback_value;
			}
		}

		if let NodeTypePersistentMetadata::Node(NodePersistentMetadata {
			position: NodePosition::Absolute(position),
		}) = &mut template.node_type_metadata
		{
			*position += IVec2::new(2, 2);
		}

		template
	}
}

/// Where copied nodes go when inserted into a layer's chain.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ChainInsertionPoint {
	/// Beside a node in a layer's chain, just downstream of it.
	BesideNode(NodeId),
	/// At the downstream end of a layer's chain, beside the layer.
	BesideLayer(NodeId),
}

#[derive(PartialEq)]
pub enum FlowType {
	/// Iterate over all upstream nodes (inclusive) from every input (the primary and all secondary).
	UpstreamFlow,
	/// Iterate over nodes (inclusive) connected to the primary input.
	PrimaryFlow,
	/// Iterate over the secondary input (inclusive) for layer nodes and primary input for non layer nodes.
	HorizontalFlow,
	/// Same as horizontal flow, but only iterates over connections to primary outputs
	HorizontalPrimaryOutputFlow,
	/// Upstream flow starting from the either the node (inclusive) or secondary input of the layer (not inclusive).
	LayerChildrenUpstreamFlow,
}
/// Iterate over upstream nodes. The behavior changes based on the `flow_type` that's set.
/// - [`FlowType::UpstreamFlow`]: iterates over all upstream nodes from every input (the primary and all secondary).
/// - [`FlowType::PrimaryFlow`]: iterates along the horizontal inputs of nodes, so in the case of a node chain `a -> b -> c`, this would yield `c, b, a` if we started from `c`.
/// - [`FlowType::HorizontalFlow`]: iterates over the secondary input for layer nodes and primary input for non layer nodes.
/// - [`FlowType::LayerChildrenUpstreamFlow`]: iterates over all upstream nodes from the secondary input of the node.
pub(crate) struct FlowIter<'a> {
	pub(crate) stack: Vec<NodeId>,
	pub(crate) network: &'a NodeNetwork,
	pub(crate) network_metadata: &'a NodeNetworkMetadata,
	pub(crate) flow_type: FlowType,
}
impl Iterator for FlowIter<'_> {
	type Item = NodeId;
	fn next(&mut self) -> Option<Self::Item> {
		loop {
			let node_id = self.stack.pop()?;

			if let (Some(document_node), Some(node_metadata)) = (self.network.nodes.get(&node_id), self.network_metadata.persistent_metadata.node_metadata.get(&node_id)) {
				let skip = if matches!(self.flow_type, FlowType::HorizontalFlow | FlowType::HorizontalPrimaryOutputFlow) && node_metadata.persistent_metadata.is_layer() {
					1
				} else {
					0
				};
				let take = if self.flow_type == FlowType::UpstreamFlow { u32::MAX as usize } else { 1 };
				let inputs = document_node.inputs.iter().skip(skip).take(take);

				let node_ids = inputs.filter_map(|input| match input {
					NodeInput::Node { output_index, .. } if self.flow_type == FlowType::HorizontalPrimaryOutputFlow && *output_index != 0 => None,
					NodeInput::Node { node_id, .. } => Some(node_id),
					_ => None,
				});

				self.stack.extend(node_ids);

				return Some(node_id);
			}
		}
	}
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ImportOrExport {
	Import(usize),
	Export(usize),
}

/// The primary input (index 0) of any node: a chain node's horizontal wire continuing from the left, or a layer's vertical stack wire from the sibling rendered below it.
pub const PRIMARY_INPUT_INDEX: usize = 0;
/// The secondary input (index 1) of a layer-shaped node: the horizontal wire from the left, carrying the node chain or child stack that the layer renders.
pub const LAYER_SECONDARY_INPUT_INDEX: usize = 1;
/// The height in grid cells of a node displayed as a layer, which is fixed regardless of its inputs.
pub const LAYER_GRID_HEIGHT: u32 = 2;

/// The primary output (index 0) of a node, which most nodes expose as their only output.
pub const PRIMARY_OUTPUT_INDEX: usize = 0;

/// Represents an input connector with index based on the [`DocumentNode::inputs`] index, not the visible input index
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum InputConnector {
	#[serde(rename = "node")]
	Node {
		#[serde(rename = "nodeId")]
		node_id: NodeId,
		#[serde(rename = "inputIndex")]
		input_index: usize,
	},
	#[serde(rename = "export")]
	Export(usize),
}

impl Default for InputConnector {
	fn default() -> Self {
		InputConnector::Export(0)
	}
}

impl InputConnector {
	/// Reference a node's input by its parameter symbol, e.g. `InputConnector::node(node_id, stroke::WeightInput)`, or by an erased [`ParameterRef`] chosen at runtime.
	pub fn node(node_id: NodeId, parameter: impl Into<ParameterRef>) -> Self {
		InputConnector::Node {
			node_id,
			input_index: parameter.into().input_index,
		}
	}

	/// Reference a node's primary input: a chain node's continuation from the left, or a layer's stack wire from the bottom.
	pub fn primary_input(node_id: NodeId) -> Self {
		InputConnector::Node {
			node_id,
			input_index: PRIMARY_INPUT_INDEX,
		}
	}

	/// Reference a layer-shaped node's secondary input, the wire from the left carrying the content that the layer renders.
	pub fn layer_secondary_input(node_id: NodeId) -> Self {
		InputConnector::Node {
			node_id,
			input_index: LAYER_SECONDARY_INPUT_INDEX,
		}
	}

	/// Reference a node's input by a runtime index, for genuinely dynamic cases like clicked ports, input enumeration, and document upgrades.
	pub fn node_at_index(node_id: NodeId, input_index: usize) -> Self {
		InputConnector::Node { node_id, input_index }
	}

	pub fn input_index(&self) -> usize {
		match self {
			InputConnector::Node { input_index, .. } => *input_index,
			InputConnector::Export(input_index) => *input_index,
		}
	}

	pub fn node_id(&self) -> Option<NodeId> {
		match self {
			InputConnector::Node { node_id, .. } => Some(*node_id),
			_ => None,
		}
	}
}

/// Represents an output connector
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OutputConnector {
	#[serde(rename = "node")]
	Node {
		#[serde(rename = "nodeId")]
		node_id: NodeId,
		#[serde(rename = "outputIndex")]
		output_index: usize,
	},
	#[serde(rename = "import")]
	Import(usize),
}

impl Default for OutputConnector {
	fn default() -> Self {
		OutputConnector::Import(0)
	}
}

impl OutputConnector {
	pub fn node(node_id: NodeId, output_index: usize) -> Self {
		OutputConnector::Node { node_id, output_index }
	}

	/// Reference a node's primary (first) output.
	pub fn primary_output(node_id: NodeId) -> Self {
		OutputConnector::Node {
			node_id,
			output_index: PRIMARY_OUTPUT_INDEX,
		}
	}

	pub fn index(&self) -> usize {
		match self {
			OutputConnector::Node { output_index, .. } => *output_index,
			OutputConnector::Import(output_index) => *output_index,
		}
	}

	pub fn node_id(&self) -> Option<NodeId> {
		match self {
			OutputConnector::Node { node_id, .. } => Some(*node_id),
			_ => None,
		}
	}

	pub fn from_input(input: &NodeInput) -> Option<Self> {
		match input {
			NodeInput::Import { import_index, .. } => Some(Self::Import(*import_index)),
			NodeInput::Node { node_id, output_index, .. } => Some(Self::node(*node_id, *output_index)),
			_ => None,
		}
	}
}

/// Which side of a redirect dot an edit took a wire from, since each side leaves the dot orphaned only once it has no wire left there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DotSide {
	/// The wire feeding the dot.
	Incoming,
	/// One of the wires the dot feeds.
	Outgoing,
}

/// The corners of the click target for a connector on a node or layer's left or right side, or an import or export.
/// It fills its row's height and reaches further away from the node than into it, leaving the node body grabbable.
pub(crate) fn side_port_bounds(center: DVec2, is_input: bool) -> [DVec2; 2] {
	const INWARD_REACH: f64 = 8.;
	const OUTWARD_REACH: f64 = 12.;
	const HALF_HEIGHT: f64 = 12.;

	// Inputs face left and outputs face right
	let (left_reach, right_reach) = if is_input { (OUTWARD_REACH, INWARD_REACH) } else { (INWARD_REACH, OUTWARD_REACH) };
	[center - DVec2::new(left_reach, HALF_HEIGHT), center + DVec2::new(right_reach, HALF_HEIGHT)]
}

/// The corners of the click target for a layer's top output or bottom input, which reaches only 8 px toward the connector it faces in a stack.
fn stack_port_bounds(center: DVec2) -> [DVec2; 2] {
	const HALF_SIZE: DVec2 = DVec2::new(12., 8.);

	[center - HALF_SIZE, center + HALF_SIZE]
}

pub(crate) fn port_click_target([corner1, corner2]: [DVec2; 2]) -> ClickTarget {
	const CORNER_RADIUS: f64 = 4.;

	let path = rounded_rectangle_path(corner1, corner2, [CORNER_RADIUS; 4]);
	ClickTarget::new_with_path(path, 0.)
}

#[derive(Debug, Clone)]
pub struct Ports {
	pub(crate) input_ports: Vec<(usize, ClickTarget)>,
	pub(crate) output_ports: Vec<(usize, ClickTarget)>,
	/// Where a wire attaches to an input port with a click target not centered on that point.
	pub(crate) input_anchors: Vec<(usize, DVec2)>,
	/// Where a wire attaches to an output port with a click target not centered on that point.
	pub(crate) output_anchors: Vec<(usize, DVec2)>,
}

impl Default for Ports {
	fn default() -> Self {
		Self::new()
	}
}

impl Ports {
	pub fn new() -> Ports {
		Ports {
			input_ports: Vec::new(),
			output_ports: Vec::new(),
			input_anchors: Vec::new(),
			output_anchors: Vec::new(),
		}
	}

	pub fn click_targets(&self) -> impl Iterator<Item = &ClickTarget> {
		self.input_ports
			.iter()
			.map(|(_, click_target)| click_target)
			.chain(self.output_ports.iter().map(|(_, click_target)| click_target))
	}

	pub fn input_ports(&self) -> impl Iterator<Item = &(usize, ClickTarget)> {
		self.input_ports.iter()
	}

	pub fn output_ports(&self) -> impl Iterator<Item = &(usize, ClickTarget)> {
		self.output_ports.iter()
	}

	pub(crate) fn insert_input_port_at_center(&mut self, input_index: usize, center: DVec2) {
		self.insert_custom_input_port(input_index, port_click_target(side_port_bounds(center, true)));
		self.input_anchors.push((input_index, center));
	}

	pub(crate) fn insert_custom_input_port(&mut self, input_index: usize, click_target: ClickTarget) {
		self.input_ports.push((input_index, click_target));
	}

	pub(crate) fn insert_output_port_at_center(&mut self, output_index: usize, center: DVec2) {
		self.insert_custom_output_port(output_index, port_click_target(side_port_bounds(center, false)));
		self.output_anchors.push((output_index, center));
	}

	pub(crate) fn insert_custom_output_port(&mut self, output_index: usize, click_target: ClickTarget) {
		self.output_ports.push((output_index, click_target));
	}

	pub(crate) fn insert_node_input(&mut self, input_index: usize, row_index: usize, node_top_left: DVec2) {
		// The center of the click target is always 24 px down from the top left corner of the node
		let center = node_top_left + DVec2::new(0., 24. + 24. * row_index as f64);
		self.insert_input_port_at_center(input_index, center);
	}

	pub(crate) fn insert_node_output(&mut self, output_index: usize, node_top_left: DVec2) {
		// The center of the click target is always 24 px down from the top left corner of the node
		let center = node_top_left + DVec2::new(5. * 24., 24. + 24. * output_index as f64);
		self.insert_output_port_at_center(output_index, center);
	}

	pub(crate) fn insert_layer_input(&mut self, input_index: usize, node_top_left: DVec2) {
		// The primary input sits beneath the layer's thumbnail, and the secondary input on its left side
		if input_index == 0 {
			let center = node_top_left + DVec2::new(2. * 24., 24. * 2. + 8.);
			self.insert_custom_input_port(input_index, port_click_target(stack_port_bounds(center)));
		} else {
			self.insert_input_port_at_center(input_index, node_top_left + DVec2::new(0., 24. * 1.));
		}
	}

	pub(crate) fn insert_layer_output(&mut self, node_top_left: DVec2) {
		// The center of the click target is 8 px above the layer, over its thumbnail
		let center = node_top_left + DVec2::new(2. * 24., -8.);
		self.insert_custom_output_port(0, port_click_target(stack_port_bounds(center)));
	}

	pub fn clicked_input_port_from_point(&self, point: DVec2) -> Option<usize> {
		self.input_ports.iter().find_map(|(port, click_target)| click_target.intersect_point_no_stroke(point).then_some(*port))
	}

	pub fn clicked_output_port_from_point(&self, point: DVec2) -> Option<usize> {
		self.output_ports.iter().find_map(|(port, click_target)| click_target.intersect_point_no_stroke(point).then_some(*port))
	}

	/// Adds the input and output of a redirect dot. Its wires both attach at its center, but they're grabbed from the gutters on either side of its body.
	pub(crate) fn insert_dot_ports(&mut self, center: DVec2) {
		const GUTTER_OFFSET: f64 = 10.;
		const GUTTER_RADIUS: f64 = 5.;

		let gutter = |x_offset: f64| {
			let gutter_center = center + DVec2::new(x_offset, 0.);
			ClickTarget::new_with_path(ellipse_path(gutter_center - DVec2::splat(GUTTER_RADIUS), gutter_center + DVec2::splat(GUTTER_RADIUS)), 0.)
		};
		self.insert_custom_input_port(0, gutter(-GUTTER_OFFSET));
		self.insert_custom_output_port(0, gutter(GUTTER_OFFSET));
		self.input_anchors.push((0, center));
		self.output_anchors.push((0, center));
	}

	pub fn input_port_position(&self, index: usize) -> Option<DVec2> {
		if let Some((_, anchor)) = self.input_anchors.iter().find(|(port_index, _)| *port_index == index) {
			return Some(*anchor);
		}
		self.input_ports
			.iter()
			.find_map(|(port_index, click_target)| if *port_index == index { click_target.bounding_box_center() } else { None })
	}

	pub fn output_port_position(&self, index: usize) -> Option<DVec2> {
		if let Some((_, anchor)) = self.output_anchors.iter().find(|(port_index, _)| *port_index == index) {
			return Some(*anchor);
		}
		self.output_ports
			.iter()
			.find_map(|(port_index, click_target)| if *port_index == index { click_target.bounding_box_center() } else { None })
	}
}

#[derive(PartialEq, Debug, Clone, Copy, Hash, Default, serde::Serialize, serde::Deserialize)]
pub struct RootNode {
	pub node_id: NodeId,
	pub output_index: usize,
}

impl RootNode {
	pub fn to_connector(&self) -> OutputConnector {
		OutputConnector::Node {
			node_id: self.node_id,
			output_index: self.output_index,
		}
	}
}

#[derive(PartialEq, Debug, Clone, Copy, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum Previewing {
	/// If there is a node to restore the connection to the export for, then it is stored in the option.
	/// Otherwise, nothing gets restored and the primary export is disconnected.
	Yes { root_node_to_restore: Option<RootNode> },
	#[default]
	No,
}

/// All fields in NetworkMetadata should automatically be updated by using the network interface API. If a field is none then it should be calculated based on the network state.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct NodeNetworkMetadata {
	pub persistent_metadata: NodeNetworkPersistentMetadata,
	#[serde(skip)]
	pub transient_metadata: NodeNetworkTransientMetadata,
}

impl Clone for NodeNetworkMetadata {
	fn clone(&self) -> Self {
		NodeNetworkMetadata {
			persistent_metadata: self.persistent_metadata.clone(),
			transient_metadata: Default::default(),
		}
	}
}

impl PartialEq for NodeNetworkMetadata {
	fn eq(&self, other: &Self) -> bool {
		self.persistent_metadata == other.persistent_metadata
	}
}

impl NodeNetworkMetadata {
	pub fn nested_metadata(&self, nested_path: &[NodeId]) -> Option<&Self> {
		let mut network_metadata = Some(self);

		for segment in nested_path {
			network_metadata = network_metadata
				.and_then(|network| network.persistent_metadata.node_metadata.get(segment))
				.and_then(|node| node.persistent_metadata.network_metadata.as_ref());
		}
		network_metadata
	}

	/// Get the mutable nested network given by the path of node ids
	pub fn nested_metadata_mut(&mut self, nested_path: &[NodeId]) -> Option<&mut Self> {
		let mut network_metadata = Some(self);

		for segment in nested_path {
			network_metadata = network_metadata
				.and_then(|network: &mut NodeNetworkMetadata| network.persistent_metadata.node_metadata.get_mut(segment))
				.and_then(|node| node.persistent_metadata.network_metadata.as_mut());
		}
		network_metadata
	}
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NodeNetworkPersistentMetadata {
	/// The identifier for the node definition created for custom network nodes in [`DocumentNodeDefinition`].
	/// It is only used to associate network nodes with their definition. Protonodes use their ProtonodeIdentifier.
	/// The reference is removed once the node is modified, since the node now stores its own implementation and inputs.
	/// TODO: Used during serialization/deserialization to prevent storing implementation or inputs (and possible other fields) if they are the same as the definition.
	/// TODO: Implement node versioning so that references to old nodes can be updated to the new node definition.
	pub reference: Option<String>,
	/// Node metadata must exist for every document node in the network
	#[serde(serialize_with = "graphene_std::vector::serialize_hashmap", deserialize_with = "graphene_std::vector::deserialize_hashmap")]
	pub node_metadata: HashMap<NodeId, DocumentNodeMetadata>,
	/// The display order of pinned nodes in the Inspector (shown when nothing is selected in this network), keyed by node ID.
	#[serde(default)]
	pub pinned_node_order: Vec<NodeId>,
	/// Cached metadata for each node, which is calculated when adding a node to node_metadata
	/// Indicates whether the network is currently rendered with a particular node that is previewed, and if so, which connection should be restored when the preview ends.
	pub previewing: Previewing,
	// Stores the transform and navigation state for the network
	pub navigation_metadata: NavigationMetadata,
	/// Stack of selection snapshots for previous history states. Session state that is not persisted into saved documents.
	#[serde(skip)]
	pub selection_undo_history: VecDeque<SelectedNodes>,
	/// Stack of selection snapshots for future history states.
	#[serde(skip)]
	pub selection_redo_history: VecDeque<SelectedNodes>,
}

/// A lazily computed cache slot whose load and read paths work through &self, with interior mutability guarding the stored value.
#[derive(Debug, Clone)]
pub(crate) struct TransientCache<T>(std::cell::RefCell<Option<T>>);

impl<T> Default for TransientCache<T> {
	fn default() -> Self {
		TransientCache(std::cell::RefCell::new(None))
	}
}

impl<T> TransientCache<T> {
	pub(crate) fn is_loaded(&self) -> bool {
		self.0.borrow().is_some()
	}

	pub(crate) fn store(&self, value: T) {
		*self.0.borrow_mut() = Some(value);
	}

	pub(crate) fn unload(&self) {
		*self.0.borrow_mut() = None;
	}

	/// Computes and stores the value with `load` if it is not already loaded.
	///
	/// `load` runs with no borrow held, so it is free to read other cache slots.
	pub(crate) fn ensure_loaded(&self, load: impl FnOnce() -> Option<T>) {
		let already_loaded = self.0.borrow().is_some();
		if already_loaded {
			return;
		}
		let Some(value) = load() else { return };
		self.store(value);
	}

	/// Runs `read` on the cached value, computing it with `load` first if it is not loaded.
	pub(crate) fn with_loaded_or<R>(&self, load: impl FnOnce() -> Option<T>, read: impl FnOnce(&T) -> R) -> Option<R> {
		self.ensure_loaded(load);
		self.with_loaded(read)
	}

	/// Runs `read` on the cached value if it is loaded.
	pub(crate) fn with_loaded<R>(&self, read: impl FnOnce(&T) -> R) -> Option<R> {
		self.0.borrow().as_ref().map(read)
	}

	/// Direct access without runtime borrow tracking, for callers already holding exclusive access.
	pub(crate) fn get_loaded_mut(&mut self) -> Option<&mut T> {
		self.0.get_mut().as_mut()
	}
}

/// If some network calculation is too slow to compute for every usage, cache the data here
#[derive(Debug, Default, Clone)]
pub struct NodeNetworkTransientMetadata {
	pub selected_nodes: SelectedNodes,
	/// Sole dependents of the top of the stacks of all selected nodes. Used to determine which nodes are checked for collision when shifting.
	/// The LayerOwner is used to determine whether the collided node should be shifted, or the layer that owns it.
	pub(crate) stack_dependents: TransientCache<HashMap<NodeId, LayerOwner>>,
	/// Cache for the bounding box around all nodes in node graph space.
	pub(crate) all_nodes_bounding_box: TransientCache<[DVec2; 2]>,
	/// Cache for what the imports and exports keep their distance from, unloaded along with `all_nodes_bounding_box`.
	pub(crate) import_export_clearance: TransientCache<ImportExportClearance>,
	/// Cache for all outward wire connections
	pub(crate) outward_wires: TransientCache<HashMap<OutputConnector, Vec<InputConnector>>>,
	/// All export connector click targets
	pub(crate) import_export_ports: TransientCache<Ports>,

	/// Cached wire SVG paths per input connector, where an entry's presence means that wire is loaded.
	pub(crate) wires: std::cell::RefCell<HashMap<InputConnector, WirePathUpdate>>,
	/// Drag-session state: how far each unowned stack dependent has been pushed vertically from its resting position. Cleared when the drag ends.
	pub(crate) drag_offsets: std::cell::RefCell<HashMap<NodeId, i32>>,
}

/// What the imports and exports keep their distance from horizontally, in node graph space. They keep a few grid cells from the sides of a node,
/// but just one from a redirect dot, which only bends a wire, so the dots are measured apart from the other nodes.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ImportExportClearance {
	/// The leftmost and rightmost sides of the nodes other than redirect dots.
	pub node_sides: Option<[f64; 2]>,
	/// The leftmost and rightmost centers of the redirect dots.
	pub dot_centers: Option<[f64; 2]>,
}

impl ImportExportClearance {
	/// The furthest right the imports can sit: `node_gap` (a negative distance) from the leftmost node's side rounded to the grid, or a grid cell left of the leftmost dot.
	pub fn import_limit(&self, node_gap: f64) -> f64 {
		let beside_nodes = self.node_sides.map(|[left, _]| (left / GRID_SIZE as f64 + 0.5).floor() * GRID_SIZE as f64 + node_gap);
		let beside_dots = self.dot_centers.map(|[left, _]| left - GRID_SIZE as f64);
		beside_nodes.into_iter().chain(beside_dots).reduce(f64::min).unwrap_or(node_gap)
	}

	/// The furthest left the exports can sit: `node_gap` from the rightmost node's side rounded to the grid, or a grid cell right of the rightmost dot.
	pub fn export_limit(&self, node_gap: f64) -> f64 {
		let beside_nodes = self.node_sides.map(|[_, right]| (right / GRID_SIZE as f64 + 0.5).floor() * GRID_SIZE as f64 + node_gap);
		let beside_dots = self.dot_centers.map(|[_, right]| right + GRID_SIZE as f64);
		beside_nodes.into_iter().chain(beside_dots).reduce(f64::max).unwrap_or(node_gap)
	}
}

#[derive(Debug, Clone)]
pub struct NetworkEdgeDistance {
	/// The viewport pixel distance between the left edge of the node graph and the exports.
	pub exports_to_edge_distance: DVec2,
	/// The viewport pixel distance between the left edge of the node graph and the imports.
	pub imports_to_edge_distance: DVec2,
}

#[derive(Debug, Clone)]
pub enum LayerOwner {
	// Used to get the layer that should be shifted when there is a collision.
	Layer(NodeId),
	None,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct DocumentNodeMetadata {
	#[serde(deserialize_with = "deserialize_node_persistent_metadata")]
	pub persistent_metadata: DocumentNodePersistentMetadata,
	#[serde(skip)]
	pub transient_metadata: DocumentNodeTransientMetadata,
}

impl Clone for DocumentNodeMetadata {
	fn clone(&self) -> Self {
		DocumentNodeMetadata {
			persistent_metadata: self.persistent_metadata.clone(),
			transient_metadata: Default::default(),
		}
	}
}

impl PartialEq for DocumentNodeMetadata {
	fn eq(&self, other: &Self) -> bool {
		self.persistent_metadata == other.persistent_metadata
	}
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NumberInputSettings {
	pub unit: Option<String>,
	pub min: Option<f64>,
	pub max: Option<f64>,
	pub step: Option<f64>,
	pub mode: NumberInputMode,
	pub range_min: Option<f64>,
	pub range_max: Option<f64>,
	pub is_integer: bool,
	pub blank_assist: bool,
}

impl Default for NumberInputSettings {
	fn default() -> Self {
		NumberInputSettings {
			unit: None,
			min: None,
			max: None,
			step: None,
			mode: NumberInputMode::default(),
			range_min: None,
			range_max: None,
			is_integer: false,
			blank_assist: true,
		}
	}
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct Vec2InputSettings {
	pub x: String,
	pub y: String,
	pub unit: String,
	pub min: Option<f64>,
	pub is_integer: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum WidgetOverride {
	None,
	Hidden,
	String(String),
	Number(NumberInputSettings),
	Vec2(Vec2InputSettings),
	Custom(String),
}

// TODO: Custom deserialization/serialization to ensure number of input metadata rows matches number of node inputs
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct InputPersistentMetadata {
	/// A general datastore than can store key value pairs of any types for any input
	/// Each instance of the input node needs to store its own data, since it can lose the reference to its
	/// node definition if the node signature is modified by the user. For example adding/removing/renaming an import/export of a network node.
	#[serde(serialize_with = "graphene_std::vector::serialize_hashmap_as_sorted_object")]
	pub input_data: HashMap<String, Value>,
	// An input can override a widget, which would otherwise be automatically generated from the type
	// The string is the identifier to the widget override function stored in INPUT_OVERRIDES
	pub widget_override: Option<String>,
	/// An empty input name means to use the type as the name.
	pub input_name: String,
	/// Displayed as the tooltip description.
	pub input_description: String,
}

impl InputPersistentMetadata {
	pub fn with_name(mut self, input_name: &str) -> Self {
		self.input_name = input_name.to_string();
		self
	}
	pub fn with_override(mut self, widget_override: WidgetOverride) -> Self {
		match widget_override {
			// Uses the default widget for the type
			WidgetOverride::None => {
				self.widget_override = None;
			}
			WidgetOverride::Hidden => {
				self.widget_override = Some("hidden".to_string());
			}
			WidgetOverride::String(string_inspector) => {
				self.input_data.insert("string_inspector".to_string(), Value::String(string_inspector));
				self.widget_override = Some("string".to_string());
			}
			WidgetOverride::Number(mut number_inspector) => {
				if let Some(unit) = number_inspector.unit.take() {
					self.input_data.insert("unit".to_string(), json!(unit));
				}
				if let Some(min) = number_inspector.min.take() {
					self.input_data.insert("min".to_string(), json!(min));
				}
				if let Some(max) = number_inspector.max.take() {
					self.input_data.insert("max".to_string(), json!(max));
				}
				if let Some(step) = number_inspector.step.take() {
					self.input_data.insert("step".to_string(), json!(step));
				}
				if let Some(range_min) = number_inspector.range_min.take() {
					self.input_data.insert("range_min".to_string(), json!(range_min));
				}
				if let Some(range_max) = number_inspector.range_max.take() {
					self.input_data.insert("range_max".to_string(), json!(range_max));
				}
				self.input_data.insert("mode".to_string(), json!(number_inspector.mode));
				self.input_data.insert("is_integer".to_string(), Value::Bool(number_inspector.is_integer));
				self.input_data.insert("blank_assist".to_string(), Value::Bool(number_inspector.blank_assist));
				self.widget_override = Some("number".to_string());
			}
			WidgetOverride::Vec2(vec2_inspector) => {
				self.input_data.insert("x".to_string(), json!(vec2_inspector.x));
				self.input_data.insert("y".to_string(), json!(vec2_inspector.y));
				self.input_data.insert("unit".to_string(), json!(vec2_inspector.unit));
				self.input_data.insert("is_integer".to_string(), Value::Bool(vec2_inspector.is_integer));
				if let Some(min) = vec2_inspector.min {
					self.input_data.insert("min".to_string(), json!(min));
				}
				self.widget_override = Some("vec2".to_string());
			}
			WidgetOverride::Custom(lambda_name) => {
				self.widget_override = Some(lambda_name);
			}
		};
		self
	}

	pub fn with_description(mut self, description: &str) -> Self {
		self.input_description = description.to_string();
		self
	}
}

/// Persistent metadata for each node in the network, which must be included when creating, serializing, and deserializing saving a node.
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentNodePersistentMetadata {
	/// The identity storage holds for this node, pinned when the interface is built from a document so
	/// a conversion back reuses it instead of re-deriving one. Absent for a node storage has not named,
	/// which falls back to a hash of its location.
	#[serde(default)]
	pub storage_id: Option<u64>,
	/// A name chosen by the user for this instance of the node. Empty indicates no given name, in which case the implementation name is displayed to the user in italics.
	#[serde(default)]
	pub display_name: String,
	/// Stores metadata to override the widgets in the Inspector for each input. These can either be generated automatically based on the type, or with a custom function.
	/// Must match the length of node inputs
	pub input_metadata: Vec<InputMetadata>,
	pub output_names: Vec<String>,
	/// Indexed like `output_names`, where a missing or empty entry means the output has no description.
	#[serde(default)]
	pub output_descriptions: Vec<String>,
	/// Represents the lock icon for locking/unlocking the node in the graph UI. When locked, a node cannot be moved in the graph UI.
	#[serde(default)]
	pub locked: bool,
	/// Indicates that the node will be shown in the Inspector when it would otherwise be empty, letting a user easily edit its inputs by just deselecting everything.
	#[serde(default)]
	pub pinned: bool,
	/// Metadata that is specific to either nodes or layers, which are chosen states for displaying as a left-to-right node or bottom-to-top layer.
	/// All fields in NodeTypePersistentMetadata should automatically be updated by using the network interface API
	pub node_type_metadata: NodeTypePersistentMetadata,
	/// This should always be Some for nodes with a [`DocumentNodeImplementation::Network`], and none for [`DocumentNodeImplementation::ProtoNode`]
	pub network_metadata: Option<NodeNetworkMetadata>,
}

impl DocumentNodePersistentMetadata {
	pub fn is_layer(&self) -> bool {
		matches!(self.node_type_metadata, NodeTypePersistentMetadata::Layer(_))
	}
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct InputMetadata {
	pub persistent_metadata: InputPersistentMetadata,
}

impl PartialEq for InputMetadata {
	fn eq(&self, other: &Self) -> bool {
		self.persistent_metadata == other.persistent_metadata
	}
}

impl From<(&str, &str)> for InputMetadata {
	fn from(input_name_and_description: (&str, &str)) -> Self {
		InputMetadata {
			persistent_metadata: InputPersistentMetadata::default()
				.with_name(input_name_and_description.0)
				.with_description(input_name_and_description.1),
		}
	}
}

impl InputMetadata {
	pub fn with_name_description_override(input_name: &str, description: &str, widget_override: WidgetOverride) -> Self {
		InputMetadata {
			persistent_metadata: InputPersistentMetadata::default().with_name(input_name).with_description(description).with_override(widget_override),
		}
	}
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum NodeTypePersistentMetadata {
	Layer(LayerPersistentMetadata),
	Node(NodePersistentMetadata),
}

impl Default for NodeTypePersistentMetadata {
	fn default() -> Self {
		NodeTypePersistentMetadata::node(IVec2::ZERO)
	}
}

impl NodeTypePersistentMetadata {
	pub fn node(position: IVec2) -> NodeTypePersistentMetadata {
		NodeTypePersistentMetadata::Node(NodePersistentMetadata {
			position: NodePosition::Absolute(position),
		})
	}
	pub fn layer(position: IVec2) -> NodeTypePersistentMetadata {
		NodeTypePersistentMetadata::Layer(LayerPersistentMetadata {
			position: LayerPosition::Absolute(position),
		})
	}
}

/// All fields in LayerMetadata should automatically be updated by using the network interface API
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LayerPersistentMetadata {
	/// Stores the position of a layer node, which can either be Absolute or Stack
	pub position: LayerPosition,
}

impl PartialEq for LayerPersistentMetadata {
	fn eq(&self, other: &Self) -> bool {
		self.position == other.position
	}
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NodePersistentMetadata {
	/// Stores the position of a non layer node, which can either be Absolute or Chain
	pub(crate) position: NodePosition,
}

impl NodePersistentMetadata {
	pub fn new(position: NodePosition) -> Self {
		Self { position }
	}
	pub fn position(&self) -> &NodePosition {
		&self.position
	}
}

/// A layer can either be position as Absolute or in a Stack
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum LayerPosition {
	// Position of the node in grid spaces
	Absolute(IVec2),
	// A layer is in a Stack when it feeds into the bottom input of a layer. The Y position stores the vertical distance between the layer and its upstream sibling/parent.
	Stack(u32),
}

/// A node can either be position as Absolute or in a Chain
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum NodePosition {
	// Position of the node in grid spaces
	Absolute(IVec2),
	// In a chain the position is based on the number of nodes to the first layer node
	Chain,
}

/// Cached metadata that should be calculated when creating a node, and should be recalculated when modifying a node property that affects one of the cached fields.
#[derive(Debug, Default, Clone)]
pub struct DocumentNodeTransientMetadata {
	// The click targets are stored as a single struct since it is very rare for only one to be updated, and recomputing all click targets in one function is more efficient than storing them separately.
	pub(crate) click_targets: TransientCache<DocumentNodeClickTargets>,
	/// All nodes that should be moved when this layer is moved, kept here since only layers own nodes.
	pub(crate) owned_nodes: TransientCache<HashSet<NodeId>>,
	/// Width in grid units from the left edge of the layer's thumbnail to its left end, cached since text measurement is slow. Only loaded for layers.
	pub(crate) layer_width: TransientCache<u32>,
}

#[derive(Debug, Clone)]
pub struct DocumentNodeClickTargets {
	/// In order to keep the displayed position of the node in sync with the click target, the displayed position of a node is derived from the top left of the click target
	/// Ensure node_click_target is kept in sync when modifying a node property that changes its size. Currently this is alias, inputs, is_layer, and metadata
	pub node_click_target: ClickTarget,
	/// Stores all port click targets in node graph space.
	pub port_click_targets: Ports,
	// Click targets that are specific to either nodes or layers, which are chosen states for displaying as a left-to-right node or bottom-to-top layer.
	pub node_type_metadata: NodeTypeClickTargets,
}

#[derive(Debug, Clone)]
pub enum NodeTypeClickTargets {
	Layer(Box<LayerClickTargets>),
	Node, // No transient click targets are stored exclusively for nodes
}

/// All fields in TransientLayerMetadata should automatically be updated by using the network interface API
#[derive(Debug, Clone)]
pub struct LayerClickTargets {
	/// Cache for all visibility buttons. Should be automatically updated when update_click_target is called
	pub visibility_click_target: ClickTarget,
	/// Cache for the lock icon button, only present when the layer is locked.
	pub lock_click_target: Option<ClickTarget>,
	/// Cache for the grip icon, which is next to the visibility button.
	pub grip_click_target: ClickTarget,
	/// Cache for the layer's display-name text bounds. Used to detect double-click rename and
	/// to skip the drill-into-subgraph behavior when the click lands on the name itself.
	/// `None` for layers whose display name is empty.
	pub name_click_target: Option<ClickTarget>,
}

pub enum LayerClickTargetTypes {
	Visibility,
	Lock,
	Grip,
	Name,
	// Preview,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NavigationMetadata {
	/// The current pan, and zoom state of the viewport's view of the node graph.
	/// Ensure `DocumentMessage::UpdateDocumentTransform` is called when the pan, zoom, or transform changes.
	pub node_graph_ptz: PTZ,
	// TODO: Eventually remove once te click targets are extracted from the native render
	/// Transform from node graph space to viewport space.
	pub node_graph_to_viewport: DAffine2,
	// TODO: Eventually remove once the import/export positions are extracted from the native render
	/// The width of the node graph in viewport space
	#[serde(default)]
	pub node_graph_width: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TransactionStatus {
	Started,
	Modified,
	#[default]
	Finished,
}

/// The open transaction's bookkeeping from [`NodeNetworkInterface::transaction_checkpoint`], to put back once the writes made after it are undone.
#[derive(Clone, Debug)]
pub struct TransactionCheckpoint {
	pub(super) status: TransactionStatus,
	pub(super) edited_nodes: HashSet<NodeId>,
}

/// How [`NodeNetworkInterface::is_sole_dependent`] should treat a downstream connector it encounters.
#[derive(Clone, Copy, Debug)]
pub(crate) enum SoleDependentStep {
	/// The downstream path ends here, inside the dependent set.
	Terminate,
	/// Keep walking downstream through this node.
	Continue,
	/// The path leaves the dependent set, so the candidate is not a sole dependent.
	Escape,
}

pub(crate) fn visit_network_resources(network: &NodeNetwork, visit: &mut impl FnMut(ResourceId)) {
	for export in &network.exports {
		visit_input_resource(export, visit);
	}
	for node in network.nodes.values() {
		visit_node_resources(node, visit);
	}
}

pub(crate) fn visit_node_resources(node: &DocumentNode, visit: &mut impl FnMut(ResourceId)) {
	for input in &node.inputs {
		visit_input_resource(input, visit);
	}
	if let DocumentNodeImplementation::Network(nested) = &node.implementation {
		visit_network_resources(nested, visit);
	}
}

pub(crate) fn visit_input_resource(input: &NodeInput, visit: &mut impl FnMut(ResourceId)) {
	if let NodeInput::Value { tagged_value, .. } = input
		&& let TaggedValue::Resource(id) = &**tagged_value
	{
		visit(*id);
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn resource_visits_reach_nested_networks_and_exports() {
		let shared = ResourceId::from(1);
		let uses = |id: ResourceId| NodeInput::value(TaggedValue::Resource(id), false);
		let node = |inputs: Vec<NodeInput>| DocumentNode { inputs, ..Default::default() };
		let inner = NodeNetwork {
			exports: vec![uses(shared)],
			nodes: [(NodeId(2), node(vec![uses(shared)]))].into_iter().collect(),
			..Default::default()
		};
		let outer = NodeNetwork {
			nodes: [
				(NodeId(1), node(vec![uses(shared), uses(ResourceId::from(2))])),
				(
					NodeId(3),
					DocumentNode {
						implementation: DocumentNodeImplementation::Network(inner),
						..Default::default()
					},
				),
			]
			.into_iter()
			.collect(),
			..Default::default()
		};

		let mut visits = Vec::new();
		visit_network_resources(&outer, &mut |id| visits.push(id));
		assert_eq!(visits.iter().filter(|id| **id == shared).count(), 3);
		assert_eq!(visits.len(), 4);
	}

	#[test]
	fn port_click_targets_reach_around_the_connector_where_wires_attach() {
		let center = DVec2::new(100., 50.);
		let mut ports = Ports::new();
		ports.insert_input_port_at_center(0, center);
		ports.insert_output_port_at_center(0, center + DVec2::new(200., 0.));

		assert_eq!(ports.clicked_input_port_from_point(center), Some(0));
		assert_eq!(ports.clicked_input_port_from_point(center + DVec2::new(5., 5.)), Some(0));
		assert_eq!(ports.clicked_input_port_from_point(center + DVec2::new(0., 11.)), Some(0));
		assert_eq!(ports.clicked_input_port_from_point(center + DVec2::new(-11., 0.)), Some(0));
		assert_eq!(ports.clicked_input_port_from_point(center + DVec2::new(10., 0.)), None);
		assert_eq!(ports.clicked_input_port_from_point(center + DVec2::new(0., 13.)), None);

		assert_eq!(ports.clicked_output_port_from_point(center + DVec2::new(200., 0.)), Some(0));
		assert_eq!(ports.clicked_output_port_from_point(center), None);

		assert_eq!(ports.input_port_position(0), Some(center), "wires should attach at the connector's center, not the click target's");
	}
}
