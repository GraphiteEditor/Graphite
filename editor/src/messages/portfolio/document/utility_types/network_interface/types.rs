use super::*;

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
	pub fn node(node_id: NodeId, input_index: usize) -> Self {
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

#[derive(Debug, Clone)]
pub struct Ports {
	pub(crate) input_ports: Vec<(usize, ClickTarget)>,
	pub(crate) output_ports: Vec<(usize, ClickTarget)>,
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
		let subpath = Subpath::new_ellipse(center - DVec2::new(8., 8.), center + DVec2::new(8., 8.));
		self.insert_custom_input_port(input_index, ClickTarget::new_with_subpath(subpath, 0.));
	}

	pub(crate) fn insert_custom_input_port(&mut self, input_index: usize, click_target: ClickTarget) {
		self.input_ports.push((input_index, click_target));
	}

	pub(crate) fn insert_output_port_at_center(&mut self, output_index: usize, center: DVec2) {
		let subpath = Subpath::new_ellipse(center - DVec2::new(8., 8.), center + DVec2::new(8., 8.));
		self.insert_custom_output_port(output_index, ClickTarget::new_with_subpath(subpath, 0.));
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
		let center = if input_index == 0 {
			node_top_left + DVec2::new(2. * 24., 24. * 2. + 8.)
		} else {
			node_top_left + DVec2::new(0., 24. * 1.)
		};
		self.insert_input_port_at_center(input_index, center);
	}

	pub(crate) fn insert_layer_output(&mut self, node_top_left: DVec2) {
		// The center of the click target is always 24 px down from the top left corner of the node
		let center = node_top_left + DVec2::new(2. * 24., -8.);
		self.insert_output_port_at_center(0, center);
	}

	pub fn clicked_input_port_from_point(&self, point: DVec2) -> Option<usize> {
		self.input_ports.iter().find_map(|(port, click_target)| click_target.intersect_point_no_stroke(point).then_some(*port))
	}

	pub fn clicked_output_port_from_point(&self, point: DVec2) -> Option<usize> {
		self.output_ports.iter().find_map(|(port, click_target)| click_target.intersect_point_no_stroke(point).then_some(*port))
	}

	pub fn input_port_position(&self, index: usize) -> Option<DVec2> {
		self.input_ports.iter().find_map(|(port_index, click_target)| {
			if *port_index == index {
				click_target.bounding_box().map(|bounds| bounds[0] + DVec2::new(8., 8.))
			} else {
				None
			}
		})
	}

	pub fn output_port_position(&self, index: usize) -> Option<DVec2> {
		self.output_ports.iter().find_map(|(port_index, click_target)| {
			if *port_index == index {
				click_target.bounding_box().map(|bounds| bounds[0] + DVec2::new(8., 8.))
			} else {
				None
			}
		})
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
	/// The display order of pinned nodes in the Properties panel (shown when nothing is selected in this network), keyed by node ID.
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

/// This is the same as Option, but more clear in the context of having cached metadata either being loaded or unloaded
#[derive(Debug, Default, Clone)]
pub enum TransientMetadata<T> {
	Loaded(T),
	#[default]
	Unloaded,
}

impl<T> TransientMetadata<T> {
	/// Set the current transient metadata to unloaded
	pub fn unload(&mut self) {
		*self = TransientMetadata::Unloaded;
	}

	pub fn is_loaded(&self) -> bool {
		matches!(self, TransientMetadata::Loaded(_))
	}
}

/// A lazily computed cache slot whose load and read paths work through &self, with interior mutability guarding the stored value.
#[derive(Debug, Clone)]
pub(crate) struct TransientCache<T>(std::cell::RefCell<TransientMetadata<T>>);

impl<T> Default for TransientCache<T> {
	fn default() -> Self {
		TransientCache(std::cell::RefCell::new(TransientMetadata::Unloaded))
	}
}

impl<T> TransientCache<T> {
	pub(crate) fn is_loaded(&self) -> bool {
		self.0.borrow().is_loaded()
	}

	pub(crate) fn store(&self, value: T) {
		*self.0.borrow_mut() = TransientMetadata::Loaded(value);
	}

	pub(crate) fn unload(&self) {
		*self.0.borrow_mut() = TransientMetadata::Unloaded;
	}

	/// Runs `read` on the cached value if it is loaded.
	pub(crate) fn with_loaded<R>(&self, read: impl FnOnce(&T) -> R) -> Option<R> {
		match &*self.0.borrow() {
			TransientMetadata::Loaded(value) => Some(read(value)),
			TransientMetadata::Unloaded => None,
		}
	}

	/// Direct access without runtime borrow tracking, for callers already holding exclusive access.
	pub(crate) fn get_loaded_mut(&mut self) -> Option<&mut T> {
		match self.0.get_mut() {
			TransientMetadata::Loaded(value) => Some(value),
			TransientMetadata::Unloaded => None,
		}
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
	// /// Cache bounding box for all "groups of nodes", which will be used to prevent overlapping nodes
	// node_group_bounding_box: Vec<(Subpath<ManipulatorGroupId>, Vec<Nodes>)>,
	/// Cache for all outward wire connections
	pub(crate) outward_wires: TransientCache<HashMap<OutputConnector, Vec<InputConnector>>>,
	/// All export connector click targets
	pub(crate) import_export_ports: TransientCache<Ports>,
	/// Click targets for adding, removing, and moving import/export ports
	pub(crate) modify_import_export: TransientCache<ModifyImportExportClickTarget>,

	// Wires from the exports
	pub wires: Vec<TransientMetadata<WirePathUpdate>>,
}

#[derive(Debug, Clone)]
pub struct ModifyImportExportClickTarget {
	// Subtract icon that appears when hovering over an import/export
	pub remove_imports_exports: Ports,
	// Grip drag icon that appears when hovering over an import/export
	pub reorder_imports_exports: Ports,
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
	// The vertical offset of a node from the start of its shift. Should be reset when the drag ends.
	None(i32),
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

// TODO: Custom deserialization/serialization to ensure number of properties row matches number of node inputs
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
			WidgetOverride::String(string_properties) => {
				self.input_data.insert("string_properties".to_string(), Value::String(string_properties));
				self.widget_override = Some("string".to_string());
			}
			WidgetOverride::Number(mut number_properties) => {
				if let Some(unit) = number_properties.unit.take() {
					self.input_data.insert("unit".to_string(), json!(unit));
				}
				if let Some(min) = number_properties.min.take() {
					self.input_data.insert("min".to_string(), json!(min));
				}
				if let Some(max) = number_properties.max.take() {
					self.input_data.insert("max".to_string(), json!(max));
				}
				if let Some(step) = number_properties.step.take() {
					self.input_data.insert("step".to_string(), json!(step));
				}
				if let Some(range_min) = number_properties.range_min.take() {
					self.input_data.insert("range_min".to_string(), json!(range_min));
				}
				if let Some(range_max) = number_properties.range_max.take() {
					self.input_data.insert("range_max".to_string(), json!(range_max));
				}
				self.input_data.insert("mode".to_string(), json!(number_properties.mode));
				self.input_data.insert("is_integer".to_string(), Value::Bool(number_properties.is_integer));
				self.input_data.insert("blank_assist".to_string(), Value::Bool(number_properties.blank_assist));
				self.widget_override = Some("number".to_string());
			}
			WidgetOverride::Vec2(vec2_properties) => {
				self.input_data.insert("x".to_string(), json!(vec2_properties.x));
				self.input_data.insert("y".to_string(), json!(vec2_properties.y));
				self.input_data.insert("unit".to_string(), json!(vec2_properties.unit));
				self.input_data.insert("is_integer".to_string(), Value::Bool(vec2_properties.is_integer));
				if let Some(min) = vec2_properties.min {
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

#[derive(Debug, Clone, Default)]
pub(crate) struct InputTransientMetadata {
	pub(crate) wire: TransientMetadata<WirePathUpdate>,
	// downstream_protonode: populated for all inputs after each compile
	// types: populated for each protonode after each
}

/// Persistent metadata for each node in the network, which must be included when creating, serializing, and deserializing saving a node.
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentNodePersistentMetadata {
	/// A name chosen by the user for this instance of the node. Empty indicates no given name, in which case the implementation name is displayed to the user in italics.
	#[serde(default)]
	pub display_name: String,
	/// Stores metadata to override the properties in the properties panel for each input. These can either be generated automatically based on the type, or with a custom function.
	/// Must match the length of node inputs
	pub input_metadata: Vec<InputMetadata>,
	pub output_names: Vec<String>,
	/// Represents the lock icon for locking/unlocking the node in the graph UI. When locked, a node cannot be moved in the graph UI.
	#[serde(default)]
	pub locked: bool,
	/// Indicates that the node will be shown in the Properties panel when it would otherwise be empty, letting a user easily edit its properties by just deselecting everything.
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

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct InputMetadata {
	pub persistent_metadata: InputPersistentMetadata,
	#[serde(skip)]
	pub(crate) transient_metadata: InputTransientMetadata,
}

impl Clone for InputMetadata {
	fn clone(&self) -> Self {
		InputMetadata {
			persistent_metadata: self.persistent_metadata.clone(),
			transient_metadata: Default::default(),
		}
	}
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
			..Default::default()
		}
	}
}

impl InputMetadata {
	pub fn with_name_description_override(input_name: &str, description: &str, widget_override: WidgetOverride) -> Self {
		InputMetadata {
			persistent_metadata: InputPersistentMetadata::default().with_name(input_name).with_description(description).with_override(widget_override),
			..Default::default()
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
	// TODO: Store click target for the preview button, which will appear when the node is a selected/(hovered?) layer node
	// preview_click_target: Option<ClickTarget>,
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
	Layer(LayerClickTargets),
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
	// TODO: Store click target for the preview button, which will appear when the node is a selected/(hovered?) layer node
	// preview_click_target: ClickTarget,
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

pub(crate) fn collect_network_resources(network: &NodeNetwork, out: &mut HashSet<ResourceId>) {
	for node in network.nodes.values() {
		collect_node_resources(node, out);
	}
}

/// Collects resource IDs referenced by a node and its nested networks.
pub fn collect_node_resources(node: &DocumentNode, out: &mut HashSet<ResourceId>) {
	for input in &node.inputs {
		if let NodeInput::Value { tagged_value, .. } = input
			&& let TaggedValue::Resource(id) = &**tagged_value
		{
			out.insert(*id);
		}
	}
	if let DocumentNodeImplementation::Network(nested) = &node.implementation {
		collect_network_resources(nested, out);
	}
}
