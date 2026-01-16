mod document_node_derive;

use super::node_properties::choice::enum_choice;
use super::node_properties::{self, ParameterWidgetsInfo};
use super::utility_types::FrontendNodeType;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::utility_types::network_interface::{
	DocumentNodeMetadata, DocumentNodePersistentMetadata, InputMetadata, NodeNetworkInterface, NodeNetworkMetadata, NodeNetworkPersistentMetadata, NodeTemplate, NodeTypePersistentMetadata,
	NumberInputSettings, Vec2InputSettings, WidgetOverride,
};
use crate::messages::portfolio::utility_types::PersistentData;
use crate::messages::prelude::Message;
use crate::node_graph_executor::NodeGraphExecutor;
use glam::DVec2;
use graph_craft::ProtoNodeIdentifier;
use graph_craft::concrete;
use graph_craft::document::value::*;
use graph_craft::document::*;
use graphene_std::brush::brush_cache::BrushCache;
use graphene_std::extract_xy::XY;
use graphene_std::raster::{CellularDistanceFunction, CellularReturnType, Color, DomainWarpType, FractalType, NoiseType, RedGreenBlueAlpha};
use graphene_std::raster_types::{CPU, Raster};
use graphene_std::table::Table;
#[allow(unused_imports)]
use graphene_std::transform::Footprint;
use graphene_std::vector::Vector;
use graphene_std::*;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};

pub struct NodePropertiesContext<'a> {
	pub persistent_data: &'a PersistentData,
	pub responses: &'a mut VecDeque<Message>,
	pub executor: &'a mut NodeGraphExecutor,
	pub network_interface: &'a mut NodeNetworkInterface,
	pub selection_network_path: &'a [NodeId],
	pub document_name: &'a str,
}

impl NodePropertiesContext<'_> {
	pub fn call_widget_override(&mut self, node_id: &NodeId, index: usize) -> Option<Vec<LayoutGroup>> {
		let input_properties_row = self.network_interface.persistent_input_metadata(node_id, index, self.selection_network_path)?;
		if let Some(widget_override) = &input_properties_row.widget_override {
			let Some(widget_override_lambda) = INPUT_OVERRIDES.get(widget_override) else {
				log::error!("Could not get widget override '{widget_override}' lambda in call_widget_override");
				return None;
			};
			widget_override_lambda(*node_id, index, self)
				.map_err(|error| log::error!("Error in widget override lambda: {error}"))
				.ok()
		} else {
			None
		}
	}
}

/// The key used to access definitions for a network node or proto node.
/// For proto nodes, this is their [`ProtoNodeIdentifier`].
/// For network nodes, it doesn't necessarily have to be the same as the network's display name, but it often is.
#[derive(Debug, Clone, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(tag = "type", content = "data")]
pub enum DefinitionIdentifier {
	ProtoNode(ProtoNodeIdentifier),
	Network(String),
}

impl DefinitionIdentifier {
	pub fn implementation_name_from_identifier(&self) -> String {
		match self {
			DefinitionIdentifier::Network(name) => name.clone(),
			DefinitionIdentifier::ProtoNode(proto_node_identifier) => registry::NODE_METADATA
				.lock()
				.unwrap()
				.get(proto_node_identifier)
				.map(|metadata| metadata.display_name.to_string())
				.unwrap_or_else(|| {
					let mut last_segment = proto_node_identifier.as_str().split("::").last().unwrap_or_default().to_string();
					last_segment = last_segment.strip_suffix("Node").unwrap_or(&last_segment).to_string();
					last_segment
				}),
		}
	}
}

impl From<Value> for DefinitionIdentifier {
	fn from(value: Value) -> Self {
		match value {
			Value::Object(mut map) => {
				let ty = map.remove("type").unwrap().as_str().unwrap().to_owned();

				match ty.as_ref() {
					"Network" => {
						let data = map.remove("data").unwrap().as_str().unwrap().to_owned();
						DefinitionIdentifier::Network(data)
					}
					"ProtoNode" => {
						let value = map.remove("data").unwrap();
						let proto: ProtoNodeIdentifier = serde_json::from_value(value).unwrap();
						DefinitionIdentifier::ProtoNode(proto)
					}
					_ => panic!("Unknown `DefinitionIdentifier` type: {:?}", ty),
				}
			}

			_ => panic!("Expected a JSON object to convert to `DefinitionIdentifier`"),
		}
	}
}

/// Acts as a description for a [DocumentNode] before it gets instantiated as one.
#[derive(Debug, Clone)]
pub struct DocumentNodeDefinition {
	/// Used to create the [`DefinitionIdentifier::Network`] identifier.
	pub identifier: &'static str,

	/// All data required to construct a [`DocumentNode`] and [`DocumentNodeMetadata`]
	pub node_template: NodeTemplate,

	/// Definition specific data. In order for the editor to access this data, the reference will be used.
	pub category: &'static str,

	/// User-facing description of the node's functionality.
	pub description: Cow<'static, str>,

	/// Node level overrides are stored based on the reference, not the instance. If the node is modified such that it becomes a local version
	/// (for example an input is added), the reference is no longer to the definition, and the overrides are lost.
	/// Most nodes should not use node based properties, since they are less flexible than input level properties.
	pub properties: Option<&'static str>,
}

// We use the once_cell to use the document node definitions throughout the editor without passing a reference
// TODO: If dynamic node library is required, use a Mutex as well
static DOCUMENT_NODE_TYPES: once_cell::sync::Lazy<HashMap<DefinitionIdentifier, DocumentNodeDefinition>> = once_cell::sync::Lazy::new(document_node_definitions);

/// Defines the "signature" or "header file"-like metadata for the document nodes, but not the implementation (which is defined in the node registry).
/// The [`DocumentNode`] is the instance while these [`DocumentNodeDefinition`]s are the "classes" or "blueprints" from which the instances are built.
/// Only the position can be set for protonodes within a definition. The rest of the metadata comes from the node macro in NODE_METADATA
fn document_node_definitions() -> HashMap<DefinitionIdentifier, DocumentNodeDefinition> {
	let custom = vec![
		// TODO: Auto-generate this from its proto node macro
		DocumentNodeDefinition {
			identifier: "Passthrough",
			category: "General",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::ProtoNode(ops::identity::IDENTIFIER),
					inputs: vec![NodeInput::value(TaggedValue::None, true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_metadata: vec![("Content", "TODO").into()],
					output_names: vec!["Out".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("Passes-through the input value without changing it. This is useful for rerouting wires for organization purposes."),
			properties: None,
		},
		// TODO: Auto-generate this from its proto node macro
		DocumentNodeDefinition {
			identifier: "Monitor",
			category: "Debug",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::ProtoNode(memo::monitor::IDENTIFIER),
					inputs: vec![NodeInput::value(TaggedValue::None, true)],
					call_argument: generic!(T),
					skip_deduplication: true,
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_metadata: vec![("In", "TODO").into()],
					output_names: vec!["Out".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("The Monitor node is used by the editor to access the data flowing through it."),
			properties: Some("monitor_properties"),
		},
		DocumentNodeDefinition {
			identifier: "Default Network",
			category: "General",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork::default()),
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					network_metadata: Some(NodeNetworkMetadata::default()),
					..Default::default()
				},
			},
			description: Cow::Borrowed("An empty node network you can use to create your own custom nodes."),
			properties: None,
		},
		// TODO: Auto-generate this from its proto node macro
		DocumentNodeDefinition {
			identifier: "Cache",
			category: "General",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					inputs: vec![NodeInput::value(TaggedValue::None, true)],
					implementation: DocumentNodeImplementation::ProtoNode(memo::memo::IDENTIFIER),
					call_argument: generic!(T),
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_metadata: vec![("Data", "TODO").into()],
					output_names: vec!["Data".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed(
				"Improves rendering performance if used in rare circumstances where automatic caching is not yet advanced enough to handle the situation.\n\
				\n\
				Stores the last evaluated data that flowed through this node, and immediately returns that data on subsequent renders if the context has not changed.",
			),
			properties: None,
		},
		DocumentNodeDefinition {
			identifier: "Merge",
			category: "General",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(4), 0)],
						nodes: [
							// Primary (bottom) input type coercion
							DocumentNode {
								inputs: vec![NodeInput::import(generic!(T), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(graphic::to_graphic::IDENTIFIER),
								..Default::default()
							},
							// Secondary (left) input type coercion
							DocumentNode {
								inputs: vec![NodeInput::import(generic!(T), 1)],
								implementation: DocumentNodeImplementation::ProtoNode(graphic::wrap_graphic::IDENTIFIER),
								..Default::default()
							},
							// Store the ID of the parent node (which encapsulates this sub-network) in each row we are extending the table with.
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(1), 0), NodeInput::Reflection(graph_craft::document::DocumentNodeMetadata::DocumentNodePath)],
								implementation: DocumentNodeImplementation::ProtoNode(graphic::source_node_id::IDENTIFIER),
								..Default::default()
							},
							// The monitor node is used to display a thumbnail in the UI
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(2), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(memo::monitor::IDENTIFIER),
								skip_deduplication: true,
								..Default::default()
							},
							DocumentNode {
								call_argument: generic!(T),
								inputs: vec![NodeInput::node(NodeId(0), 0), NodeInput::node(NodeId(3), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(graphic::extend::IDENTIFIER),
								..Default::default()
							},
						]
						.into_iter()
						.enumerate()
						.map(|(id, node)| (NodeId(id as u64), node))
						.collect(),
						..Default::default()
					}),
					inputs: vec![
						NodeInput::value(TaggedValue::Graphic(Default::default()), true),
						NodeInput::value(TaggedValue::Graphic(Default::default()), true),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_metadata: vec![("Base", "TODO").into(), ("Content", "TODO").into()],
					output_names: vec!["Out".to_string()],
					node_type_metadata: NodeTypePersistentMetadata::layer(IVec2::new(0, 0)),
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(-21, -3)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(-21, -1)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(-14, -1)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(-7, -1)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, -3)),
										..Default::default()
									},
									..Default::default()
								},
							]
							.into_iter()
							.enumerate()
							.map(|(id, node)| (NodeId(id as u64), node))
							.collect(),
							..Default::default()
						},
						..Default::default()
					}),
					..Default::default()
				},
			},
			description: Cow::Borrowed("Merges the provided content as a new element in the graphic table that represents a layer compositing stack."),
			properties: None,
		},
		DocumentNodeDefinition {
			identifier: "Artboard",
			category: "General",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(3), 0)],
						nodes: [
							// Ensure this ID is kept in sync with the ID in set_alias so that the name input is kept in sync with the alias
							DocumentNode {
								call_argument: generic!(T),
								implementation: DocumentNodeImplementation::ProtoNode(artboard::create_artboard::IDENTIFIER),
								inputs: vec![
									NodeInput::import(concrete!(TaggedValue), 1),
									NodeInput::value(TaggedValue::String(String::from("Artboard")), false),
									NodeInput::import(concrete!(TaggedValue), 2),
									NodeInput::import(concrete!(TaggedValue), 3),
									NodeInput::import(concrete!(TaggedValue), 4),
									NodeInput::import(concrete!(TaggedValue), 5),
								],
								..Default::default()
							},
							// Store the ID of the parent node (which encapsulates this sub-network) in each row we are extending the table with.
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(0), 0), NodeInput::Reflection(graph_craft::document::DocumentNodeMetadata::DocumentNodePath)],
								implementation: DocumentNodeImplementation::ProtoNode(graphic::source_node_id::IDENTIFIER),
								..Default::default()
							},
							// The monitor node is used to display a thumbnail in the UI.
							// TODO: Check if thumbnail is reversed
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(1), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(memo::monitor::IDENTIFIER),
								call_argument: generic!(T),
								skip_deduplication: true,
								..Default::default()
							},
							DocumentNode {
								inputs: vec![
									NodeInput::import(graphene_std::Type::Fn(Box::new(concrete!(Context)), Box::new(concrete!(Table<Artboard>))), 0),
									NodeInput::node(NodeId(2), 0),
								],
								implementation: DocumentNodeImplementation::ProtoNode(graphic::extend::IDENTIFIER),
								..Default::default()
							},
						]
						.into_iter()
						.enumerate()
						.map(|(id, node)| (NodeId(id as u64), node))
						.collect(),
						..Default::default()
					}),
					inputs: vec![
						NodeInput::value(TaggedValue::Artboard(Default::default()), true),
						NodeInput::value(TaggedValue::Graphic(Default::default()), true),
						NodeInput::value(TaggedValue::DVec2(DVec2::ZERO), false),
						NodeInput::value(TaggedValue::DVec2(DVec2::new(1920., 1080.)), false),
						NodeInput::value(TaggedValue::Color(Table::new_from_element(Color::WHITE)), false),
						NodeInput::value(TaggedValue::Bool(false), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_metadata: vec![
						("Base", "TODO").into(),
						InputMetadata::with_name_description_override("Content", "TODO", WidgetOverride::Hidden),
						InputMetadata::with_name_description_override(
							"Location",
							"TODO",
							WidgetOverride::Vec2(Vec2InputSettings {
								x: "X".to_string(),
								y: "Y".to_string(),
								unit: " px".to_string(),
								is_integer: true,
								..Default::default()
							}),
						),
						InputMetadata::with_name_description_override(
							"Dimensions",
							"TODO",
							WidgetOverride::Vec2(Vec2InputSettings {
								x: "W".to_string(),
								y: "H".to_string(),
								unit: " px".to_string(),
								is_integer: true,
								..Default::default()
							}),
						),
						InputMetadata::with_name_description_override("Background", "TODO", WidgetOverride::Custom("artboard_background".to_string())),
						("Clip", "TODO").into(),
					],
					output_names: vec!["Out".to_string()],
					node_type_metadata: NodeTypePersistentMetadata::layer(IVec2::new(0, 0)),
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(-21, -3)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(-14, -3)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(-7, -3)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, -4)),
										..Default::default()
									},
									..Default::default()
								},
							]
							.into_iter()
							.enumerate()
							.map(|(id, node)| (NodeId(id as u64), node))
							.collect(),
							..Default::default()
						},
						..Default::default()
					}),
					..Default::default()
				},
			},
			description: Cow::Borrowed("Creates a new Artboard which can be used as a working surface."),
			properties: None,
		},
		DocumentNodeDefinition {
			identifier: "Blend Shapes",
			category: "Vector",
			// [IMPORTS]2 -> 0[0:Floor]
			// [0:Floor]0 -> 0[1:Subtract]
			// "1: f64" -> 1[1:Subtract]
			// "(): ()" -> 0[2:Instance Index]
			// "0: u32" -> 1[2:Instance Index]
			// [2:Instance Index]0 -> 0[3:Divide]
			// [1:Subtract]0 -> 1[3:Divide]
			// [IMPORTS]1 -> 0[4:Position on Path]
			// [3:Divide]0 -> 1[4:Position on Path]
			// "false: bool" -> 2[4:Position on Path]
			// "false: bool" -> 3[4:Position on Path]
			// "(): ()" -> 0[5:Instance Vector]
			// [5:Instance Vector]0 -> 0[6:Reset Transform]
			// "true: bool" -> 1[6:Reset Transform]
			// "false: bool" -> 2[6:Reset Transform]
			// "false: bool" -> 3[6:Reset Transform]
			// [12:Flatten Vector]0 -> 0[7:Instance Map]
			// [6:Reset Transform]0 -> 1[7:Instance Map]
			// [7:Instance Map]0 -> 0[8:Morph]
			// [15:Multiply]0 -> 1[8:Morph]
			// [8:Morph]0 -> 0[9:Transform]
			// [4:Position on Path]0 -> 1[9:Transform]
			// "0: f64" -> 2[9:Transform]
			// "(0, 0): DVec2" -> 3[9:Transform]
			// "(0, 0): DVec2" -> 4[9:Transform]
			// [IMPORTS]1 -> 0[10:Count Points]
			// [10:Count Points]0 -> 0[11:Equals]
			// [13:Count Elements]0 -> 1[11:Equals]
			// [IMPORTS]0 -> 0[12:Flatten Vector]
			// [12:Flatten Vector]0 -> 0[13:Count Elements]
			// [13:Count Elements]0 -> 0[14:Subtract]
			// "1: f64" -> 1[14:Subtract]
			// [3:Divide]0 -> 0[15:Multiply]
			// [14:Subtract]0 -> 1[15:Multiply]
			// [12:Flatten Vector]0 -> 0[16:Morph]
			// [15:Multiply]0 -> 1[16:Morph]
			// [11:Equals]0 -> 0[17:Switch]
			// [9:Transform]0 -> 1[17:Switch]
			// [16:Morph]0 -> 2[17:Switch]
			// [17:Switch]0 -> 0[18:Instance Repeat]
			// [0:Floor]0 -> 1[18:Instance Repeat]
			// [IMPORTS]3 -> 2[18:Instance Repeat]
			// [18:Instance Repeat]0 -> 0[EXPORTS]
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(18), 0)],
						nodes: [
							// 0: Floor
							DocumentNode {
								implementation: DocumentNodeImplementation::ProtoNode(math_nodes::floor::IDENTIFIER),
								inputs: vec![NodeInput::import(concrete!(f64), 2)],
								..Default::default()
							},
							// 1: Subtract
							DocumentNode {
								implementation: DocumentNodeImplementation::ProtoNode(math_nodes::subtract::IDENTIFIER),
								inputs: vec![NodeInput::node(NodeId(0), 0), NodeInput::value(TaggedValue::F64(1.), false)],
								..Default::default()
							},
							// 2: Instance Index
							DocumentNode {
								implementation: DocumentNodeImplementation::ProtoNode(vector_nodes::instance_index::IDENTIFIER),
								inputs: vec![NodeInput::value(TaggedValue::None, false), NodeInput::value(TaggedValue::U32(0), false)],
								..Default::default()
							},
							// 3: Divide
							DocumentNode {
								implementation: DocumentNodeImplementation::ProtoNode(math_nodes::divide::IDENTIFIER),
								inputs: vec![NodeInput::node(NodeId(2), 0), NodeInput::node(NodeId(1), 0)],
								..Default::default()
							},
							// 4: Position on Path
							DocumentNode {
								implementation: DocumentNodeImplementation::ProtoNode(vector_nodes::position_on_path::IDENTIFIER),
								inputs: vec![
									NodeInput::import(generic!(T), 1),
									NodeInput::node(NodeId(3), 0),
									NodeInput::value(TaggedValue::Bool(false), false),
									NodeInput::value(TaggedValue::Bool(false), false),
								],
								..Default::default()
							},
							// 5: Instance Vector
							DocumentNode {
								implementation: DocumentNodeImplementation::ProtoNode(vector_nodes::instance_vector::IDENTIFIER),
								inputs: vec![NodeInput::value(TaggedValue::None, false)],
								..Default::default()
							},
							// 6: Reset Transform
							DocumentNode {
								implementation: DocumentNodeImplementation::ProtoNode(transform_nodes::reset_transform::IDENTIFIER),
								inputs: vec![
									NodeInput::node(NodeId(5), 0),
									NodeInput::value(TaggedValue::Bool(true), false),
									NodeInput::value(TaggedValue::Bool(false), false),
									NodeInput::value(TaggedValue::Bool(false), false),
								],
								..Default::default()
							},
							// 7: Instance Map
							DocumentNode {
								implementation: DocumentNodeImplementation::ProtoNode(vector_nodes::instance_map::IDENTIFIER),
								inputs: vec![NodeInput::node(NodeId(12), 0), NodeInput::node(NodeId(6), 0)],
								..Default::default()
							},
							// 8: Morph
							DocumentNode {
								implementation: DocumentNodeImplementation::ProtoNode(vector::morph::IDENTIFIER),
								inputs: vec![NodeInput::node(NodeId(7), 0), NodeInput::node(NodeId(15), 0)],
								..Default::default()
							},
							// 9: Transform
							DocumentNode {
								implementation: DocumentNodeImplementation::ProtoNode(transform_nodes::transform::IDENTIFIER),
								inputs: vec![
									NodeInput::node(NodeId(8), 0),
									NodeInput::node(NodeId(4), 0),
									NodeInput::value(TaggedValue::F64(0.), false),
									NodeInput::value(TaggedValue::DVec2(DVec2::ONE), false),
									NodeInput::value(TaggedValue::DVec2(DVec2::ZERO), false),
								],
								..Default::default()
							},
							// 10: Count Points
							DocumentNode {
								implementation: DocumentNodeImplementation::ProtoNode(vector_nodes::count_points::IDENTIFIER),
								inputs: vec![NodeInput::import(generic!(T), 1)],
								..Default::default()
							},
							// 11: Equals
							DocumentNode {
								implementation: DocumentNodeImplementation::ProtoNode(math_nodes::equals::IDENTIFIER),
								inputs: vec![NodeInput::node(NodeId(10), 0), NodeInput::node(NodeId(13), 0)],
								..Default::default()
							},
							// 12: Flatten Vector
							DocumentNode {
								implementation: DocumentNodeImplementation::ProtoNode(graphic_nodes::graphic::flatten_vector::IDENTIFIER),
								inputs: vec![NodeInput::import(generic!(T), 0)],
								..Default::default()
							},
							// 13: Count Elements
							DocumentNode {
								implementation: DocumentNodeImplementation::ProtoNode(vector::count_elements::IDENTIFIER),
								inputs: vec![NodeInput::node(NodeId(12), 0)],
								..Default::default()
							},
							// 14: Subtract
							DocumentNode {
								implementation: DocumentNodeImplementation::ProtoNode(math_nodes::subtract::IDENTIFIER),
								inputs: vec![NodeInput::node(NodeId(13), 0), NodeInput::value(TaggedValue::F64(1.), false)],
								..Default::default()
							},
							// 15: Multiply
							DocumentNode {
								implementation: DocumentNodeImplementation::ProtoNode(math_nodes::multiply::IDENTIFIER),
								inputs: vec![NodeInput::node(NodeId(3), 0), NodeInput::node(NodeId(14), 0)],
								..Default::default()
							},
							// 16: Morph
							DocumentNode {
								implementation: DocumentNodeImplementation::ProtoNode(vector::morph::IDENTIFIER),
								inputs: vec![NodeInput::node(NodeId(12), 0), NodeInput::node(NodeId(15), 0)],
								..Default::default()
							},
							// 17: Switch
							DocumentNode {
								implementation: DocumentNodeImplementation::ProtoNode(logic::switch::IDENTIFIER),
								inputs: vec![NodeInput::node(NodeId(11), 0), NodeInput::node(NodeId(9), 0), NodeInput::node(NodeId(16), 0)],
								..Default::default()
							},
							// 18: Instance Repeat
							DocumentNode {
								implementation: DocumentNodeImplementation::ProtoNode(vector_nodes::instance_repeat::IDENTIFIER),
								inputs: vec![NodeInput::node(NodeId(17), 0), NodeInput::node(NodeId(0), 0), NodeInput::import(generic!(T), 3)],
								..Default::default()
							},
						]
						.into_iter()
						.enumerate()
						.map(|(id, node)| (NodeId(id as u64), node))
						.collect(),
						..Default::default()
					}),
					inputs: vec![
						NodeInput::value(TaggedValue::Vector(Default::default()), true),
						NodeInput::value(TaggedValue::Vector(Default::default()), true),
						NodeInput::value(TaggedValue::F64(10.), false),
						NodeInput::value(TaggedValue::Bool(Default::default()), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_metadata: vec![("Content", "TODO").into(), ("Path", "TODO").into(), ("Count", "TODO").into(), ("Reverse", "TODO").into()],
					output_names: vec!["Out".to_string()],
					node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								// 0: Floor
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								// 1: Subtract
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(7, -1)),
										..Default::default()
									},
									..Default::default()
								},
								// 2: Instance Index
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(7, -2)),
										..Default::default()
									},
									..Default::default()
								},
								// 3: Divide
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(14, -2)),
										..Default::default()
									},
									..Default::default()
								},
								// 4: Position on Path
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(28, -3)),
										..Default::default()
									},
									..Default::default()
								},
								// 5: Instance Vector
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(7, 2)),
										..Default::default()
									},
									..Default::default()
								},
								// 6: Reset Transform
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(14, 2)),
										..Default::default()
									},
									..Default::default()
								},
								// 7: Instance Map
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(21, 1)),
										..Default::default()
									},
									..Default::default()
								},
								// 8: Morph
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(28, 1)),
										..Default::default()
									},
									..Default::default()
								},
								// 9: Transform
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(35, 1)),
										..Default::default()
									},
									..Default::default()
								},
								// 10: Count Points
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(7, 4)),
										..Default::default()
									},
									..Default::default()
								},
								// 11: Equals
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(14, 4)),
										..Default::default()
									},
									..Default::default()
								},
								// 12: Flatten Vector
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 6)),
										..Default::default()
									},
									..Default::default()
								},
								// 13: Count Elements
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(7, 8)),
										..Default::default()
									},
									..Default::default()
								},
								// 14: Subtract
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(14, 8)),
										..Default::default()
									},
									..Default::default()
								},
								// 15: Multiply
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(21, 7)),
										..Default::default()
									},
									..Default::default()
								},
								// 16: Morph
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(28, 6)),
										..Default::default()
									},
									..Default::default()
								},
								// 17: Switch
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(42, 4)),
										..Default::default()
									},
									..Default::default()
								},
								// 18: Instance Repeat
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(49, -1)),
										..Default::default()
									},
									..Default::default()
								},
							]
							.into_iter()
							.enumerate()
							.map(|(id, node)| (NodeId(id as u64), node))
							.collect(),
							..Default::default()
						},
						..Default::default()
					}),
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: None,
		},
		DocumentNodeDefinition {
			identifier: "Origins to Polyline",
			category: "Vector",
			// "(): ()" -> 0[0:Instance Vector]
			// [0:Instance Vector]0 -> 0[1:Extract Transform]
			// [1:Extract Transform]0 -> 0[2:Decompose Translation]
			// [2:Decompose Translation]0 -> 0[3:Vec2 to Point]
			// [IMPORTS]0 -> 0[4:Flatten Vector]
			// [4:Flatten Vector]0 -> 0[5:Instance Map]
			// [3:Vec2 to Point]0 -> 1[5:Instance Map]
			// [5:Instance Map]0 -> 0[6: Flatten Path]
			// [6:Flatten Path]0 -> 0[7:Points to Polyline]
			// "false: bool" -> 1[7:Points to Polyline]
			// [7:Points to Polyline]0 -> 0[EXPORTS]
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(7), 0)],
						nodes: [
							// 0: Instance Vector
							DocumentNode {
								implementation: DocumentNodeImplementation::ProtoNode(vector_nodes::instance_vector::IDENTIFIER),
								inputs: vec![NodeInput::value(TaggedValue::None, false)],
								..Default::default()
							},
							// 1: Extract Transform
							DocumentNode {
								implementation: DocumentNodeImplementation::ProtoNode(transform_nodes::extract_transform::IDENTIFIER),
								inputs: vec![NodeInput::node(NodeId(0), 0)],
								..Default::default()
							},
							// 2: Decompose Translation
							DocumentNode {
								implementation: DocumentNodeImplementation::ProtoNode(transform_nodes::decompose_translation::IDENTIFIER),
								inputs: vec![NodeInput::node(NodeId(1), 0)],
								..Default::default()
							},
							// 3: Vec2 to Point
							DocumentNode {
								implementation: DocumentNodeImplementation::ProtoNode(vector_nodes::vec_2_to_point::IDENTIFIER),
								inputs: vec![NodeInput::node(NodeId(2), 0)],
								..Default::default()
							},
							// 4: Flatten Vector
							DocumentNode {
								implementation: DocumentNodeImplementation::ProtoNode(graphic_nodes::graphic::flatten_vector::IDENTIFIER),
								inputs: vec![NodeInput::import(generic!(T), 0)],
								..Default::default()
							},
							// 5: Instance Map
							DocumentNode {
								implementation: DocumentNodeImplementation::ProtoNode(vector_nodes::instance_map::IDENTIFIER),
								inputs: vec![NodeInput::node(NodeId(4), 0), NodeInput::node(NodeId(3), 0)],
								..Default::default()
							},
							// 6: Flatten Path
							DocumentNode {
								implementation: DocumentNodeImplementation::ProtoNode(vector::flatten_path::IDENTIFIER),
								inputs: vec![NodeInput::node(NodeId(5), 0)],
								..Default::default()
							},
							// 7: Points to Polyline
							DocumentNode {
								implementation: DocumentNodeImplementation::ProtoNode(vector::points_to_polyline::IDENTIFIER),
								inputs: vec![NodeInput::node(NodeId(6), 0), NodeInput::value(TaggedValue::Bool(false), false)],
								..Default::default()
							},
						]
						.into_iter()
						.enumerate()
						.map(|(id, node)| (NodeId(id as u64), node))
						.collect(),
						..Default::default()
					}),
					inputs: vec![NodeInput::value(TaggedValue::Vector(Default::default()), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_metadata: vec![("Vector", "TODO").into()],
					output_names: vec!["Vector".to_string()],
					node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								// 0: Instance Vector
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 1)),
										..Default::default()
									},
									..Default::default()
								},
								// 1: Extract Transform
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(7, 1)),
										..Default::default()
									},
									..Default::default()
								},
								// 2: Decompose Transform
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(14, 1)),
										..Default::default()
									},
									..Default::default()
								},
								// 3: Vec2 to Point
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(21, 1)),
										..Default::default()
									},
									..Default::default()
								},
								// 4: Flatten Vector
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(21, 0)),
										..Default::default()
									},
									..Default::default()
								},
								// 5: Instance Map
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(28, 0)),
										..Default::default()
									},
									..Default::default()
								},
								// 6: Flatten Path
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(35, 0)),
										..Default::default()
									},
									..Default::default()
								},
								// 7: Points to Polyline
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(42, 0)),
										..Default::default()
									},
									..Default::default()
								},
							]
							.into_iter()
							.enumerate()
							.map(|(id, node)| (NodeId(id as u64), node))
							.collect(),
							..Default::default()
						},
						..Default::default()
					}),
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: None,
		},
		DocumentNodeDefinition {
			identifier: "Load Image",
			category: "Web Request",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(1), 0)],
						nodes: [
							DocumentNode {
								inputs: vec![NodeInput::value(TaggedValue::None, false), NodeInput::scope("editor-api"), NodeInput::import(concrete!(String), 1)],
								implementation: DocumentNodeImplementation::ProtoNode(wasm_application_io::load_resource::IDENTIFIER),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(0), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(wasm_application_io::decode_image::IDENTIFIER),
								..Default::default()
							},
						]
						.into_iter()
						.enumerate()
						.map(|(id, node)| (NodeId(id as u64), node))
						.collect(),
						..Default::default()
					}),
					inputs: vec![NodeInput::value(TaggedValue::None, false), NodeInput::value(TaggedValue::String("graphite:null".to_string()), false)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_metadata: vec![("Empty", "TODO").into(), ("URL", "TODO").into()],
					output_names: vec!["Image".to_string()],
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(7, 0)),
										..Default::default()
									},
									..Default::default()
								},
							]
							.into_iter()
							.enumerate()
							.map(|(id, node)| (NodeId(id as u64), node))
							.collect(),
							..Default::default()
						},
						..Default::default()
					}),
					..Default::default()
				},
			},
			description: Cow::Borrowed("Loads an image from a given URL."),
			properties: None,
		},
		#[cfg(all(feature = "gpu", target_family = "wasm"))]
		DocumentNodeDefinition {
			identifier: "Rasterize",
			category: "Raster",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(2), 0)],
						nodes: [
							DocumentNode {
								inputs: vec![NodeInput::scope("editor-api")],
								implementation: DocumentNodeImplementation::ProtoNode(wasm_application_io::create_surface::IDENTIFIER),
								skip_deduplication: true,
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(0), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(memo::memo::IDENTIFIER),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::import(generic!(T), 0), NodeInput::import(concrete!(Footprint), 1), NodeInput::node(NodeId(1), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(wasm_application_io::rasterize::IDENTIFIER),
								..Default::default()
							},
						]
						.into_iter()
						.enumerate()
						.map(|(id, node)| (NodeId(id as u64), node))
						.collect(),
						..Default::default()
					}),
					inputs: vec![
						NodeInput::value(TaggedValue::Vector(Default::default()), true),
						NodeInput::value(
							TaggedValue::Footprint(Footprint {
								transform: DAffine2::from_scale_angle_translation(DVec2::new(1000., 1000.), 0., DVec2::new(0., 0.)),
								resolution: UVec2::new(1000, 1000),
								..Default::default()
							}),
							false,
						),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_metadata: vec![("Artwork", "TODO").into(), ("Footprint", "TODO").into()],
					output_names: vec!["Canvas".to_string()],
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 2)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(7, 2)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(14, 0)),
										..Default::default()
									},
									..Default::default()
								},
							]
							.into_iter()
							.enumerate()
							.map(|(id, node)| (NodeId(id as u64), node))
							.collect(),
							..Default::default()
						},
						..Default::default()
					}),
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: None,
		},
		// TODO: Auto-generate this from its proto node macro
		DocumentNodeDefinition {
			identifier: "Noise Pattern",
			category: "Raster: Pattern",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::ProtoNode(graphene_std::raster_nodes::std_nodes::noise_pattern::IDENTIFIER),
					inputs: vec![
						NodeInput::value(TaggedValue::None, false),
						NodeInput::value(TaggedValue::Bool(true), false),
						NodeInput::value(TaggedValue::U32(0), false),
						NodeInput::value(TaggedValue::F64(10.), false),
						NodeInput::value(TaggedValue::NoiseType(NoiseType::default()), false),
						NodeInput::value(TaggedValue::DomainWarpType(DomainWarpType::default()), false),
						NodeInput::value(TaggedValue::F64(100.), false),
						NodeInput::value(TaggedValue::FractalType(FractalType::default()), false),
						NodeInput::value(TaggedValue::U32(3), false),
						NodeInput::value(TaggedValue::F64(2.), false),
						NodeInput::value(TaggedValue::F64(0.5), false),
						NodeInput::value(TaggedValue::F64(0.), false), // 0-1 range
						NodeInput::value(TaggedValue::F64(2.), false),
						NodeInput::value(TaggedValue::CellularDistanceFunction(CellularDistanceFunction::default()), false),
						NodeInput::value(TaggedValue::CellularReturnType(CellularReturnType::default()), false),
						NodeInput::value(TaggedValue::F64(1.), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_metadata: vec![
						("Spacer", "TODO").into(),
						("Clip", "TODO").into(),
						("Seed", "TODO").into(),
						InputMetadata::with_name_description_override("Scale", "TODO", WidgetOverride::Custom("noise_properties_scale".to_string())),
						InputMetadata::with_name_description_override("Noise Type", "TODO", WidgetOverride::Custom("noise_properties_noise_type".to_string())),
						InputMetadata::with_name_description_override("Domain Warp Type", "TODO", WidgetOverride::Custom("noise_properties_domain_warp_type".to_string())),
						InputMetadata::with_name_description_override("Domain Warp Amplitude", "TODO", WidgetOverride::Custom("noise_properties_domain_warp_amplitude".to_string())),
						InputMetadata::with_name_description_override("Fractal Type", "TODO", WidgetOverride::Custom("noise_properties_fractal_type".to_string())),
						InputMetadata::with_name_description_override("Fractal Octaves", "TODO", WidgetOverride::Custom("noise_properties_fractal_octaves".to_string())),
						InputMetadata::with_name_description_override("Fractal Lacunarity", "TODO", WidgetOverride::Custom("noise_properties_fractal_lacunarity".to_string())),
						InputMetadata::with_name_description_override("Fractal Gain", "TODO", WidgetOverride::Custom("noise_properties_fractal_gain".to_string())),
						InputMetadata::with_name_description_override("Fractal Weighted Strength", "TODO", WidgetOverride::Custom("noise_properties_fractal_weighted_strength".to_string())),
						InputMetadata::with_name_description_override("Fractal Ping Pong Strength", "TODO", WidgetOverride::Custom("noise_properties_ping_pong_strength".to_string())),
						InputMetadata::with_name_description_override("Cellular Distance Function", "TODO", WidgetOverride::Custom("noise_properties_cellular_distance_function".to_string())),
						InputMetadata::with_name_description_override("Cellular Return Type", "TODO", WidgetOverride::Custom("noise_properties_cellular_return_type".to_string())),
						InputMetadata::with_name_description_override("Cellular Jitter", "TODO", WidgetOverride::Custom("noise_properties_cellular_jitter".to_string())),
					],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("Generates customizable procedural noise patterns."),
			properties: None,
		},
		DocumentNodeDefinition {
			identifier: "Split Channels",
			category: "Raster: Channels",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![
							NodeInput::value(TaggedValue::None, false),
							NodeInput::node(NodeId(0), 0),
							NodeInput::node(NodeId(1), 0),
							NodeInput::node(NodeId(2), 0),
							NodeInput::node(NodeId(3), 0),
						],
						nodes: [
							DocumentNode {
								inputs: vec![
									NodeInput::import(concrete!(Table<Raster<CPU>>), 0),
									NodeInput::value(TaggedValue::RedGreenBlueAlpha(RedGreenBlueAlpha::Red), false),
								],
								implementation: DocumentNodeImplementation::ProtoNode(raster_nodes::adjustments::extract_channel::IDENTIFIER),
								call_argument: generic!(T),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![
									NodeInput::import(concrete!(Table<Raster<CPU>>), 0),
									NodeInput::value(TaggedValue::RedGreenBlueAlpha(RedGreenBlueAlpha::Green), false),
								],
								implementation: DocumentNodeImplementation::ProtoNode(raster_nodes::adjustments::extract_channel::IDENTIFIER),
								call_argument: generic!(T),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![
									NodeInput::import(concrete!(Table<Raster<CPU>>), 0),
									NodeInput::value(TaggedValue::RedGreenBlueAlpha(RedGreenBlueAlpha::Blue), false),
								],
								implementation: DocumentNodeImplementation::ProtoNode(raster_nodes::adjustments::extract_channel::IDENTIFIER),
								call_argument: generic!(T),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![
									NodeInput::import(concrete!(Table<Raster<CPU>>), 0),
									NodeInput::value(TaggedValue::RedGreenBlueAlpha(RedGreenBlueAlpha::Alpha), false),
								],
								implementation: DocumentNodeImplementation::ProtoNode(raster_nodes::adjustments::extract_channel::IDENTIFIER),
								call_argument: generic!(T),
								..Default::default()
							},
						]
						.into_iter()
						.enumerate()
						.map(|(id, node)| (NodeId(id as u64), node))
						.collect(),
						..Default::default()
					}),
					inputs: vec![NodeInput::value(TaggedValue::Raster(Default::default()), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_metadata: vec![("Image", "TODO").into()],
					output_names: vec!["".to_string(), "Red".to_string(), "Green".to_string(), "Blue".to_string(), "Alpha".to_string()],
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 2)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 4)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 6)),
										..Default::default()
									},
									..Default::default()
								},
							]
							.into_iter()
							.enumerate()
							.map(|(id, node)| (NodeId(id as u64), node))
							.collect(),
							..Default::default()
						},
						..Default::default()
					}),
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: None,
		},
		DocumentNodeDefinition {
			identifier: "Split Vec2",
			category: "Math: Vector",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::value(TaggedValue::None, false), NodeInput::node(NodeId(0), 0), NodeInput::node(NodeId(1), 0)],
						nodes: [
							DocumentNode {
								inputs: vec![NodeInput::import(concrete!(DVec2), 0), NodeInput::value(TaggedValue::XY(XY::X), false)],
								implementation: DocumentNodeImplementation::ProtoNode(extract_xy::extract_xy::IDENTIFIER),
								call_argument: generic!(T),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::import(concrete!(DVec2), 0), NodeInput::value(TaggedValue::XY(XY::Y), false)],
								implementation: DocumentNodeImplementation::ProtoNode(extract_xy::extract_xy::IDENTIFIER),
								call_argument: generic!(T),
								..Default::default()
							},
						]
						.into_iter()
						.enumerate()
						.map(|(id, node)| (NodeId(id as u64), node))
						.collect(),

						..Default::default()
					}),
					inputs: vec![NodeInput::value(TaggedValue::DVec2(DVec2::ZERO), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_metadata: vec![("Vec2", "TODO").into()],
					output_names: vec!["".to_string(), "X".to_string(), "Y".to_string()],
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 2)),
										..Default::default()
									},
									..Default::default()
								},
							]
							.into_iter()
							.enumerate()
							.map(|(id, node)| (NodeId(id as u64), node))
							.collect(),
							..Default::default()
						},
						..Default::default()
					}),
					..Default::default()
				},
			},
			description: Cow::Borrowed(
				"Decomposes the X and Y components of a vec2.\n\
				\n\
				The inverse of this node is \"Vec2 Value\", which can have either or both its X and Y parameters exposed as graph inputs.",
			),
			properties: None,
		},
		// TODO: Remove this and just use the proto node definition directly
		DocumentNodeDefinition {
			identifier: "Brush",
			category: "Raster",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(0), 0)],
						nodes: vec![DocumentNode {
							inputs: vec![
								NodeInput::import(concrete!(Table<Raster<CPU>>), 0),
								NodeInput::import(concrete!(Vec<brush::brush_stroke::BrushStroke>), 1),
								NodeInput::import(concrete!(BrushCache), 2),
							],
							implementation: DocumentNodeImplementation::ProtoNode(brush::brush::brush::IDENTIFIER),
							..Default::default()
						}]
						.into_iter()
						.enumerate()
						.map(|(id, node)| (NodeId(id as u64), node))
						.collect(),
						..Default::default()
					}),
					inputs: vec![
						NodeInput::value(TaggedValue::Raster(Default::default()), true),
						NodeInput::value(TaggedValue::BrushStrokes(Vec::new()), false),
						NodeInput::value(TaggedValue::BrushCache(BrushCache::default()), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_metadata: vec![("Background", "TODO").into(), ("Trace", "TODO").into(), ("Cache", "TODO").into()],
					output_names: vec!["Image".to_string()],
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [DocumentNodeMetadata {
								persistent_metadata: DocumentNodePersistentMetadata {
									node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
									..Default::default()
								},
								..Default::default()
							}]
							.into_iter()
							.enumerate()
							.map(|(id, node)| (NodeId(id as u64), node))
							.collect(),
							..Default::default()
						},
						..Default::default()
					}),
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: None,
		},
		// TODO: Auto-generate this from its proto node macro
		DocumentNodeDefinition {
			identifier: "Memoize",
			category: "Debug",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::ProtoNode(memo::memo::IDENTIFIER),
					inputs: vec![NodeInput::value(TaggedValue::Raster(Default::default()), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_metadata: vec![("Image", "TODO").into()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: None,
		},
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			identifier: "Upload Texture",
			category: "Debug: GPU",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(2), 0)],
						nodes: [
							DocumentNode {
								inputs: vec![NodeInput::scope("editor-api")],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<&WgpuExecutor>")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::import(concrete!(Table<Raster<CPU>>), 0), NodeInput::node(NodeId(0), 0)],
								call_argument: generic!(T),
								implementation: DocumentNodeImplementation::ProtoNode(wgpu_executor::texture_conversion::upload_texture::IDENTIFIER),
								..Default::default()
							},
							DocumentNode {
								call_argument: generic!(T),
								inputs: vec![NodeInput::node(NodeId(1), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(memo::memo::IDENTIFIER),
								..Default::default()
							},
						]
						.into_iter()
						.enumerate()
						.map(|(id, node)| (NodeId(id as u64), node))
						.collect(),
						..Default::default()
					}),
					inputs: vec![NodeInput::value(TaggedValue::Raster(Default::default()), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					output_names: vec!["Texture".to_string()],
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(-7, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(7, 0)),
										..Default::default()
									},
									..Default::default()
								},
							]
							.into_iter()
							.enumerate()
							.map(|(id, node)| (NodeId(id as u64), node))
							.collect(),
							..Default::default()
						},
						..Default::default()
					}),
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: None,
		},
		DocumentNodeDefinition {
			identifier: "Extract",
			category: "Debug",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Extract,
					inputs: vec![NodeInput::value(TaggedValue::DocumentNode(DocumentNode::default()), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_metadata: vec![("Node", "TODO").into()],
					output_names: vec!["Document Node".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: None,
		},
		// Aims for interoperable compatibility with:
		// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=levl%27%20%3D%20Levels-,%27curv%27%20%3D%20Curves,-%27expA%27%20%3D%20Exposure
		// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=Max%20input%20range-,Curves,-Curves%20settings%20files
		//
		// Some further analysis available at:
		// https://geraldbakker.nl/psnumbers/curves.html
		// TODO: Fix this, it's currently broken
		// DocumentNodeDefinition {
		// 	identifier: "Curves",
		// 	category: "Raster: Adjustment",
		// 	node_template: NodeTemplate {
		// 		document_node: DocumentNode {
		// 			implementation: DocumentNodeImplementation::proto("core_types::raster::CurvesNode"),
		// 			inputs: vec![
		// 				NodeInput::value(TaggedValue::Raster(Default::default()), true),
		// 				NodeInput::value(TaggedValue::Curve(Default::default()), false),
		// 			],
		// 			..Default::default()
		// 		},
		// 		persistent_node_metadata: DocumentNodePersistentMetadata {
		// 			input_properties: vec![("Image", "TODO").into(), ("Curve", "TODO").into()],
		// 			output_names: vec!["Image".to_string()],
		// 			..Default::default()
		// 		},
		// 	},
		// 	description: Cow::Borrowed("TODO"),
		// 	properties: None,
		// },
		DocumentNodeDefinition {
			identifier: "Path",
			category: "Vector",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(1), 0)],
						nodes: vec![
							DocumentNode {
								inputs: vec![NodeInput::import(concrete!(Table<Vector>), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(memo::monitor::IDENTIFIER),
								call_argument: generic!(T),
								skip_deduplication: true,
								..Default::default()
							},
							DocumentNode {
								inputs: vec![
									NodeInput::node(NodeId(0), 0),
									NodeInput::import(concrete!(graphene_std::vector::VectorModification), 1),
									NodeInput::Reflection(graph_craft::document::DocumentNodeMetadata::DocumentNodePath),
								],
								call_argument: generic!(T),
								implementation: DocumentNodeImplementation::ProtoNode(vector::path_modify::IDENTIFIER),
								..Default::default()
							},
						]
						.into_iter()
						.enumerate()
						.map(|(id, node)| (NodeId(id as u64), node))
						.collect(),
						..Default::default()
					}),
					inputs: vec![
						NodeInput::value(TaggedValue::Vector(Default::default()), true),
						NodeInput::value(TaggedValue::VectorModification(Default::default()), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_metadata: vec![("Content", "TODO").into(), ("Modification", "TODO").into()],
					output_names: vec!["Modified".to_string()],
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(7, 0)),
										..Default::default()
									},
									..Default::default()
								},
							]
							.into_iter()
							.enumerate()
							.map(|(id, node)| (NodeId(id as u64), node))
							.collect(),
							..Default::default()
						},
						..Default::default()
					}),
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: None,
		},
		// TODO: Auto-generate this from its proto node macro
		DocumentNodeDefinition {
			identifier: "Transform",
			category: "Math: Transform",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					inputs: vec![
						// Value
						NodeInput::value(TaggedValue::DAffine2(DAffine2::default()), true),
						// Translation
						NodeInput::value(TaggedValue::DVec2(DVec2::ZERO), false),
						// Rotation
						NodeInput::value(TaggedValue::F64(0.), false),
						// Scale
						NodeInput::value(TaggedValue::DVec2(DVec2::ONE), false),
						// Skew
						NodeInput::value(TaggedValue::DVec2(DVec2::ZERO), false),
						// Origin Offset
						NodeInput::value(TaggedValue::DVec2(DVec2::ZERO), false),
						// Scale Appearance
						NodeInput::value(TaggedValue::Bool(true), false),
					],
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![
							// From the Transform node
							NodeInput::node(NodeId(1), 0),
						],
						nodes: [
							// Monitor node
							DocumentNode {
								inputs: vec![
									// From the Value import
									NodeInput::import(generic!(T), 0),
								],
								implementation: DocumentNodeImplementation::ProtoNode(memo::monitor::IDENTIFIER),
								call_argument: generic!(T),
								skip_deduplication: true,
								..Default::default()
							},
							// Transform node
							DocumentNode {
								inputs: vec![
									// From the Monitor node
									NodeInput::node(NodeId(0), 0),
									// From the Translation import
									NodeInput::import(concrete!(DVec2), 1),
									// From the Rotation import
									NodeInput::import(concrete!(f64), 2),
									// From the Scale import
									NodeInput::import(concrete!(DVec2), 3),
									// From the Skew import
									NodeInput::import(concrete!(DVec2), 4),
								],
								implementation: DocumentNodeImplementation::ProtoNode(transform_nodes::transform::IDENTIFIER),
								..Default::default()
							},
						]
						.into_iter()
						.enumerate()
						.map(|(id, node)| (NodeId(id as u64), node))
						.collect(),
						..Default::default()
					}),
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(7, 0)),
										..Default::default()
									},
									..Default::default()
								},
							]
							.into_iter()
							.enumerate()
							.map(|(id, node)| (NodeId(id as u64), node))
							.collect(),
							..Default::default()
						},
						..Default::default()
					}),
					input_metadata: vec![
						("Value", "TODO").into(),
						InputMetadata::with_name_description_override(
							"Translation",
							"TODO",
							WidgetOverride::Vec2(Vec2InputSettings {
								x: "X".to_string(),
								y: "Y".to_string(),
								unit: " px".to_string(),
								..Default::default()
							}),
						),
						InputMetadata::with_name_description_override("Rotation", "TODO", WidgetOverride::Custom("transform_rotation".to_string())),
						InputMetadata::with_name_description_override(
							"Scale",
							"TODO",
							WidgetOverride::Vec2(Vec2InputSettings {
								x: "W".to_string(),
								y: "H".to_string(),
								unit: "x".to_string(),
								..Default::default()
							}),
						),
						InputMetadata::with_name_description_override("Skew", "TODO", WidgetOverride::Custom("transform_skew".to_string())),
						InputMetadata::with_name_description_override("Origin Offset", "TODO", WidgetOverride::Custom("hidden".to_string())),
						InputMetadata::with_name_description_override("Scale Appearance", "TODO", WidgetOverride::Custom("hidden".to_string())),
					],
					output_names: vec!["Data".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: None,
		},
		DocumentNodeDefinition {
			identifier: "Boolean Operation",
			category: "Vector",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(1), 0)],
						nodes: vec![
							DocumentNode {
								inputs: vec![NodeInput::import(concrete!(Table<Vector>), 0), NodeInput::import(concrete!(vector::style::Fill), 1)],
								implementation: DocumentNodeImplementation::ProtoNode(path_bool::boolean_operation::IDENTIFIER),
								call_argument: generic!(T),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(0), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(memo::memo::IDENTIFIER),
								call_argument: generic!(T),
								..Default::default()
							},
						]
						.into_iter()
						.enumerate()
						.map(|(id, node)| (NodeId(id as u64), node))
						.collect(),
						..Default::default()
					}),
					inputs: vec![
						NodeInput::value(TaggedValue::Graphic(Default::default()), true),
						NodeInput::value(TaggedValue::BooleanOperation(path_bool::BooleanOperation::Union), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(7, 0)),
										..Default::default()
									},
									..Default::default()
								},
							]
							.into_iter()
							.enumerate()
							.map(|(id, node)| (NodeId(id as u64), node))
							.collect(),
							..Default::default()
						},
						..Default::default()
					}),
					input_metadata: vec![("Content", "TODO").into(), ("Operation", "TODO").into()],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: None,
		},
		DocumentNodeDefinition {
			identifier: "Sample Polyline",
			category: "Vector: Modifier",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(2), 0)],
						nodes: [
							DocumentNode {
								inputs: vec![NodeInput::import(concrete!(Table<Vector>), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(vector::subpath_segment_lengths::IDENTIFIER),
								call_argument: generic!(T),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![
									NodeInput::import(concrete!(Table<Vector>), 0),
									NodeInput::import(concrete!(vector::misc::PointSpacingType), 1),
									NodeInput::import(concrete!(f64), 2),
									NodeInput::import(concrete!(u32), 3),
									NodeInput::import(concrete!(f64), 4),
									NodeInput::import(concrete!(f64), 5),
									NodeInput::import(concrete!(bool), 6),
									NodeInput::node(NodeId(0), 0),
								],
								implementation: DocumentNodeImplementation::ProtoNode(vector::sample_polyline::IDENTIFIER),
								call_argument: generic!(T),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(1), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(memo::memo::IDENTIFIER),
								call_argument: generic!(T),
								..Default::default()
							},
						]
						.into_iter()
						.enumerate()
						.map(|(id, node)| (NodeId(id as u64), node))
						.collect(),
						..Default::default()
					}),
					inputs: vec![
						NodeInput::value(TaggedValue::Vector(Default::default()), true),
						NodeInput::value(TaggedValue::PointSpacingType(Default::default()), false),
						NodeInput::value(TaggedValue::F64(100.), false),
						NodeInput::value(TaggedValue::U32(100), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::Bool(false), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 7)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(7, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(14, 0)),
										..Default::default()
									},
									..Default::default()
								},
							]
							.into_iter()
							.enumerate()
							.map(|(id, node)| (NodeId(id as u64), node))
							.collect(),
							..Default::default()
						},
						..Default::default()
					}),
					input_metadata: vec![
						("Content", "The shape to be resampled and converted into a polyline.").into(),
						("Spacing", node_properties::SAMPLE_POLYLINE_DESCRIPTION_SPACING).into(),
						InputMetadata::with_name_description_override(
							"Separation",
							node_properties::SAMPLE_POLYLINE_DESCRIPTION_SEPARATION,
							WidgetOverride::Number(NumberInputSettings {
								min: Some(0.),
								unit: Some(" px".to_string()),
								..Default::default()
							}),
						),
						InputMetadata::with_name_description_override(
							"Quantity",
							node_properties::SAMPLE_POLYLINE_DESCRIPTION_QUANTITY,
							WidgetOverride::Number(NumberInputSettings {
								min: Some(2.),
								is_integer: true,
								..Default::default()
							}),
						),
						InputMetadata::with_name_description_override(
							"Start Offset",
							node_properties::SAMPLE_POLYLINE_DESCRIPTION_START_OFFSET,
							WidgetOverride::Number(NumberInputSettings {
								min: Some(0.),
								unit: Some(" px".to_string()),
								..Default::default()
							}),
						),
						InputMetadata::with_name_description_override(
							"Stop Offset",
							node_properties::SAMPLE_POLYLINE_DESCRIPTION_STOP_OFFSET,
							WidgetOverride::Number(NumberInputSettings {
								min: Some(0.),
								unit: Some(" px".to_string()),
								..Default::default()
							}),
						),
						("Adaptive Spacing", node_properties::SAMPLE_POLYLINE_DESCRIPTION_ADAPTIVE_SPACING).into(),
					],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("Convert vector geometry into a polyline composed of evenly spaced points."),
			properties: Some("sample_polyline_properties"),
		},
		DocumentNodeDefinition {
			identifier: "Scatter Points",
			category: "Vector: Modifier",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(1), 0)],
						nodes: [
							DocumentNode {
								inputs: vec![
									NodeInput::import(concrete!(Table<Vector>), 0),
									NodeInput::import(concrete!(f64), 1),
									NodeInput::import(concrete!(u32), 2),
								],
								call_argument: generic!(T),
								implementation: DocumentNodeImplementation::ProtoNode(vector::poisson_disk_points::IDENTIFIER),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(0), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(memo::memo::IDENTIFIER),
								call_argument: generic!(T),
								..Default::default()
							},
						]
						.into_iter()
						.enumerate()
						.map(|(id, node)| (NodeId(id as u64), node))
						.collect(),
						..Default::default()
					}),
					inputs: vec![
						NodeInput::value(TaggedValue::Vector(Default::default()), true),
						NodeInput::value(TaggedValue::F64(10.), false),
						NodeInput::value(TaggedValue::U32(0), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(7, 0)),
										..Default::default()
									},
									..Default::default()
								},
							]
							.into_iter()
							.enumerate()
							.map(|(id, node)| (NodeId(id as u64), node))
							.collect(),
							..Default::default()
						},
						..Default::default()
					}),
					input_metadata: vec![
						("Content", "TODO").into(),
						InputMetadata::with_name_description_override(
							"Separation Disk Diameter",
							"TODO",
							WidgetOverride::Number(NumberInputSettings {
								min: Some(0.01),
								mode: NumberInputMode::Range,
								range_min: Some(1.),
								range_max: Some(100.),
								..Default::default()
							}),
						),
						InputMetadata::with_name_description_override(
							"Seed",
							"TODO",
							WidgetOverride::Number(NumberInputSettings {
								min: Some(0.),
								is_integer: true,
								..Default::default()
							}),
						),
					],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: None,
		},
	];

	document_node_derive::post_process_nodes(custom)
}

type NodeProperties = HashMap<String, Box<dyn Fn(NodeId, &mut NodePropertiesContext) -> Vec<LayoutGroup> + Send + Sync>>;

pub static NODE_OVERRIDES: once_cell::sync::Lazy<NodeProperties> = once_cell::sync::Lazy::new(static_node_properties);

/// Defines the logic for inputs to display a custom properties panel widget.
fn static_node_properties() -> NodeProperties {
	let mut map: NodeProperties = HashMap::new();
	map.insert("brightness_contrast_properties".to_string(), Box::new(node_properties::brightness_contrast_properties));
	map.insert("channel_mixer_properties".to_string(), Box::new(node_properties::channel_mixer_properties));
	map.insert("fill_properties".to_string(), Box::new(node_properties::fill_properties));
	map.insert("stroke_properties".to_string(), Box::new(node_properties::stroke_properties));
	map.insert("offset_path_properties".to_string(), Box::new(node_properties::offset_path_properties));
	map.insert("selective_color_properties".to_string(), Box::new(node_properties::selective_color_properties));
	map.insert("exposure_properties".to_string(), Box::new(node_properties::exposure_properties));
	map.insert("math_properties".to_string(), Box::new(node_properties::math_properties));
	map.insert("rectangle_properties".to_string(), Box::new(node_properties::rectangle_properties));
	map.insert("grid_properties".to_string(), Box::new(node_properties::grid_properties));
	map.insert("spiral_properties".to_string(), Box::new(node_properties::spiral_properties));
	map.insert("sample_polyline_properties".to_string(), Box::new(node_properties::sample_polyline_properties));
	map.insert(
		"monitor_properties".to_string(),
		Box::new(|_node_id, _context| node_properties::string_properties("Used internally by the editor to obtain a layer thumbnail.")),
	);
	map
}

type InputProperties = HashMap<String, Box<dyn Fn(NodeId, usize, &mut NodePropertiesContext) -> Result<Vec<LayoutGroup>, String> + Send + Sync>>;

static INPUT_OVERRIDES: once_cell::sync::Lazy<InputProperties> = once_cell::sync::Lazy::new(static_input_properties);

/// Defines the logic for inputs to display a custom properties panel widget.
fn static_input_properties() -> InputProperties {
	let mut map: InputProperties = HashMap::new();
	map.insert("hidden".to_string(), Box::new(|_node_id, _index, _context| Ok(Vec::new())));
	map.insert(
		"string".to_string(),
		Box::new(|node_id, index, context| {
			let Some(value) = context.network_interface.input_data(&node_id, index, "string_properties", context.selection_network_path) else {
				return Err(format!("Could not get string properties for node {node_id}"));
			};
			let Some(string) = value.as_str() else {
				return Err(format!("Could not downcast string properties for node {node_id}"));
			};
			Ok(node_properties::string_properties(string))
		}),
	);
	map.insert(
		"number".to_string(),
		Box::new(|node_id, index, context| {
			let mut number_input = NumberInput::default();
			if let Some(unit) = context
				.network_interface
				.input_data(&node_id, index, "unit", context.selection_network_path)
				.and_then(|value| value.as_str())
			{
				number_input = number_input.unit(unit);
			}
			if let Some(min) = context
				.network_interface
				.input_data(&node_id, index, "min", context.selection_network_path)
				.and_then(|value| value.as_f64())
			{
				number_input = number_input.min(min);
			}
			if let Some(max) = context
				.network_interface
				.input_data(&node_id, index, "max", context.selection_network_path)
				.and_then(|value| value.as_f64())
			{
				number_input = number_input.max(max);
			}
			if let Some(step) = context
				.network_interface
				.input_data(&node_id, index, "step", context.selection_network_path)
				.and_then(|value| value.as_f64())
			{
				number_input = number_input.step(step);
			}
			if let Some(mode) = context.network_interface.input_data(&node_id, index, "mode", context.selection_network_path).map(|value| {
				let mode: NumberInputMode = serde_json::from_value(value.clone()).unwrap();
				mode
			}) {
				number_input = number_input.mode(mode);
			}
			if let Some(range_min) = context
				.network_interface
				.input_data(&node_id, index, "range_min", context.selection_network_path)
				.and_then(|value| value.as_f64())
			{
				number_input = number_input.range_min(Some(range_min));
			}
			if let Some(range_max) = context
				.network_interface
				.input_data(&node_id, index, "range_max", context.selection_network_path)
				.and_then(|value| value.as_f64())
			{
				number_input = number_input.range_max(Some(range_max));
			}
			if let Some(is_integer) = context
				.network_interface
				.input_data(&node_id, index, "is_integer", context.selection_network_path)
				.and_then(|value| value.as_bool())
			{
				number_input = number_input.is_integer(is_integer);
			}
			let blank_assist = context
				.network_interface
				.input_data(&node_id, index, "blank_assist", context.selection_network_path)
				.and_then(|value| value.as_bool())
				.unwrap_or_else(|| {
					log::error!("Could not get blank assist when displaying number input for node {node_id}, index {index}");
					true
				});

			Ok(vec![LayoutGroup::Row {
				widgets: node_properties::number_widget(ParameterWidgetsInfo::new(node_id, index, blank_assist, context), number_input),
			}])
		}),
	);
	map.insert(
		// The custom number input settings are only available on proto nodes
		"optional_f64".to_string(),
		Box::new(|node_id, index, context| {
			let node_metadata = registry::NODE_METADATA.lock().unwrap();
			let mut number_input = NumberInput::default();
			if let Some(field) = context
				.network_interface
				.implementation(&node_id, context.selection_network_path)
				.and_then(|implementation| if let DocumentNodeImplementation::ProtoNode(id) = implementation { Some(id) } else { None })
				.and_then(|proto_node_identifier| node_metadata.get(proto_node_identifier))
				.and_then(|metadata| metadata.fields.get(index))
			{
				if let Some(unit) = field.unit {
					number_input = number_input.unit(unit);
				}
				if let Some(number_min) = field.number_min {
					number_input = number_input.min(number_min);
				}
				if let Some(number_max) = field.number_max {
					number_input = number_input.max(number_max);
				}
				if let Some((range_min, range_max)) = field.number_mode_range {
					number_input = number_input.range_min(Some(range_min));
					number_input = number_input.range_max(Some(range_max));
				}
				number_input = number_input.is_integer(false);
				if let Some(number_step) = field.number_step {
					number_input = number_input.step(number_step);
				}
			};
			Ok(vec![LayoutGroup::Row {
				// NOTE: The bool input MUST be at the input index directly before the f64 input!
				widgets: node_properties::optional_f64_widget(ParameterWidgetsInfo::new(node_id, index, false, context), index - 1, number_input),
			}])
		}),
	);
	map.insert(
		"vec2".to_string(),
		Box::new(|node_id, index, context| {
			let x = context
				.network_interface
				.input_data(&node_id, index, "x", context.selection_network_path)
				.and_then(|value| value.as_str())
				.unwrap_or_else(|| {
					log::error!("Could not get x for vec2 input");
					""
				})
				.to_string();
			let y = context
				.network_interface
				.input_data(&node_id, index, "y", context.selection_network_path)
				.and_then(|value| value.as_str())
				.unwrap_or_else(|| {
					log::error!("Could not get y for vec2 input");
					""
				})
				.to_string();
			let unit = context
				.network_interface
				.input_data(&node_id, index, "unit", context.selection_network_path)
				.and_then(|value| value.as_str())
				.unwrap_or_else(|| {
					log::error!("Could not get unit for vec2 input");
					""
				})
				.to_string();
			let min = context
				.network_interface
				.input_data(&node_id, index, "min", context.selection_network_path)
				.and_then(|value| value.as_f64());
			let is_integer = context
				.network_interface
				.input_data(&node_id, index, "is_integer", context.selection_network_path)
				.and_then(|value| value.as_bool())
				.unwrap_or_default();

			Ok(vec![node_properties::vec2_widget(
				ParameterWidgetsInfo::new(node_id, index, true, context),
				&x,
				&y,
				&unit,
				min,
				is_integer,
			)])
		}),
	);
	map.insert(
		"noise_properties_scale".to_string(),
		Box::new(|node_id, index, context| {
			let (_, coherent_noise_active, _, _, _, _) = node_properties::query_noise_pattern_state(node_id, context)?;
			let scale = node_properties::number_widget(
				ParameterWidgetsInfo::new(node_id, index, true, context),
				NumberInput::default().min(0.).disabled(!coherent_noise_active),
			);
			Ok(vec![scale.into()])
		}),
	);
	map.insert(
		"noise_properties_noise_type".to_string(),
		Box::new(|node_id, index, context| {
			let noise_type_row = enum_choice::<NoiseType>().for_socket(ParameterWidgetsInfo::new(node_id, index, true, context)).property_row();
			Ok(vec![noise_type_row, LayoutGroup::Row { widgets: Vec::new() }])
		}),
	);
	map.insert(
		"noise_properties_domain_warp_type".to_string(),
		Box::new(|node_id, index, context| {
			let (_, coherent_noise_active, _, _, _, _) = node_properties::query_noise_pattern_state(node_id, context)?;
			let domain_warp_type = enum_choice::<DomainWarpType>()
				.for_socket(ParameterWidgetsInfo::new(node_id, index, true, context))
				.disabled(!coherent_noise_active)
				.property_row();
			Ok(vec![domain_warp_type])
		}),
	);
	map.insert(
		"noise_properties_domain_warp_amplitude".to_string(),
		Box::new(|node_id, index, context| {
			let (_, coherent_noise_active, _, _, domain_warp_active, _) = node_properties::query_noise_pattern_state(node_id, context)?;
			let domain_warp_amplitude = node_properties::number_widget(
				ParameterWidgetsInfo::new(node_id, index, true, context),
				NumberInput::default().min(0.).disabled(!coherent_noise_active || !domain_warp_active),
			);
			Ok(vec![domain_warp_amplitude.into(), LayoutGroup::Row { widgets: Vec::new() }])
		}),
	);
	map.insert(
		"noise_properties_fractal_type".to_string(),
		Box::new(|node_id, index, context| {
			let (_, coherent_noise_active, _, _, _, _) = node_properties::query_noise_pattern_state(node_id, context)?;
			let fractal_type_row = enum_choice::<FractalType>()
				.for_socket(ParameterWidgetsInfo::new(node_id, index, true, context))
				.disabled(!coherent_noise_active)
				.property_row();
			Ok(vec![fractal_type_row])
		}),
	);
	map.insert(
		"noise_properties_fractal_octaves".to_string(),
		Box::new(|node_id, index, context| {
			let (fractal_active, coherent_noise_active, _, _, _, domain_warp_only_fractal_type_wrongly_active) = node_properties::query_noise_pattern_state(node_id, context)?;
			let fractal_octaves = node_properties::number_widget(
				ParameterWidgetsInfo::new(node_id, index, true, context),
				NumberInput::default()
					.mode_range()
					.min(1.)
					.max(10.)
					.range_max(Some(4.))
					.is_integer(true)
					.disabled(!coherent_noise_active || !fractal_active || domain_warp_only_fractal_type_wrongly_active),
			);
			Ok(vec![fractal_octaves.into()])
		}),
	);
	map.insert(
		"noise_properties_fractal_lacunarity".to_string(),
		Box::new(|node_id, index, context| {
			let (fractal_active, coherent_noise_active, _, _, _, domain_warp_only_fractal_type_wrongly_active) = node_properties::query_noise_pattern_state(node_id, context)?;
			let fractal_lacunarity = node_properties::number_widget(
				ParameterWidgetsInfo::new(node_id, index, true, context),
				NumberInput::default()
					.mode_range()
					.min(0.)
					.range_max(Some(10.))
					.disabled(!coherent_noise_active || !fractal_active || domain_warp_only_fractal_type_wrongly_active),
			);
			Ok(vec![fractal_lacunarity.into()])
		}),
	);
	map.insert(
		"noise_properties_fractal_gain".to_string(),
		Box::new(|node_id, index, context| {
			let (fractal_active, coherent_noise_active, _, _, _, domain_warp_only_fractal_type_wrongly_active) = node_properties::query_noise_pattern_state(node_id, context)?;
			let fractal_gain = node_properties::number_widget(
				ParameterWidgetsInfo::new(node_id, index, true, context),
				NumberInput::default()
					.mode_range()
					.min(0.)
					.range_max(Some(10.))
					.disabled(!coherent_noise_active || !fractal_active || domain_warp_only_fractal_type_wrongly_active),
			);
			Ok(vec![fractal_gain.into()])
		}),
	);
	map.insert(
		"noise_properties_fractal_weighted_strength".to_string(),
		Box::new(|node_id, index, context| {
			let (fractal_active, coherent_noise_active, _, _, _, domain_warp_only_fractal_type_wrongly_active) = node_properties::query_noise_pattern_state(node_id, context)?;
			let fractal_weighted_strength = node_properties::number_widget(
				ParameterWidgetsInfo::new(node_id, index, true, context),
				NumberInput::default()
					.mode_range()
					.min(0.)
					.max(1.) // Defined for the 0-1 range
					.disabled(!coherent_noise_active || !fractal_active || domain_warp_only_fractal_type_wrongly_active),
			);
			Ok(vec![fractal_weighted_strength.into()])
		}),
	);
	map.insert(
		"noise_properties_ping_pong_strength".to_string(),
		Box::new(|node_id, index, context| {
			let (fractal_active, coherent_noise_active, _, ping_pong_active, _, domain_warp_only_fractal_type_wrongly_active) = node_properties::query_noise_pattern_state(node_id, context)?;
			let fractal_ping_pong_strength = node_properties::number_widget(
				ParameterWidgetsInfo::new(node_id, index, true, context),
				NumberInput::default()
					.mode_range()
					.min(0.)
					.range_max(Some(10.))
					.disabled(!ping_pong_active || !coherent_noise_active || !fractal_active || domain_warp_only_fractal_type_wrongly_active),
			);
			Ok(vec![fractal_ping_pong_strength.into(), LayoutGroup::Row { widgets: Vec::new() }])
		}),
	);
	map.insert(
		"noise_properties_cellular_distance_function".to_string(),
		Box::new(|node_id, index, context| {
			let (_, coherent_noise_active, cellular_noise_active, _, _, _) = node_properties::query_noise_pattern_state(node_id, context)?;
			let cellular_distance_function_row = enum_choice::<CellularDistanceFunction>()
				.for_socket(ParameterWidgetsInfo::new(node_id, index, true, context))
				.disabled(!coherent_noise_active || !cellular_noise_active)
				.property_row();
			Ok(vec![cellular_distance_function_row])
		}),
	);
	map.insert(
		"noise_properties_cellular_return_type".to_string(),
		Box::new(|node_id, index, context| {
			let (_, coherent_noise_active, cellular_noise_active, _, _, _) = node_properties::query_noise_pattern_state(node_id, context)?;
			let cellular_return_type = enum_choice::<CellularReturnType>()
				.for_socket(ParameterWidgetsInfo::new(node_id, index, true, context))
				.disabled(!coherent_noise_active || !cellular_noise_active)
				.property_row();
			Ok(vec![cellular_return_type])
		}),
	);
	map.insert(
		"noise_properties_cellular_jitter".to_string(),
		Box::new(|node_id, index, context| {
			let (_, coherent_noise_active, cellular_noise_active, _, _, _) = node_properties::query_noise_pattern_state(node_id, context)?;
			let cellular_jitter = node_properties::number_widget(
				ParameterWidgetsInfo::new(node_id, index, true, context),
				NumberInput::default()
					.mode_range()
					.range_min(Some(0.))
					.range_max(Some(1.))
					.disabled(!coherent_noise_active || !cellular_noise_active),
			);
			Ok(vec![cellular_jitter.into()])
		}),
	);
	map.insert(
		"assign_colors_gradient".to_string(),
		Box::new(|node_id, index, context| {
			let gradient_row = node_properties::color_widget(ParameterWidgetsInfo::new(node_id, index, true, context), ColorInput::default().allow_none(false));
			Ok(vec![gradient_row])
		}),
	);
	map.insert(
		"assign_colors_seed".to_string(),
		Box::new(|node_id, index, context| {
			let randomize_enabled = node_properties::query_assign_colors_randomize(node_id, context)?;
			let seed_row = node_properties::number_widget(
				ParameterWidgetsInfo::new(node_id, index, true, context),
				NumberInput::default().min(0.).int().disabled(!randomize_enabled),
			);
			Ok(vec![seed_row.into()])
		}),
	);
	map.insert(
		"assign_colors_repeat_every".to_string(),
		Box::new(|node_id, index, context| {
			let randomize_enabled = node_properties::query_assign_colors_randomize(node_id, context)?;
			let repeat_every_row = node_properties::number_widget(
				ParameterWidgetsInfo::new(node_id, index, true, context),
				NumberInput::default().min(0.).int().disabled(randomize_enabled),
			);
			Ok(vec![repeat_every_row.into()])
		}),
	);
	map.insert(
		"transform_rotation".to_string(),
		Box::new(|node_id, index, context| {
			let mut widgets = node_properties::start_widgets(ParameterWidgetsInfo::new(node_id, index, true, context));

			let document_node = node_properties::get_document_node(node_id, context)?;
			let Some(input) = document_node.inputs.get(index) else {
				return Err("Input not found in transform rotation input override".to_string());
			};
			if let Some(&TaggedValue::F64(val)) = input.as_non_exposed_value() {
				widgets.extend_from_slice(&[
					Separator::new(SeparatorStyle::Unrelated).widget_instance(),
					NumberInput::new(Some(val))
						.unit("°")
						.mode(NumberInputMode::Range)
						.range_min(Some(-180.))
						.range_max(Some(180.))
						.on_update(node_properties::update_value(
							|number_input: &NumberInput| TaggedValue::F64(number_input.value.unwrap()),
							node_id,
							index,
						))
						.on_commit(node_properties::commit_value)
						.widget_instance(),
				]);
			}

			Ok(vec![LayoutGroup::Row { widgets }])
		}),
	);
	// Skew has a custom override that maps to degrees
	map.insert(
		"transform_skew".to_string(),
		Box::new(|node_id, index, context| {
			let mut widgets = node_properties::start_widgets(ParameterWidgetsInfo::new(node_id, index, true, context));

			let document_node = node_properties::get_document_node(node_id, context)?;
			let Some(input) = document_node.inputs.get(index) else {
				return Err("Input not found in transform skew input override".to_string());
			};
			if let Some(&TaggedValue::DVec2(val)) = input.as_non_exposed_value() {
				widgets.extend_from_slice(&[
					Separator::new(SeparatorStyle::Unrelated).widget_instance(),
					NumberInput::new(Some(val.x))
						.label("X")
						.unit("°")
						.min(-89.9)
						.max(89.9)
						.on_update(node_properties::update_value(
							move |input: &NumberInput| TaggedValue::DVec2(DVec2::new(input.value.unwrap(), val.y)),
							node_id,
							index,
						))
						.on_commit(node_properties::commit_value)
						.widget_instance(),
					Separator::new(SeparatorStyle::Related).widget_instance(),
					NumberInput::new(Some(val.y))
						.label("Y")
						.unit("°")
						.min(-89.9)
						.max(89.9)
						.on_update(node_properties::update_value(
							move |input: &NumberInput| TaggedValue::DVec2(DVec2::new(val.x, input.value.unwrap())),
							node_id,
							index,
						))
						.on_commit(node_properties::commit_value)
						.widget_instance(),
				]);
			}

			Ok(vec![LayoutGroup::Row { widgets }])
		}),
	);
	map.insert(
		"text_area".to_string(),
		Box::new(|node_id, index, context| {
			Ok(vec![LayoutGroup::Row {
				widgets: node_properties::text_area_widget(ParameterWidgetsInfo::new(node_id, index, true, context)),
			}])
		}),
	);
	map.insert(
		"text_font".to_string(),
		Box::new(|node_id, index, context| {
			let (font, style) = node_properties::font_inputs(ParameterWidgetsInfo::new(node_id, index, true, context));
			let mut result = vec![LayoutGroup::Row { widgets: font }];
			if let Some(style) = style {
				result.push(LayoutGroup::Row { widgets: style });
			}
			Ok(result)
		}),
	);
	map.insert(
		"artboard_background".to_string(),
		Box::new(|node_id, index, context| {
			Ok(vec![node_properties::color_widget(
				ParameterWidgetsInfo::new(node_id, index, true, context),
				ColorInput::default().allow_none(false),
			)])
		}),
	);
	map.insert(
		"text_align".to_string(),
		Box::new(|node_id, index, context| {
			let choices = enum_choice::<text::TextAlign>().for_socket(ParameterWidgetsInfo::new(node_id, index, true, context)).property_row();
			Ok(vec![choices])
		}),
	);
	map
}

pub fn resolve_network_node_type(identifier: &str) -> Option<&'static DocumentNodeDefinition> {
	resolve_document_node_type(&DefinitionIdentifier::Network(identifier.into()))
}

pub fn resolve_proto_node_type(identifier: ProtoNodeIdentifier) -> Option<&'static DocumentNodeDefinition> {
	resolve_document_node_type(&DefinitionIdentifier::ProtoNode(identifier))
}

pub fn resolve_document_node_type(identifier: &DefinitionIdentifier) -> Option<&'static DocumentNodeDefinition> {
	DOCUMENT_NODE_TYPES.get(identifier)
}

pub fn collect_node_types() -> Vec<FrontendNodeType> {
	DOCUMENT_NODE_TYPES
		.iter()
		.filter(|(_, definition)| !definition.category.is_empty())
		.map(|(identifier, definition)| {
			let input_types = definition
				.node_template
				.document_node
				.inputs
				.iter()
				.map(|node_input| node_input.as_value().map(|node_value| node_value.ty().nested_type().to_string()).unwrap_or_default())
				.collect::<Vec<String>>();
			let mut name = definition.node_template.persistent_node_metadata.display_name.clone();
			if name.is_empty() {
				name = identifier.implementation_name_from_identifier()
			}
			FrontendNodeType {
				identifier: identifier.clone(),
				name,
				category: definition.category.to_string(),
				input_types,
			}
		})
		.collect()
}

pub fn collect_node_descriptions() -> Vec<(DefinitionIdentifier, String)> {
	DOCUMENT_NODE_TYPES
		.iter()
		.map(|(identifier, definition)| (identifier.clone(), if definition.description != "TODO" { definition.description.to_string() } else { String::new() }))
		.collect()
}

impl DocumentNodeDefinition {
	/// Converts the [DocumentNodeDefinition] type to a [NodeTemplate], using the provided `input_override` and falling back to the default inputs.
	/// `input_override` does not have to be the correct length.
	pub fn node_template_input_override(&self, input_override: impl IntoIterator<Item = Option<NodeInput>>) -> NodeTemplate {
		let mut template = self.node_template.clone();
		// TODO: Replace the .enumerate() with changing the iterator to take a tuple of (index, input) so the user is forced to provide the correct index
		input_override.into_iter().enumerate().for_each(|(index, input_override)| {
			if let Some(input_override) = input_override {
				// Only value inputs should be overridden, since node inputs change graph structure and must be handled by the network interface.
				// However, this would require changing some tooling which creates multiple nodes at once, before they are inserted into the network.
				// debug_assert!(input_override.as_node().is_none(), "Node inputs are not supported in input overrides");
				template.document_node.inputs[index] = input_override;
			}
		});

		// Ensure that the input properties are initialized for every Document Node input for every node
		fn populate_input_properties(node_template: &mut NodeTemplate, mut path: Vec<NodeId>) {
			if let Some(current_node) = path.pop() {
				let DocumentNodeImplementation::Network(template_network) = &node_template.document_node.implementation else {
					log::error!("Template network should always exist");
					return;
				};
				let Some(nested_network) = template_network.nested_network(&path) else {
					log::error!("Nested network should exist for path");
					return;
				};
				let Some(input_length) = nested_network.nodes.get(&current_node).map(|node| node.inputs.len()) else {
					log::error!("Could not get current node in nested network");
					return;
				};
				let Some(template_network_metadata) = &mut node_template.persistent_node_metadata.network_metadata else {
					log::error!("Template should have metadata if it has network implementation");
					return;
				};
				let Some(nested_network_metadata) = template_network_metadata.nested_metadata_mut(&path) else {
					log::error!("Path is not valid for network");
					return;
				};
				let Some(nested_node_metadata) = nested_network_metadata.persistent_metadata.node_metadata.get_mut(&current_node) else {
					log::error!("Path is not valid for network");
					return;
				};
				nested_node_metadata.persistent_metadata.input_metadata.resize_with(input_length, InputMetadata::default);

				// Recurse over all sub-nodes if the current node is a network implementation
				let mut current_path = path.clone();
				current_path.push(current_node);
				let DocumentNodeImplementation::Network(template_network) = &node_template.document_node.implementation else {
					log::error!("Template network should always exist");
					return;
				};
				if let Some(current_nested_network) = template_network.nested_network(&current_path) {
					for sub_node_id in current_nested_network.nodes.keys().cloned().collect::<Vec<_>>() {
						let mut sub_path = current_path.clone();
						sub_path.push(sub_node_id);
						populate_input_properties(node_template, sub_path);
					}
				};
			} else {
				// Base case
				let input_len = node_template.document_node.inputs.len();
				node_template.persistent_node_metadata.input_metadata.resize_with(input_len, InputMetadata::default);
				if let DocumentNodeImplementation::Network(node_template_network) = &node_template.document_node.implementation {
					for sub_node_id in node_template_network.nodes.keys().cloned().collect::<Vec<_>>() {
						populate_input_properties(node_template, vec![sub_node_id]);
					}
				}
			}
		}
		populate_input_properties(&mut template, Vec::new());

		template
	}

	/// Converts the [DocumentNodeDefinition] type to a [NodeTemplate], completely default.
	pub fn default_node_template(&self) -> NodeTemplate {
		self.node_template_input_override(self.node_template.document_node.inputs.clone().into_iter().map(Some))
	}
}
