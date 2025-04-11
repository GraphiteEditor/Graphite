use super::node_properties;
use super::utility_types::FrontendNodeType;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::utility_types::network_interface::{
	DocumentNodeMetadata, DocumentNodePersistentMetadata, NodeNetworkInterface, NodeNetworkMetadata, NodeNetworkPersistentMetadata, NodeTemplate, NodeTypePersistentMetadata, NumberInputSettings,
	PropertiesRow, Vec2InputSettings, WidgetOverride,
};
use crate::messages::portfolio::utility_types::PersistentData;
use crate::messages::prelude::Message;
use crate::node_graph_executor::NodeGraphExecutor;
use glam::DVec2;
use graph_craft::ProtoNodeIdentifier;
use graph_craft::concrete;
use graph_craft::document::value::*;
use graph_craft::document::*;
use graph_craft::imaginate_input::ImaginateSamplingMethod;
use graphene_core::raster::brush_cache::BrushCache;
use graphene_core::raster::image::ImageFrameTable;
use graphene_core::raster::{CellularDistanceFunction, CellularReturnType, Color, DomainWarpType, FractalType, NoiseType, RedGreenBlue, RedGreenBlueAlpha};
use graphene_core::text::{Font, TypesettingConfig};
use graphene_core::transform::Footprint;
use graphene_core::vector::VectorDataTable;
use graphene_core::*;
use graphene_std::wasm_application_io::WasmEditorApi;
use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet, VecDeque};
#[cfg(feature = "gpu")]
use wgpu_executor::{Bindgroup, CommandBuffer, PipelineLayout, ShaderHandle, ShaderInputFrame, WgpuShaderInput};

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
		let input_properties_row = self.network_interface.input_properties_row(node_id, index, self.selection_network_path)?;
		if let Some(widget_override) = &input_properties_row.widget_override {
			let Some(widget_override_lambda) = INPUT_OVERRIDES.get(widget_override) else {
				log::error!("Could not get widget override lambda in call_widget_override");
				return None;
			};
			widget_override_lambda(*node_id, index, self)
				.map(|layout_group| {
					let Some(input_properties_row) = self.network_interface.input_properties_row(node_id, index, self.selection_network_path) else {
						log::error!("Could not get input properties row in call_widget_override");
						return Vec::new();
					};
					match &input_properties_row.input_data.get("tooltip").and_then(|tooltip| tooltip.as_str()) {
						Some(tooltip) => layout_group.into_iter().map(|widget| widget.with_tooltip(*tooltip)).collect::<Vec<_>>(),
						_ => layout_group,
					}
				})
				.map_err(|error| {
					log::error!("Error in widget override lambda: {}", error);
				})
				.ok()
		} else {
			None
		}
	}
}

/// Acts as a description for a [DocumentNode] before it gets instantiated as one.
#[derive(Clone)]
pub struct DocumentNodeDefinition {
	/// Used by the reference field in [`DocumentNodeMetadata`] to prevent storing a copy of the implementation, if it is unchanged from the definition.
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

// We use the once cell for lazy initialization to avoid the overhead of reconstructing the node list every time.
// TODO: make document nodes not require a `'static` lifetime to avoid having to split the construction into const and non-const parts.
static DOCUMENT_NODE_TYPES: once_cell::sync::Lazy<Vec<DocumentNodeDefinition>> = once_cell::sync::Lazy::new(static_nodes);

// TODO: Dynamic node library
/// Defines the "signature" or "header file"-like metadata for the document nodes, but not the implementation (which is defined in the node registry).
/// The [`DocumentNode`] is the instance while these [`DocumentNodeDefinition`]s are the "classes" or "blueprints" from which the instances are built.
fn static_nodes() -> Vec<DocumentNodeDefinition> {
	let mut custom = vec![
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
			description: Cow::Borrowed("A default node network you can use to create your own custom nodes."),
			properties: None,
		},
		// TODO: Auto-generate this from its proto node macro
		DocumentNodeDefinition {
			identifier: "Identity",
			category: "General",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::ops::IdentityNode"),
					inputs: vec![NodeInput::value(TaggedValue::None, true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_properties: vec!["In".into()],
					output_names: vec!["Out".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("The identity node passes its data through. You can use this to organize your node graph."),
			properties: Some("identity_properties"),
		},
		// TODO: Auto-generate this from its proto node macro
		DocumentNodeDefinition {
			identifier: "Monitor",
			category: "Debug",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::memo::MonitorNode"),
					inputs: vec![NodeInput::value(TaggedValue::None, true)],
					manual_composition: Some(generic!(T)),
					skip_deduplication: true,
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_properties: vec!["In".into()],
					output_names: vec!["Out".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("The Monitor node is used by the editor to access the data flowing through it."),
			properties: Some("monitor_properties"),
		},
		DocumentNodeDefinition {
			identifier: "Merge",
			category: "General",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(3), 0)],
						nodes: [
							// Secondary (left) input type coercion
							DocumentNode {
								inputs: vec![NodeInput::network(generic!(T), 1)],
								implementation: DocumentNodeImplementation::proto("graphene_core::graphic_element::ToElementNode"),
								manual_composition: Some(generic!(T)),
								..Default::default()
							},
							// Primary (bottom) input type coercion
							DocumentNode {
								inputs: vec![NodeInput::network(generic!(T), 0)],
								implementation: DocumentNodeImplementation::proto("graphene_core::graphic_element::ToGroupNode"),
								manual_composition: Some(generic!(T)),
								..Default::default()
							},
							// The monitor node is used to display a thumbnail in the UI
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(0), 0)],
								implementation: DocumentNodeImplementation::proto("graphene_core::memo::MonitorNode"),
								manual_composition: Some(generic!(T)),
								skip_deduplication: true,
								..Default::default()
							},
							DocumentNode {
								manual_composition: Some(generic!(T)),
								inputs: vec![
									NodeInput::node(NodeId(1), 0),
									NodeInput::node(NodeId(2), 0),
									NodeInput::Reflection(graph_craft::document::DocumentNodeMetadata::DocumentNodePath),
								],
								implementation: DocumentNodeImplementation::proto("graphene_core::graphic_element::LayerNode"),
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
						NodeInput::value(TaggedValue::GraphicGroup(GraphicGroupTable::default()), true),
						NodeInput::value(TaggedValue::GraphicGroup(GraphicGroupTable::default()), true),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_properties: vec!["Graphical Data".into(), "Over".into()],
					output_names: vec!["Out".to_string()],
					node_type_metadata: NodeTypePersistentMetadata::layer(IVec2::new(0, 0)),
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "To Element".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(-14, -1)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "To Group".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(-14, -3)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Monitor".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(-7, -1)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Layer".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(1, -3)),
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
			description: Cow::Borrowed("The Merge node combines graphical data through composition."),
			properties: None,
		},
		DocumentNodeDefinition {
			identifier: "Artboard",
			category: "General",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(2), 0)],
						nodes: [
							// Ensure this ID is kept in sync with the ID in set_alias so that the name input is kept in sync with the alias
							DocumentNode {
								manual_composition: Some(generic!(T)),
								implementation: DocumentNodeImplementation::proto("graphene_core::graphic_element::ToArtboardNode"),
								inputs: vec![
									NodeInput::network(concrete!(TaggedValue), 1),
									NodeInput::value(TaggedValue::String(String::from("Artboard")), false),
									NodeInput::network(concrete!(TaggedValue), 2),
									NodeInput::network(concrete!(TaggedValue), 3),
									NodeInput::network(concrete!(TaggedValue), 4),
									NodeInput::network(concrete!(TaggedValue), 5),
								],
								..Default::default()
							},
							// The monitor node is used to display a thumbnail in the UI.
							// TODO: Check if thumbnail is reversed
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(0), 0)],
								implementation: DocumentNodeImplementation::proto("graphene_core::memo::MonitorNode"),
								manual_composition: Some(generic!(T)),
								skip_deduplication: true,
								..Default::default()
							},
							DocumentNode {
								manual_composition: Some(concrete!(Context)),
								inputs: vec![
									NodeInput::network(graphene_core::Type::Fn(Box::new(concrete!(Context)), Box::new(concrete!(ArtboardGroupTable))), 0),
									NodeInput::node(NodeId(1), 0),
									NodeInput::Reflection(graph_craft::document::DocumentNodeMetadata::DocumentNodePath),
								],
								implementation: DocumentNodeImplementation::proto("graphene_core::graphic_element::AppendArtboardNode"),
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
						NodeInput::value(TaggedValue::ArtboardGroup(ArtboardGroupTable::default()), true),
						NodeInput::value(TaggedValue::GraphicGroup(GraphicGroupTable::default()), true),
						NodeInput::value(TaggedValue::IVec2(glam::IVec2::ZERO), false),
						NodeInput::value(TaggedValue::IVec2(glam::IVec2::new(1920, 1080)), false),
						NodeInput::value(TaggedValue::Color(Color::WHITE), false),
						NodeInput::value(TaggedValue::Bool(false), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_properties: vec![
						"Artboards".into(),
						PropertiesRow::with_override("Contents", WidgetOverride::Hidden),
						PropertiesRow::with_override(
							"Location",
							WidgetOverride::Vec2(Vec2InputSettings {
								x: "X".to_string(),
								y: "Y".to_string(),
								unit: " px".to_string(),
								..Default::default()
							}),
						),
						PropertiesRow::with_override(
							"Dimensions",
							WidgetOverride::Vec2(Vec2InputSettings {
								x: "W".to_string(),
								y: "H".to_string(),
								unit: " px".to_string(),
								..Default::default()
							}),
						),
						PropertiesRow::with_override("Background", WidgetOverride::Custom("artboard_background".to_string())),
						"Clip".into(),
					],
					output_names: vec!["Out".to_string()],
					node_type_metadata: NodeTypePersistentMetadata::layer(IVec2::new(0, 0)),
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "To Artboard".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(-10, -3)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Monitor".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(-2, -3)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Append Artboards".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(6, -4)),
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
			identifier: "Load Image",
			category: "Network",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(2), 0)],
						nodes: [
							DocumentNode {
								inputs: vec![NodeInput::value(TaggedValue::None, false), NodeInput::scope("editor-api"), NodeInput::network(concrete!(String), 1)],
								manual_composition: Some(concrete!(Context)),
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::wasm_application_io::LoadResourceNode")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(0), 0)],
								manual_composition: Some(concrete!(Context)),
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::wasm_application_io::DecodeImageNode")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(1), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::transform::CullNode")),
								manual_composition: Some(concrete!(Context)),
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
					input_properties: vec!["Empty".into(), "URL".into()],
					output_names: vec!["Image".to_string()],
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Load Resource".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Decode Image".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Cull".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
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
			description: Cow::Borrowed("Loads an image from a given URL"),
			properties: None,
		},
		DocumentNodeDefinition {
			identifier: "Create Canvas",
			category: "Debug: GPU",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(1), 0)],
						nodes: [
							DocumentNode {
								inputs: vec![NodeInput::scope("editor-api")],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::wasm_application_io::CreateSurfaceNode")),
								skip_deduplication: true,
								..Default::default()
							},
							DocumentNode {
								manual_composition: Some(concrete!(Context)),
								inputs: vec![NodeInput::node(NodeId(0), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::MemoNode")),
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
					output_names: vec!["Image".to_string()],
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Create Canvas".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Cache".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
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
			description: Cow::Borrowed("Creates a new canvas object."),
			properties: None,
		},
		DocumentNodeDefinition {
			identifier: "Draw Canvas",
			category: "Debug: GPU",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(3), 0)],
						nodes: [
							DocumentNode {
								inputs: vec![NodeInput::network(concrete!(ImageFrameTable<Color>), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<_, ImageFrameTable>")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::scope("editor-api")],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::wasm_application_io::CreateSurfaceNode")),
								skip_deduplication: true,
								..Default::default()
							},
							DocumentNode {
								manual_composition: Some(concrete!(Context)),
								inputs: vec![NodeInput::node(NodeId(1), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::MemoNode")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(0), 0), NodeInput::node(NodeId(2), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::wasm_application_io::DrawImageFrameNode")),
								..Default::default()
							},
						]
						.into_iter()
						.enumerate()
						.map(|(id, node)| (NodeId(id as u64), node))
						.collect(),
						..Default::default()
					}),
					inputs: vec![NodeInput::value(TaggedValue::ImageFrame(ImageFrameTable::one_empty_image()), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_properties: vec!["In".into()],
					output_names: vec!["Canvas".to_string()],
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Convert Image Frame".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Create Canvas".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Cache".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Draw Canvas".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
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
			description: Cow::Borrowed("Draws raster data to a canvas element."),
			properties: None,
		},
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
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::wasm_application_io::CreateSurfaceNode")),
								manual_composition: Some(concrete!(Context)),
								skip_deduplication: true,
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(0), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::MemoNode")),
								manual_composition: Some(concrete!(Context)),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::network(generic!(T), 0), NodeInput::network(concrete!(Footprint), 1), NodeInput::node(NodeId(1), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::wasm_application_io::RasterizeNode")),
								manual_composition: Some(concrete!(Context)),
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
						NodeInput::value(TaggedValue::VectorData(VectorDataTable::default()), true),
						NodeInput::value(
							TaggedValue::Footprint(Footprint {
								transform: DAffine2::from_scale_angle_translation(DVec2::new(100., 100.), 0., DVec2::new(0., 0.)),
								resolution: UVec2::new(100, 100),
								..Default::default()
							}),
							false,
						),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_properties: vec!["Artwork".into(), "Footprint".into()],
					output_names: vec!["Canvas".to_string()],
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Create Surface".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Cache".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Rasterize".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
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
			description: Cow::Borrowed("Rasterizes the given vector data"),
			properties: None,
		},
		DocumentNodeDefinition {
			identifier: "Noise Pattern",
			category: "Raster",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					manual_composition: Some(concrete!(Context)),
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::raster::NoisePatternNode")),
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
					input_properties: vec![
						"Spacer".into(),
						"Clip".into(),
						"Seed".into(),
						PropertiesRow::with_override("Scale", WidgetOverride::Custom("noise_properties_scale".to_string())),
						PropertiesRow::with_override("Noise Type", WidgetOverride::Custom("noise_properties_noise_type".to_string())),
						PropertiesRow::with_override("Domain Warp Type", WidgetOverride::Custom("noise_properties_domain_warp_type".to_string())),
						PropertiesRow::with_override("Domain Warp Amplitude", WidgetOverride::Custom("noise_properties_domain_warp_amplitude".to_string())),
						PropertiesRow::with_override("Fractal Type", WidgetOverride::Custom("noise_properties_fractal_type".to_string())),
						PropertiesRow::with_override("Fractal Octaves", WidgetOverride::Custom("noise_properties_fractal_octaves".to_string())),
						PropertiesRow::with_override("Fractal Lacunarity", WidgetOverride::Custom("noise_properties_fractal_lacunarity".to_string())),
						PropertiesRow::with_override("Fractal Gain", WidgetOverride::Custom("noise_properties_fractal_gain".to_string())),
						PropertiesRow::with_override("Fractal Weighted Strength", WidgetOverride::Custom("noise_properties_fractal_weighted_strength".to_string())),
						PropertiesRow::with_override("Fractal Ping Pong Strength", WidgetOverride::Custom("noise_properties_ping_pong_strength".to_string())),
						PropertiesRow::with_override("Cellular Distance Function", WidgetOverride::Custom("noise_properties_cellular_distance_function".to_string())),
						PropertiesRow::with_override("Cellular Return Type", WidgetOverride::Custom("noise_properties_cellular_return_type".to_string())),
						PropertiesRow::with_override("Cellular Jitter", WidgetOverride::Custom("noise_properties_cellular_jitter".to_string())),
					],
					output_names: vec!["Image".to_string()],
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Noise Pattern".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Cull".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
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
			description: Cow::Borrowed("Generates different noise patterns."),
			properties: None,
		},
		// TODO: This needs to work with resolution-aware (raster with footprint, post-Cull node) data.
		// TODO: Auto-generate this from its proto node macro
		DocumentNodeDefinition {
			identifier: "Mask",
			category: "Raster",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_std::raster::MaskImageNode"),
					inputs: vec![
						NodeInput::value(TaggedValue::ImageFrame(ImageFrameTable::one_empty_image()), true),
						NodeInput::value(TaggedValue::ImageFrame(ImageFrameTable::one_empty_image()), true),
					],
					manual_composition: Some(generic!(T)),
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_properties: vec!["Image".into(), PropertiesRow::with_override("Stencil", WidgetOverride::Custom("mask_stencil".to_string()))],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: None,
		},
		// TODO: This needs to work with resolution-aware (raster with footprint, post-Cull node) data.
		// TODO: Auto-generate this from its proto node macro
		DocumentNodeDefinition {
			identifier: "Insert Channel",
			category: "Raster",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_std::raster::InsertChannelNode"),
					inputs: vec![
						NodeInput::value(TaggedValue::ImageFrame(ImageFrameTable::one_empty_image()), true),
						NodeInput::value(TaggedValue::ImageFrame(ImageFrameTable::one_empty_image()), true),
						NodeInput::value(TaggedValue::RedGreenBlue(RedGreenBlue::default()), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_properties: vec!["Image".into(), PropertiesRow::with_override("Insertion", WidgetOverride::Hidden), "Into".into()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: None,
		},
		// TODO: This needs to work with resolution-aware (raster with footprint, post-Cull node) data.
		DocumentNodeDefinition {
			identifier: "Combine Channels",
			category: "Raster",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_std::raster::CombineChannelsNode"),
					inputs: vec![
						NodeInput::value(TaggedValue::None, false),
						NodeInput::value(TaggedValue::ImageFrame(ImageFrameTable::one_empty_image()), true),
						NodeInput::value(TaggedValue::ImageFrame(ImageFrameTable::one_empty_image()), true),
						NodeInput::value(TaggedValue::ImageFrame(ImageFrameTable::one_empty_image()), true),
						NodeInput::value(TaggedValue::ImageFrame(ImageFrameTable::one_empty_image()), true),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_properties: vec!["None".into(), "Red".into(), "Green".into(), "Blue".into(), "Alpha".into()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: None,
		},
		DocumentNodeDefinition {
			identifier: "Split Channels",
			category: "Raster",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![
							NodeInput::node(NodeId(0), 0),
							NodeInput::node(NodeId(1), 0),
							NodeInput::node(NodeId(2), 0),
							NodeInput::node(NodeId(3), 0),
						],
						nodes: [
							DocumentNode {
								inputs: vec![
									NodeInput::network(concrete!(ImageFrameTable<Color>), 0),
									NodeInput::value(TaggedValue::RedGreenBlueAlpha(RedGreenBlueAlpha::Red), false),
								],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::raster::adjustments::ExtractChannelNode")),
								manual_composition: Some(generic!(T)),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![
									NodeInput::network(concrete!(ImageFrameTable<Color>), 0),
									NodeInput::value(TaggedValue::RedGreenBlueAlpha(RedGreenBlueAlpha::Green), false),
								],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::raster::adjustments::ExtractChannelNode")),
								manual_composition: Some(generic!(T)),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![
									NodeInput::network(concrete!(ImageFrameTable<Color>), 0),
									NodeInput::value(TaggedValue::RedGreenBlueAlpha(RedGreenBlueAlpha::Blue), false),
								],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::raster::adjustments::ExtractChannelNode")),
								manual_composition: Some(generic!(T)),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![
									NodeInput::network(concrete!(ImageFrameTable<Color>), 0),
									NodeInput::value(TaggedValue::RedGreenBlueAlpha(RedGreenBlueAlpha::Alpha), false),
								],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::raster::adjustments::ExtractChannelNode")),
								manual_composition: Some(generic!(T)),
								..Default::default()
							},
						]
						.into_iter()
						.enumerate()
						.map(|(id, node)| (NodeId(id as u64), node))
						.collect(),

						..Default::default()
					}),
					inputs: vec![NodeInput::value(TaggedValue::ImageFrame(ImageFrameTable::one_empty_image()), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_properties: vec!["Image".into()],
					output_names: vec!["Red".to_string(), "Green".to_string(), "Blue".to_string(), "Alpha".to_string()],
					has_primary_output: false,
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Extract Channel".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Extract Channel".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 2)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Extract Channel".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 4)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Extract Channel".to_string(),
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
			identifier: "Brush",
			category: "Raster",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(0), 0)],
						nodes: vec![DocumentNode {
							inputs: vec![
								NodeInput::network(concrete!(ImageFrameTable<Color>), 0),
								NodeInput::network(concrete!(ImageFrameTable<Color>), 1),
								NodeInput::network(concrete!(Vec<graphene_core::vector::brush_stroke::BrushStroke>), 2),
								NodeInput::network(concrete!(BrushCache), 3),
							],
							manual_composition: Some(concrete!(Context)),
							implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::brush::BrushNode")),
							..Default::default()
						}]
						.into_iter()
						.enumerate()
						.map(|(id, node)| (NodeId(id as u64), node))
						.collect(),
						..Default::default()
					}),
					inputs: vec![
						NodeInput::value(TaggedValue::ImageFrame(ImageFrameTable::one_empty_image()), true),
						NodeInput::value(TaggedValue::ImageFrame(ImageFrameTable::one_empty_image()), true),
						NodeInput::value(TaggedValue::BrushStrokes(Vec::new()), false),
						NodeInput::value(TaggedValue::BrushCache(BrushCache::new_proto()), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_properties: vec!["Background".into(), "Bounds".into(), "Trace".into(), "Cache".into()],
					output_names: vec!["Image".to_string()],
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Brush".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Cull".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
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
			identifier: "Memoize",
			category: "Debug",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::memo::MemoNode"),
					inputs: vec![NodeInput::value(TaggedValue::ImageFrame(ImageFrameTable::one_empty_image()), true)],
					manual_composition: Some(concrete!(Context)),
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_properties: vec!["Image".into()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: None,
		},
		DocumentNodeDefinition {
			identifier: "Memoize Impure",
			category: "Debug",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::memo::ImpureMemoNode"),
					inputs: vec![NodeInput::value(TaggedValue::ImageFrame(ImageFrameTable::one_empty_image()), true)],
					manual_composition: Some(concrete!(Context)),
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_properties: vec!["Image".into()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: None,
		},
		DocumentNodeDefinition {
			identifier: "Image",
			category: "Raster",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(0), 0)],
						nodes: vec![DocumentNode {
							inputs: vec![NodeInput::network(concrete!(ImageFrameTable<Color>), 1)],
							implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::transform::CullNode")),
							manual_composition: Some(concrete!(Context)),
							..Default::default()
						}]
						.into_iter()
						.enumerate()
						.map(|(id, node)| (NodeId(id as u64), node))
						.collect(),
						..Default::default()
					}),
					inputs: vec![
						NodeInput::value(TaggedValue::None, false),
						NodeInput::value(TaggedValue::ImageFrame(ImageFrameTable::one_empty_image()), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_properties: vec!["Empty".into(), "Image".into()],
					output_names: vec!["Image".to_string()],
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [DocumentNodeMetadata {
								persistent_metadata: DocumentNodePersistentMetadata {
									display_name: "Cull".to_string(),
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
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			identifier: "Uniform",
			category: "Debug: GPU",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(2), 0)],
						nodes: [
							DocumentNode {
								inputs: vec![NodeInput::scope("editor-api")],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::network(generic!(T), 0), NodeInput::node(NodeId(0), 0)],
								manual_composition: Some(concrete!(Context)),
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::UniformNode")),
								..Default::default()
							},
							DocumentNode {
								manual_composition: Some(concrete!(Context)),
								inputs: vec![NodeInput::node(NodeId(1), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::MemoNode")),
								..Default::default()
							},
						]
						.into_iter()
						.enumerate()
						.map(|(id, node)| (NodeId(id as u64), node))
						.collect(),
						..Default::default()
					}),
					inputs: vec![NodeInput::value(TaggedValue::F64(0.), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_properties: vec!["In".into()],
					output_names: vec!["Uniform".to_string()],
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Extract Executor".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Create Uniform".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Cache".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
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
			identifier: "Storage",
			category: "Debug: GPU",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(2), 0)],
						nodes: [
							DocumentNode {
								inputs: vec![NodeInput::scope("editor-api")],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::network(concrete!(Vec<u8>), 0), NodeInput::node(NodeId(0), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::StorageNode")),
								..Default::default()
							},
							DocumentNode {
								manual_composition: Some(concrete!(Context)),
								inputs: vec![NodeInput::node(NodeId(1), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::MemoNode")),
								..Default::default()
							},
						]
						.into_iter()
						.enumerate()
						.map(|(id, node)| (NodeId(id as u64), node))
						.collect(),
						..Default::default()
					}),
					inputs: vec![NodeInput::value(TaggedValue::None, true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_properties: vec!["In".into()],
					output_names: vec!["Storage".to_string()],
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Extract Executor".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Create Storage".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Cache".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
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
			identifier: "Create Output Buffer",
			category: "Debug: GPU",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(2), 0)],
						nodes: [
							DocumentNode {
								inputs: vec![NodeInput::scope("editor-api")],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::network(concrete!(usize), 0), NodeInput::node(NodeId(0), 0), NodeInput::network(concrete!(Type), 1)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::CreateOutputBufferNode")),
								..Default::default()
							},
							DocumentNode {
								manual_composition: Some(concrete!(Context)),
								inputs: vec![NodeInput::node(NodeId(1), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::MemoNode")),
								..Default::default()
							},
						]
						.into_iter()
						.enumerate()
						.map(|(id, node)| (NodeId(id as u64), node))
						.collect(),
						..Default::default()
					}),
					inputs: vec![NodeInput::value(TaggedValue::None, true), NodeInput::value(TaggedValue::None, true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_properties: vec!["In".into(), "In".into()],
					output_names: vec!["Output Buffer".to_string()],
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Extract Executor".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Create Output Buffer".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Cache".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
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
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			identifier: "Create Compute Pass",
			category: "Debug: GPU",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(2), 0)],
						nodes: [
							DocumentNode {
								inputs: vec![NodeInput::scope("editor-api")],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![
									NodeInput::network(concrete!(PipelineLayout), 0),
									NodeInput::node(NodeId(0), 0),
									NodeInput::network(concrete!(WgpuShaderInput), 2),
									NodeInput::network(concrete!(gpu_executor::ComputePassDimensions), 3),
								],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::CreateComputePassNode")),
								..Default::default()
							},
							DocumentNode {
								manual_composition: Some(concrete!(Context)),
								inputs: vec![NodeInput::node(NodeId(1), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::MemoNode")),
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
						NodeInput::network(concrete!(PipelineLayout), 0),
						NodeInput::network(concrete!(WgpuShaderInput), 2),
						NodeInput::network(concrete!(gpu_executor::ComputePassDimensions), 3),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_properties: vec!["In".into(), "In".into(), "In".into()],
					output_names: vec!["Command Buffer".to_string()],
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Extract Executor".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Create Compute Pass".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Cache".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
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
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			identifier: "Create Pipeline Layout",
			category: "Debug: GPU",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("wgpu_executor::CreatePipelineLayoutNode"),
					inputs: vec![
						NodeInput::network(concrete!(ShaderHandle), 0),
						NodeInput::network(concrete!(String), 1),
						NodeInput::network(concrete!(Bindgroup), 2),
						NodeInput::network(concrete!(Arc<WgpuShaderInput>), 3),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_properties: vec!["Shader Handle".into(), "String".into(), "Bindgroup".into(), "Arc Shader Input".into()],
					output_names: vec!["Pipeline Layout".to_string()],
					..Default::default()
				},
			},

			description: Cow::Borrowed("TODO"),
			properties: None,
		},
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			identifier: "Execute Compute Pipeline",
			category: "Debug: GPU",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(2), 0)],
						nodes: [
							DocumentNode {
								inputs: vec![NodeInput::scope("editor-api")],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::network(concrete!(CommandBuffer), 0), NodeInput::node(NodeId(0), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::ExecuteComputePipelineNode")),
								..Default::default()
							},
							DocumentNode {
								manual_composition: Some(concrete!(Context)),
								inputs: vec![NodeInput::node(NodeId(1), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::MemoNode")),
								..Default::default()
							},
						]
						.into_iter()
						.enumerate()
						.map(|(id, node)| (NodeId(id as u64), node))
						.collect(),
						..Default::default()
					}),
					inputs: vec![NodeInput::value(TaggedValue::None, true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_properties: vec!["In".into()],
					output_names: vec!["Pipeline Result".to_string()],
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Extract Executor".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Execute Compute Pipeline".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Cache".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
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
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			identifier: "Read Output Buffer",
			category: "Debug: GPU",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(2), 0)],
						nodes: [
							DocumentNode {
								inputs: vec![NodeInput::scope("editor-api")],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::network(concrete!(Arc<WgpuShaderInput>), 0), NodeInput::node(NodeId(0), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::ReadOutputBufferNode")),
								..Default::default()
							},
							DocumentNode {
								manual_composition: Some(concrete!(Context)),
								inputs: vec![NodeInput::node(NodeId(1), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::MemoNode")),
								..Default::default()
							},
						]
						.into_iter()
						.enumerate()
						.map(|(id, node)| (NodeId(id as u64), node))
						.collect(),
						..Default::default()
					}),
					inputs: vec![NodeInput::value(TaggedValue::None, true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_properties: vec!["In".into()],
					output_names: vec!["Buffer".to_string()],
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Extract Executor".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Read Output Buffer".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Cache".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
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
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			identifier: "Create GPU Surface",
			category: "Debug: GPU",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(1), 0)],
						nodes: [
							DocumentNode {
								manual_composition: Some(concrete!(Context)),
								inputs: vec![NodeInput::scope("editor-api")],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::CreateGpuSurfaceNode")),
								..Default::default()
							},
							DocumentNode {
								manual_composition: Some(concrete!(Context)),
								inputs: vec![NodeInput::node(NodeId(0), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::ImpureMemoNode")),
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
					output_names: vec!["GPU Surface".to_string()],
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Create GPU Surface".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Cache".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
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
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			identifier: "Render Texture",
			category: "Debug: GPU",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(1), 0)],
						nodes: [
							DocumentNode {
								inputs: vec![NodeInput::scope("editor-api")],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode")),
								..Default::default()
							},
							DocumentNode {
								manual_composition: Some(concrete!(Context)),
								inputs: vec![
									NodeInput::network(concrete!(ShaderInputFrame), 0),
									NodeInput::network(concrete!(Arc<wgpu_executor::Surface>), 1),
									NodeInput::node(NodeId(0), 0),
								],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::RenderTextureNode")),
								..Default::default()
							},
						]
						.into_iter()
						.enumerate()
						.map(|(id, node)| (NodeId(id as u64), node))
						.collect(),
						..Default::default()
					}),
					inputs: vec![NodeInput::value(TaggedValue::None, true), NodeInput::value(TaggedValue::None, true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_properties: vec!["Texture".into(), "Surface".into()],
					output_names: vec!["Rendered Texture".to_string()],
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Extract Executor".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Render Texture".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
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
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::network(concrete!(ImageFrameTable<Color>), 0), NodeInput::node(NodeId(0), 0)],
								manual_composition: Some(generic!(T)),
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::UploadTextureNode")),
								..Default::default()
							},
							DocumentNode {
								manual_composition: Some(generic!(T)),
								inputs: vec![NodeInput::node(NodeId(1), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::ImpureMemoNode")),
								..Default::default()
							},
						]
						.into_iter()
						.enumerate()
						.map(|(id, node)| (NodeId(id as u64), node))
						.collect(),
						..Default::default()
					}),
					inputs: vec![NodeInput::value(TaggedValue::ImageFrame(ImageFrameTable::one_empty_image()), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_properties: vec!["In".into()],
					output_names: vec!["Texture".to_string()],
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Extract Executor".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Upload Texture".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Cache".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
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
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			identifier: "GPU Image",
			category: "Debug: GPU",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_std::executor::MapGpuSingleImageNode"),
					inputs: vec![
						NodeInput::value(TaggedValue::ImageFrame(ImageFrameTable::one_empty_image()), true),
						NodeInput::value(TaggedValue::DocumentNode(DocumentNode::default()), true),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_properties: vec!["Image".into(), "Node".into()],
					output_names: vec!["Image".to_string()],
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
					input_properties: vec!["Node".into()],
					output_names: vec!["Document Node".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: None,
		},
		DocumentNodeDefinition {
			// Aims for interoperable compatibility with:
			// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27brit%27%20%3D%20Brightness/Contrast
			// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=Padding-,Brightness%20and%20Contrast,-Key%20is%20%27brit
			identifier: "Brightness/Contrast",
			category: "Raster: Adjustment",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::raster::BrightnessContrastNode"),
					inputs: vec![
						NodeInput::value(TaggedValue::ImageFrame(ImageFrameTable::one_empty_image()), true),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::Bool(false), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_properties: vec![
						"Image".into(),
						PropertiesRow::with_override("Brightness", WidgetOverride::Custom("brightness".to_string())),
						PropertiesRow::with_override("Brightness", WidgetOverride::Custom("contrast".to_string())),
						"Use Classic".into(),
					],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: None,
		},
		// Aims for interoperable compatibility with:
		// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=levl%27%20%3D%20Levels-,%27curv%27%20%3D%20Curves,-%27expA%27%20%3D%20Exposure
		// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=Max%20input%20range-,Curves,-Curves%20settings%20files
		// TODO: Fix this, it's currently broken
		// DocumentNodeDefinition {
		// 	identifier: "Curves",
		// 	category: "Raster: Adjustment",
		// 	node_template: NodeTemplate {
		// 		document_node: DocumentNode {
		// 			implementation: DocumentNodeImplementation::proto("graphene_core::raster::CurvesNode"),
		// 			inputs: vec![
		// 				NodeInput::value(TaggedValue::ImageFrame(ImageFrameTable::empty()), true),
		// 				NodeInput::value(TaggedValue::Curve(Default::default()), false),
		// 			],
		// 			..Default::default()
		// 		},
		// 		persistent_node_metadata: DocumentNodePersistentMetadata {
		// 			input_properties: vec!["Image".into(), "Curve".into()],
		// 			output_names: vec!["Image".to_string()],
		// 			..Default::default()
		// 		},
		// 	},
		// 	description: Cow::Borrowed("TODO"),
		// 	properties: None,
		// },
		// (*IMAGINATE_NODE).clone(),
		DocumentNodeDefinition {
			identifier: "Line",
			category: "Vector: Shape",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::vector::generator_nodes::LineNode"),
					manual_composition: Some(concrete!(Context)),
					inputs: vec![
						NodeInput::value(TaggedValue::None, false),
						NodeInput::value(TaggedValue::DVec2(DVec2::new(0., -50.)), false),
						NodeInput::value(TaggedValue::DVec2(DVec2::new(0., 50.)), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_properties: vec![
						"None".into(),
						PropertiesRow::with_override(
							"Start",
							WidgetOverride::Vec2(Vec2InputSettings {
								x: "X".to_string(),
								y: "Y".to_string(),
								unit: " px".to_string(),
								..Default::default()
							}),
						),
						PropertiesRow::with_override(
							"End",
							WidgetOverride::Vec2(Vec2InputSettings {
								x: "X".to_string(),
								y: "Y".to_string(),
								unit: " px".to_string(),
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
		DocumentNodeDefinition {
			identifier: "Path",
			category: "Vector",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(1), 0)],
						nodes: vec![
							DocumentNode {
								inputs: vec![NodeInput::network(concrete!(VectorDataTable), 0)],
								implementation: DocumentNodeImplementation::proto("graphene_core::memo::MonitorNode"),
								manual_composition: Some(generic!(T)),
								skip_deduplication: true,
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(0), 0), NodeInput::network(concrete!(graphene_core::vector::VectorModification), 1)],
								manual_composition: Some(generic!(T)),
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::vector::vector_data::modification::PathModifyNode")),
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
						NodeInput::value(TaggedValue::VectorData(VectorDataTable::default()), true),
						NodeInput::value(TaggedValue::VectorModification(Default::default()), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_properties: vec!["Vector Data".into(), "Modification".into()],
					output_names: vec!["Vector Data".to_string()],
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Monitor".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Path Modify".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
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
			identifier: "Text",
			category: "Text",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_std::text::TextNode"),
					manual_composition: Some(concrete!(Context)),
					inputs: vec![
						NodeInput::scope("editor-api"),
						NodeInput::value(TaggedValue::String("Lorem ipsum".to_string()), false),
						NodeInput::value(
							TaggedValue::Font(Font::new(graphene_core::consts::DEFAULT_FONT_FAMILY.into(), graphene_core::consts::DEFAULT_FONT_STYLE.into())),
							false,
						),
						NodeInput::value(TaggedValue::F64(TypesettingConfig::default().font_size), false),
						NodeInput::value(TaggedValue::F64(TypesettingConfig::default().line_height_ratio), false),
						NodeInput::value(TaggedValue::F64(TypesettingConfig::default().character_spacing), false),
						NodeInput::value(TaggedValue::OptionalF64(TypesettingConfig::default().max_width), false),
						NodeInput::value(TaggedValue::OptionalF64(TypesettingConfig::default().max_height), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_properties: vec![
						"Editor API".into(),
						PropertiesRow::with_override("Text", WidgetOverride::Custom("text_area".to_string())),
						PropertiesRow::with_override("Font", WidgetOverride::Custom("text_font".to_string())),
						PropertiesRow::with_override(
							"Size",
							WidgetOverride::Number(NumberInputSettings {
								unit: Some(" px".to_string()),
								min: Some(1.),
								..Default::default()
							}),
						),
						PropertiesRow::with_override(
							"Line Height",
							WidgetOverride::Number(NumberInputSettings {
								min: Some(0.),
								step: Some(0.1),
								..Default::default()
							}),
						),
						PropertiesRow::with_override(
							"Character Spacing",
							WidgetOverride::Number(NumberInputSettings {
								min: Some(0.),
								step: Some(0.1),
								..Default::default()
							}),
						),
						PropertiesRow::with_override(
							"Max Width",
							WidgetOverride::Number(NumberInputSettings {
								min: Some(1.),
								blank_assist: false,
								..Default::default()
							}),
						),
						PropertiesRow::with_override(
							"Max Height",
							WidgetOverride::Number(NumberInputSettings {
								min: Some(1.),
								blank_assist: false,
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
		DocumentNodeDefinition {
			identifier: "Transform",
			category: "General",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					inputs: vec![
						NodeInput::value(TaggedValue::VectorData(VectorDataTable::default()), true),
						NodeInput::value(TaggedValue::DVec2(DVec2::ZERO), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::DVec2(DVec2::ONE), false),
						NodeInput::value(TaggedValue::DVec2(DVec2::ZERO), false),
						NodeInput::value(TaggedValue::DVec2(DVec2::splat(0.5)), false),
					],
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(1), 0)],
						nodes: [
							DocumentNode {
								inputs: vec![NodeInput::network(concrete!(VectorDataTable), 0)],
								implementation: DocumentNodeImplementation::proto("graphene_core::memo::MonitorNode"),
								manual_composition: Some(generic!(T)),
								skip_deduplication: true,
								..Default::default()
							},
							DocumentNode {
								inputs: vec![
									NodeInput::node(NodeId(0), 0),
									NodeInput::network(concrete!(DVec2), 1),
									NodeInput::network(concrete!(f64), 2),
									NodeInput::network(concrete!(DVec2), 3),
									NodeInput::network(concrete!(DVec2), 4),
									NodeInput::network(concrete!(DVec2), 5),
								],
								manual_composition: Some(concrete!(Context)),
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::transform::TransformNode")),
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
										display_name: "Monitor".to_string(),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Transform".to_string(),
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
					input_properties: vec![
						"Vector Data".into(),
						PropertiesRow::with_override(
							"Translation",
							WidgetOverride::Vec2(Vec2InputSettings {
								x: "X".to_string(),
								y: "Y".to_string(),
								unit: " px".to_string(),
								..Default::default()
							}),
						),
						PropertiesRow::with_override("Rotation", WidgetOverride::Custom("transform_rotation".to_string())),
						PropertiesRow::with_override(
							"Scale",
							WidgetOverride::Vec2(Vec2InputSettings {
								x: "W".to_string(),
								y: "H".to_string(),
								unit: "x".to_string(),
								..Default::default()
							}),
						),
						PropertiesRow::with_override("Skew", WidgetOverride::Custom("transform_skew".to_string())),
						PropertiesRow::with_override("Pivot", WidgetOverride::Hidden),
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
								inputs: vec![NodeInput::network(concrete!(VectorDataTable), 0), NodeInput::network(concrete!(vector::style::Fill), 1)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::vector::BooleanOperationNode")),
								manual_composition: Some(generic!(T)),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(0), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::ImpureMemoNode")),
								manual_composition: Some(generic!(T)),
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
						NodeInput::value(TaggedValue::GraphicGroup(GraphicGroupTable::default()), true),
						NodeInput::value(TaggedValue::BooleanOperation(vector::misc::BooleanOperation::Union), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Boolean Operation".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(-7, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Cache".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
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
					input_properties: vec!["Group of Paths".into(), "Operation".into()],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},

			description: Cow::Borrowed("TODO"),
			properties: None,
		},
		DocumentNodeDefinition {
			identifier: "Copy to Points",
			category: "Vector",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					// TODO: Wrap this implementation with a document node that has a cache node so the output is cached?
					implementation: DocumentNodeImplementation::proto("graphene_core::vector::CopyToPointsNode"),
					inputs: vec![
						NodeInput::value(TaggedValue::VectorData(graphene_core::vector::VectorDataTable::default()), true),
						NodeInput::value(TaggedValue::VectorData(graphene_core::vector::VectorDataTable::default()), true),
						NodeInput::value(TaggedValue::F64(1.), false),
						NodeInput::value(TaggedValue::F64(1.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::U32(0), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::U32(0), false),
					],
					manual_composition: Some(concrete!(Context)),
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_properties: vec![
						"Points".into(),
						Into::<PropertiesRow>::into("Instance").with_tooltip("Artwork to be copied and placed at each point"),
						PropertiesRow::with_override(
							"Random Scale Min",
							WidgetOverride::Number(NumberInputSettings {
								min: Some(0.),
								mode: NumberInputMode::Range,
								range_min: Some(0.),
								range_max: Some(2.),
								unit: Some("x".to_string()),
								..Default::default()
							}),
						)
						.with_tooltip("Minimum range of randomized sizes given to each instance"),
						PropertiesRow::with_override(
							"Random Scale Max",
							WidgetOverride::Number(NumberInputSettings {
								min: Some(0.),
								mode: NumberInputMode::Range,
								range_min: Some(0.),
								range_max: Some(2.),
								unit: Some("x".to_string()),
								..Default::default()
							}),
						)
						.with_tooltip("Minimum range of randomized sizes given to each instance"),
						PropertiesRow::with_override(
							"Random Scale Bias",
							WidgetOverride::Number(NumberInputSettings {
								mode: NumberInputMode::Range,
								range_min: Some(-50.),
								range_max: Some(50.),
								..Default::default()
							}),
						)
						.with_tooltip("Bias for the probability distribution of randomized sizes (0 is uniform, negatives favor more of small sizes, positives favor more of large sizes)"),
						PropertiesRow::with_override(
							"Random Scale Seed",
							WidgetOverride::Number(NumberInputSettings {
								min: Some(0.),
								is_integer: true,
								..Default::default()
							}),
						)
						.with_tooltip("Seed to determine unique variations on all the randomized instance sizes"),
						PropertiesRow::with_override(
							"Random Rotation",
							WidgetOverride::Number(NumberInputSettings {
								min: Some(0.),
								max: Some(360.),
								mode: NumberInputMode::Range,
								unit: Some("°".to_string()),
								..Default::default()
							}),
						)
						.with_tooltip("Range of randomized angles given to each instance, in degrees ranging from furthest clockwise to counterclockwise"),
						PropertiesRow::with_override(
							"Random Rotation Seed",
							WidgetOverride::Number(NumberInputSettings {
								min: Some(0.),
								is_integer: true,
								..Default::default()
							}),
						)
						.with_tooltip("Seed to determine unique variations on all the randomized instance angles"),
					],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},

			description: Cow::Borrowed("TODO"),
			properties: None,
		},
		DocumentNodeDefinition {
			identifier: "Sample Points",
			category: "Vector",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(2), 0)], // Taken from output 0 of Sample Points
						nodes: [
							DocumentNode {
								inputs: vec![NodeInput::network(concrete!(graphene_core::vector::VectorDataTable), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::vector::SubpathSegmentLengthsNode")),
								manual_composition: Some(generic!(T)),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![
									NodeInput::network(concrete!(graphene_core::vector::VectorDataTable), 0),
									NodeInput::network(concrete!(f64), 1),  // From the document node's parameters
									NodeInput::network(concrete!(f64), 2),  // From the document node's parameters
									NodeInput::network(concrete!(f64), 3),  // From the document node's parameters
									NodeInput::network(concrete!(bool), 4), // From the document node's parameters
									NodeInput::node(NodeId(0), 0),          // From output 0 of SubpathSegmentLengthsNode
								],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::vector::SamplePointsNode")),
								manual_composition: Some(generic!(T)),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(1), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::ImpureMemoNode")),
								manual_composition: Some(generic!(T)),
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
						NodeInput::value(TaggedValue::VectorData(graphene_core::vector::VectorDataTable::default()), true),
						NodeInput::value(TaggedValue::F64(100.), false),
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
										display_name: "Subpath Segment Lengths".to_string(),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Sample Points".to_string(),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Memoize Impure".to_string(),
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
					input_properties: vec![
						"Vector Data".into(),
						PropertiesRow::with_override(
							"Spacing",
							WidgetOverride::Number(NumberInputSettings {
								min: Some(1.),
								unit: Some(" px".to_string()),
								..Default::default()
							}),
						)
						.with_tooltip("Distance between each instance (exact if 'Adaptive Spacing' is disabled, approximate if enabled)"),
						PropertiesRow::with_override(
							"Start Offset",
							WidgetOverride::Number(NumberInputSettings {
								min: Some(0.),
								unit: Some(" px".to_string()),
								..Default::default()
							}),
						)
						.with_tooltip("Exclude some distance from the start of the path before the first instance"),
						PropertiesRow::with_override(
							"Stop Offset",
							WidgetOverride::Number(NumberInputSettings {
								min: Some(0.),
								unit: Some(" px".to_string()),
								..Default::default()
							}),
						)
						.with_tooltip("Exclude some distance from the end of the path after the last instance"),
						Into::<PropertiesRow>::into("Adaptive Spacing").with_tooltip("Round 'Spacing' to a nearby value that divides into the path length evenly"),
					],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},

			description: Cow::Borrowed("TODO"),
			properties: None,
		},
		DocumentNodeDefinition {
			identifier: "Scatter Points",
			category: "Vector",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(1), 0)],
						nodes: [
							DocumentNode {
								inputs: vec![
									NodeInput::network(concrete!(graphene_core::vector::VectorDataTable), 0),
									NodeInput::network(concrete!(f64), 1),
									NodeInput::network(concrete!(u32), 2),
								],
								manual_composition: Some(generic!(T)),
								implementation: DocumentNodeImplementation::proto("graphene_core::vector::PoissonDiskPointsNode"),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(0), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::ImpureMemoNode")),
								manual_composition: Some(generic!(T)),
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
						NodeInput::value(TaggedValue::VectorData(graphene_core::vector::VectorDataTable::default()), true),
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
										display_name: "Poisson-Disk Points".to_string(),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Memoize Impure".to_string(),
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
					input_properties: vec![
						"Vector Data".into(),
						PropertiesRow::with_override(
							"Separation Disk Diameter",
							WidgetOverride::Number(NumberInputSettings {
								min: Some(0.01),
								mode: NumberInputMode::Range,
								range_min: Some(1.),
								range_max: Some(100.),
								..Default::default()
							}),
						),
						PropertiesRow::with_override(
							"Seed",
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

	// Remove struct generics
	for DocumentNodeDefinition { node_template, .. } in custom.iter_mut() {
		let NodeTemplate {
			document_node: DocumentNode { implementation, .. },
			..
		} = node_template;
		if let DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier { name }) = implementation {
			if let Some((new_name, _suffix)) = name.rsplit_once("<") {
				*name = Cow::Owned(new_name.to_string())
			}
		};
	}
	let node_registry = graphene_core::registry::NODE_REGISTRY.lock().unwrap();
	'outer: for (id, metadata) in graphene_core::registry::NODE_METADATA.lock().unwrap().iter() {
		use graphene_core::registry::*;
		let id = id.clone();

		for node in custom.iter() {
			let DocumentNodeDefinition {
				node_template: NodeTemplate {
					document_node: DocumentNode { implementation, .. },
					..
				},
				..
			} = node;
			match implementation {
				DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier { name }) if name == &id => continue 'outer,
				_ => (),
			}
		}

		let NodeMetadata {
			display_name,
			category,
			fields,
			description,
			properties,
		} = metadata;
		let Some(implementations) = &node_registry.get(&id) else { continue };
		let valid_inputs: HashSet<_> = implementations.iter().map(|(_, node_io)| node_io.call_argument.clone()).collect();
		let first_node_io = implementations.first().map(|(_, node_io)| node_io).unwrap_or(const { &NodeIOTypes::empty() });
		let mut input_type = &first_node_io.call_argument;
		if valid_inputs.len() > 1 {
			input_type = &const { generic!(D) };
		}
		let output_type = &first_node_io.return_value;

		let inputs = fields
			.iter()
			.zip(first_node_io.inputs.iter())
			.enumerate()
			.map(|(index, (field, node_io_ty))| {
				let ty = field.default_type.as_ref().unwrap_or(node_io_ty);
				let exposed = if index == 0 { *ty != fn_type_fut!(Context, ()) } else { field.exposed };

				match field.value_source {
					RegistryValueSource::None => {}
					RegistryValueSource::Default(data) => return NodeInput::value(TaggedValue::from_primitive_string(data, ty).unwrap_or(TaggedValue::None), exposed),
					RegistryValueSource::Scope(data) => return NodeInput::scope(Cow::Borrowed(data)),
				};

				if let Some(type_default) = TaggedValue::from_type(ty) {
					return NodeInput::value(type_default, exposed);
				}
				NodeInput::value(TaggedValue::None, true)
			})
			.collect();

		let node = DocumentNodeDefinition {
			identifier: display_name,
			node_template: NodeTemplate {
				document_node: DocumentNode {
					inputs,
					manual_composition: Some(input_type.clone()),
					implementation: DocumentNodeImplementation::ProtoNode(id.clone().into()),
					visible: true,
					skip_deduplication: false,
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					// TODO: Store information for input overrides in the node macro
					input_properties: fields
						.iter()
						.map(|f| match f.widget_override {
							RegistryWidgetOverride::None => f.name.into(),
							RegistryWidgetOverride::Hidden => PropertiesRow::with_override(f.name, WidgetOverride::Hidden),
							RegistryWidgetOverride::String(str) => PropertiesRow::with_override(f.name, WidgetOverride::String(str.to_string())),
							RegistryWidgetOverride::Custom(str) => PropertiesRow::with_override(f.name, WidgetOverride::Custom(str.to_string())),
						})
						.collect(),
					output_names: vec![output_type.to_string()],
					has_primary_output: true,
					locked: false,

					..Default::default()
				},
			},
			category: category.unwrap_or("UNCATEGORIZED"),
			description: Cow::Borrowed(description),
			properties: *properties,
		};
		custom.push(node);
	}
	custom
}

pub static IMAGINATE_NODE: Lazy<DocumentNodeDefinition> = Lazy::new(|| DocumentNodeDefinition {
	identifier: "Imaginate",
	category: "Raster",
	node_template: NodeTemplate {
		document_node: DocumentNode {
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![NodeInput::node(NodeId(1), 0)],
				nodes: [
					DocumentNode {
						inputs: vec![NodeInput::network(concrete!(ImageFrameTable<Color>), 0)],
						implementation: DocumentNodeImplementation::proto("graphene_core::memo::MonitorNode"),
						manual_composition: Some(concrete!(Context)),
						skip_deduplication: true,
						..Default::default()
					},
					DocumentNode {
						inputs: vec![
							NodeInput::node(NodeId(0), 0),
							NodeInput::network(concrete!(&WasmEditorApi), 1),
							NodeInput::network(concrete!(ImaginateController), 2),
							NodeInput::network(concrete!(f64), 3),
							NodeInput::network(concrete!(Option<DVec2>), 4),
							NodeInput::network(concrete!(u32), 5),
							NodeInput::network(concrete!(ImaginateSamplingMethod), 6),
							NodeInput::network(concrete!(f64), 7),
							NodeInput::network(concrete!(String), 8),
							NodeInput::network(concrete!(String), 9),
							NodeInput::network(concrete!(bool), 10),
							NodeInput::network(concrete!(f64), 11),
							NodeInput::network(concrete!(bool), 12),
							NodeInput::network(concrete!(f64), 13),
							NodeInput::network(concrete!(ImaginateMaskStartingFill), 14),
							NodeInput::network(concrete!(bool), 15),
							NodeInput::network(concrete!(bool), 16),
							NodeInput::network(concrete!(u64), 17),
						],
						implementation: DocumentNodeImplementation::proto("graphene_std::raster::ImaginateNode"),
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
				NodeInput::value(TaggedValue::ImageFrame(ImageFrameTable::one_empty_image()), true),
				NodeInput::scope("editor-api"),
				NodeInput::value(TaggedValue::ImaginateController(Default::default()), false),
				NodeInput::value(TaggedValue::F64(0.), false), // Remember to keep index used in `ImaginateRandom` updated with this entry's index
				NodeInput::value(TaggedValue::OptionalDVec2(None), false),
				NodeInput::value(TaggedValue::U32(30), false),
				NodeInput::value(TaggedValue::ImaginateSamplingMethod(ImaginateSamplingMethod::EulerA), false),
				NodeInput::value(TaggedValue::F64(7.5), false),
				NodeInput::value(TaggedValue::String(String::new()), false),
				NodeInput::value(TaggedValue::String(String::new()), false),
				NodeInput::value(TaggedValue::Bool(false), false),
				NodeInput::value(TaggedValue::F64(66.), false),
				NodeInput::value(TaggedValue::Bool(true), false),
				NodeInput::value(TaggedValue::F64(4.), false),
				NodeInput::value(TaggedValue::ImaginateMaskStartingFill(ImaginateMaskStartingFill::Fill), false),
				NodeInput::value(TaggedValue::Bool(false), false),
				NodeInput::value(TaggedValue::Bool(false), false),
				NodeInput::value(TaggedValue::U64(0), false),
			],
			..Default::default()
		},
		persistent_node_metadata: DocumentNodePersistentMetadata {
			network_metadata: Some(NodeNetworkMetadata {
				persistent_metadata: NodeNetworkPersistentMetadata {
					node_metadata: [
						DocumentNodeMetadata {
							persistent_metadata: DocumentNodePersistentMetadata {
								display_name: "Monitor".to_string(),
								..Default::default()
							},
							..Default::default()
						},
						DocumentNodeMetadata {
							persistent_metadata: DocumentNodePersistentMetadata {
								display_name: "Imaginate".to_string(),
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
			input_properties: vec![
				"Input Image".into(),
				"Editor Api".into(),
				"Controller".into(),
				"Seed".into(),
				"Resolution".into(),
				"Samples".into(),
				"Sampling Method".into(),
				"Prompt Guidance".into(),
				"Prompt".into(),
				"Negative Prompt".into(),
				"Adapt Input Image".into(),
				"Image Creativity".into(),
				"Inpaint".into(),
				"Mask Blur".into(),
				"Mask Starting Fill".into(),
				"Improve Faces".into(),
				"Tiling".into(),
			],
			output_names: vec!["Image".to_string()],
			..Default::default()
		},
	},
	description: Cow::Borrowed("TODO"),
	properties: None, // Some(&node_properties::imaginate_properties),
});

type NodeProperties = HashMap<String, Box<dyn Fn(NodeId, &mut NodePropertiesContext) -> Vec<LayoutGroup> + Send + Sync>>;

pub static NODE_OVERRIDES: once_cell::sync::Lazy<NodeProperties> = once_cell::sync::Lazy::new(static_node_properties);

/// Defines the logic for inputs to display a custom properties panel widget.
fn static_node_properties() -> NodeProperties {
	let mut map: NodeProperties = HashMap::new();
	map.insert("channel_mixer_properties".to_string(), Box::new(node_properties::channel_mixer_properties));
	map.insert("fill_properties".to_string(), Box::new(node_properties::fill_properties));
	map.insert("stroke_properties".to_string(), Box::new(node_properties::stroke_properties));
	map.insert("offset_path_properties".to_string(), Box::new(node_properties::offset_path_properties));
	map.insert("selective_color_properties".to_string(), Box::new(node_properties::selective_color_properties));
	map.insert("exposure_properties".to_string(), Box::new(node_properties::exposure_properties));
	map.insert("math_properties".to_string(), Box::new(node_properties::math_properties));
	map.insert("rectangle_properties".to_string(), Box::new(node_properties::rectangle_properties));
	map.insert(
		"identity_properties".to_string(),
		Box::new(|_node_id, _context| node_properties::string_properties("The identity node simply passes its data through.")),
	);
	map.insert(
		"monitor_properties".to_string(),
		Box::new(|_node_id, _context| node_properties::string_properties("The Monitor node is used by the editor to access the data flowing through it.")),
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
			let Some(value) = context.network_interface.input_metadata(&node_id, index, "string_properties", context.selection_network_path) else {
				return Err(format!("Could not get string properties for node {}", node_id));
			};
			let Some(string) = value.as_str() else {
				return Err(format!("Could not downcast string properties for node {}", node_id));
			};
			Ok(node_properties::string_properties(string))
		}),
	);
	map.insert(
		"number".to_string(),
		Box::new(|node_id, index, context| {
			let (document_node, input_name) = node_properties::query_node_and_input_name(node_id, index, context)?;
			let mut number_input = NumberInput::default();
			if let Some(unit) = context
				.network_interface
				.input_metadata(&node_id, index, "unit", context.selection_network_path)
				.and_then(|value| value.as_str())
			{
				number_input = number_input.unit(unit);
			}
			if let Some(min) = context
				.network_interface
				.input_metadata(&node_id, index, "min", context.selection_network_path)
				.and_then(|value| value.as_f64())
			{
				number_input = number_input.min(min);
			}
			if let Some(max) = context
				.network_interface
				.input_metadata(&node_id, index, "max", context.selection_network_path)
				.and_then(|value| value.as_f64())
			{
				number_input = number_input.max(max);
			}
			if let Some(step) = context
				.network_interface
				.input_metadata(&node_id, index, "step", context.selection_network_path)
				.and_then(|value| value.as_f64())
			{
				number_input = number_input.step(step);
			}
			if let Some(mode) = context.network_interface.input_metadata(&node_id, index, "mode", context.selection_network_path).map(|value| {
				let mode: NumberInputMode = serde_json::from_value(value.clone()).unwrap();
				mode
			}) {
				number_input = number_input.mode(mode);
			}
			if let Some(range_min) = context
				.network_interface
				.input_metadata(&node_id, index, "range_min", context.selection_network_path)
				.and_then(|value| value.as_f64())
			{
				number_input = number_input.range_min(Some(range_min));
			}
			if let Some(range_max) = context
				.network_interface
				.input_metadata(&node_id, index, "range_max", context.selection_network_path)
				.and_then(|value| value.as_f64())
			{
				number_input = number_input.range_max(Some(range_max));
			}
			if let Some(is_integer) = context
				.network_interface
				.input_metadata(&node_id, index, "is_integer", context.selection_network_path)
				.and_then(|value| value.as_bool())
			{
				number_input = number_input.is_integer(is_integer);
			}
			let blank_assist = context
				.network_interface
				.input_metadata(&node_id, index, "blank_assist", context.selection_network_path)
				.and_then(|value| value.as_bool())
				.unwrap_or_else(|| {
					log::error!("Could not get blank assist when displaying number input for node {node_id}, index {index}");
					true
				});
			Ok(vec![LayoutGroup::Row {
				widgets: node_properties::number_widget(document_node, node_id, index, input_name, number_input, blank_assist),
			}])
		}),
	);
	map.insert(
		"vec2".to_string(),
		Box::new(|node_id, index, context| {
			let (document_node, input_name) = node_properties::query_node_and_input_name(node_id, index, context)?;
			let x = context
				.network_interface
				.input_metadata(&node_id, index, "x", context.selection_network_path)
				.and_then(|value| value.as_str())
				.unwrap_or_else(|| {
					log::error!("Could not get x for vec2 input");
					""
				});
			let y = context
				.network_interface
				.input_metadata(&node_id, index, "y", context.selection_network_path)
				.and_then(|value| value.as_str())
				.unwrap_or_else(|| {
					log::error!("Could not get y for vec2 input");
					""
				});
			let unit = context
				.network_interface
				.input_metadata(&node_id, index, "unit", context.selection_network_path)
				.and_then(|value| value.as_str())
				.unwrap_or_else(|| {
					log::error!("Could not get unit for vec2 input");
					""
				});
			let min = context
				.network_interface
				.input_metadata(&node_id, index, "min", context.selection_network_path)
				.and_then(|value| value.as_f64());

			Ok(vec![node_properties::vec2_widget(
				document_node,
				node_id,
				index,
				input_name,
				x,
				y,
				unit,
				min,
				node_properties::add_blank_assist,
			)])
		}),
	);
	map.insert(
		"noise_properties_scale".to_string(),
		Box::new(|node_id, index, context| {
			let (document_node, input_name) = node_properties::query_node_and_input_name(node_id, index, context)?;
			let (_, coherent_noise_active, _, _, _, _) = node_properties::query_noise_pattern_state(node_id, context)?;
			let scale = node_properties::number_widget(document_node, node_id, index, input_name, NumberInput::default().min(0.).disabled(!coherent_noise_active), true);
			Ok(vec![scale.into()])
		}),
	);
	map.insert(
		"noise_properties_noise_type".to_string(),
		Box::new(|node_id, index, context| {
			let (document_node, input_name) = node_properties::query_node_and_input_name(node_id, index, context)?;
			let noise_type_row = node_properties::noise_type(document_node, node_id, index, input_name, true);
			Ok(vec![noise_type_row, LayoutGroup::Row { widgets: Vec::new() }])
		}),
	);
	map.insert(
		"noise_properties_domain_warp_type".to_string(),
		Box::new(|node_id, index, context| {
			let (document_node, input_name) = node_properties::query_node_and_input_name(node_id, index, context)?;
			let (_, coherent_noise_active, _, _, _, _) = node_properties::query_noise_pattern_state(node_id, context)?;
			let domain_warp_type = node_properties::domain_warp_type(document_node, node_id, index, input_name, true, !coherent_noise_active);
			Ok(vec![domain_warp_type])
		}),
	);
	map.insert(
		"noise_properties_domain_warp_amplitude".to_string(),
		Box::new(|node_id, index, context| {
			let (document_node, input_name) = node_properties::query_node_and_input_name(node_id, index, context)?;
			let (_, coherent_noise_active, _, _, domain_warp_active, _) = node_properties::query_noise_pattern_state(node_id, context)?;
			let domain_warp_amplitude = node_properties::number_widget(
				document_node,
				node_id,
				index,
				input_name,
				NumberInput::default().min(0.).disabled(!coherent_noise_active || !domain_warp_active),
				true,
			);
			Ok(vec![domain_warp_amplitude.into(), LayoutGroup::Row { widgets: Vec::new() }])
		}),
	);
	map.insert(
		"noise_properties_fractal_type".to_string(),
		Box::new(|node_id, index, context| {
			let (document_node, input_name) = node_properties::query_node_and_input_name(node_id, index, context)?;
			let (_, coherent_noise_active, _, _, _, _) = node_properties::query_noise_pattern_state(node_id, context)?;
			let fractal_type_row = node_properties::fractal_type(document_node, node_id, index, input_name, true, !coherent_noise_active);
			Ok(vec![fractal_type_row])
		}),
	);
	map.insert(
		"noise_properties_fractal_octaves".to_string(),
		Box::new(|node_id, index, context| {
			let (document_node, input_name) = node_properties::query_node_and_input_name(node_id, index, context)?;
			let (fractal_active, coherent_noise_active, _, _, _, domain_warp_only_fractal_type_wrongly_active) = node_properties::query_noise_pattern_state(node_id, context)?;
			let fractal_octaves = node_properties::number_widget(
				document_node,
				node_id,
				index,
				input_name,
				NumberInput::default()
					.mode_range()
					.min(1.)
					.max(10.)
					.range_max(Some(4.))
					.is_integer(true)
					.disabled(!coherent_noise_active || !fractal_active || domain_warp_only_fractal_type_wrongly_active),
				true,
			);
			Ok(vec![fractal_octaves.into()])
		}),
	);
	map.insert(
		"noise_properties_fractal_lacunarity".to_string(),
		Box::new(|node_id, index, context| {
			let (document_node, input_name) = node_properties::query_node_and_input_name(node_id, index, context)?;
			let (fractal_active, coherent_noise_active, _, _, _, domain_warp_only_fractal_type_wrongly_active) = node_properties::query_noise_pattern_state(node_id, context)?;
			let fractal_lacunarity = node_properties::number_widget(
				document_node,
				node_id,
				index,
				input_name,
				NumberInput::default()
					.mode_range()
					.min(0.)
					.range_max(Some(10.))
					.disabled(!coherent_noise_active || !fractal_active || domain_warp_only_fractal_type_wrongly_active),
				true,
			);
			Ok(vec![fractal_lacunarity.into()])
		}),
	);
	map.insert(
		"noise_properties_fractal_gain".to_string(),
		Box::new(|node_id, index, context| {
			let (document_node, input_name) = node_properties::query_node_and_input_name(node_id, index, context)?;
			let (fractal_active, coherent_noise_active, _, _, _, domain_warp_only_fractal_type_wrongly_active) = node_properties::query_noise_pattern_state(node_id, context)?;
			let fractal_gain = node_properties::number_widget(
				document_node,
				node_id,
				index,
				input_name,
				NumberInput::default()
					.mode_range()
					.min(0.)
					.range_max(Some(10.))
					.disabled(!coherent_noise_active || !fractal_active || domain_warp_only_fractal_type_wrongly_active),
				true,
			);
			Ok(vec![fractal_gain.into()])
		}),
	);
	map.insert(
		"noise_properties_fractal_weighted_strength".to_string(),
		Box::new(|node_id, index, context| {
			let (document_node, input_name) = node_properties::query_node_and_input_name(node_id, index, context)?;
			let (fractal_active, coherent_noise_active, _, _, _, domain_warp_only_fractal_type_wrongly_active) = node_properties::query_noise_pattern_state(node_id, context)?;
			let fractal_weighted_strength = node_properties::number_widget(
				document_node,
				node_id,
				index,
				input_name,
				NumberInput::default()
					.mode_range()
					.min(0.)
					.max(1.) // Defined for the 0-1 range
					.disabled(!coherent_noise_active || !fractal_active || domain_warp_only_fractal_type_wrongly_active),
				true,
			);
			Ok(vec![fractal_weighted_strength.into()])
		}),
	);
	map.insert(
		"noise_properties_ping_pong_strength".to_string(),
		Box::new(|node_id, index, context| {
			let (document_node, input_name) = node_properties::query_node_and_input_name(node_id, index, context)?;
			let (fractal_active, coherent_noise_active, _, ping_pong_active, _, domain_warp_only_fractal_type_wrongly_active) = node_properties::query_noise_pattern_state(node_id, context)?;
			let fractal_ping_pong_strength = node_properties::number_widget(
				document_node,
				node_id,
				index,
				input_name,
				NumberInput::default()
					.mode_range()
					.min(0.)
					.range_max(Some(10.))
					.disabled(!ping_pong_active || !coherent_noise_active || !fractal_active || domain_warp_only_fractal_type_wrongly_active),
				true,
			);
			Ok(vec![fractal_ping_pong_strength.into(), LayoutGroup::Row { widgets: Vec::new() }])
		}),
	);
	map.insert(
		"noise_properties_cellular_distance_function".to_string(),
		Box::new(|node_id, index, context| {
			let (document_node, input_name) = node_properties::query_node_and_input_name(node_id, index, context)?;
			let (_, coherent_noise_active, cellular_noise_active, _, _, _) = node_properties::query_noise_pattern_state(node_id, context)?;
			let cellular_distance_function_row = node_properties::cellular_distance_function(document_node, node_id, index, input_name, true, !coherent_noise_active || !cellular_noise_active);
			Ok(vec![cellular_distance_function_row])
		}),
	);
	map.insert(
		"noise_properties_cellular_return_type".to_string(),
		Box::new(|node_id, index, context| {
			let (document_node, input_name) = node_properties::query_node_and_input_name(node_id, index, context)?;
			let (_, coherent_noise_active, cellular_noise_active, _, _, _) = node_properties::query_noise_pattern_state(node_id, context)?;
			let cellular_return_type = node_properties::cellular_return_type(document_node, node_id, index, input_name, true, !coherent_noise_active || !cellular_noise_active);
			Ok(vec![cellular_return_type])
		}),
	);
	map.insert(
		"noise_properties_cellular_jitter".to_string(),
		Box::new(|node_id, index, context| {
			let (document_node, input_name) = node_properties::query_node_and_input_name(node_id, index, context)?;
			let (_, coherent_noise_active, cellular_noise_active, _, _, _) = node_properties::query_noise_pattern_state(node_id, context)?;
			let cellular_jitter = node_properties::number_widget(
				document_node,
				node_id,
				index,
				input_name,
				NumberInput::default()
					.mode_range()
					.range_min(Some(0.))
					.range_max(Some(1.))
					.disabled(!coherent_noise_active || !cellular_noise_active),
				true,
			);
			Ok(vec![cellular_jitter.into()])
		}),
	);
	map.insert(
		"brightness".to_string(),
		Box::new(|node_id, index, context| {
			let (document_node, input_name) = node_properties::query_node_and_input_name(node_id, index, context)?;
			let is_use_classic = document_node
				.inputs
				.iter()
				.find_map(|input| match input.as_value() {
					Some(&TaggedValue::Bool(use_classic)) => Some(use_classic),
					_ => None,
				})
				.unwrap_or(false);
			let (b_min, b_max) = if is_use_classic { (-100., 100.) } else { (-100., 150.) };
			let brightness = node_properties::number_widget(
				document_node,
				node_id,
				index,
				input_name,
				NumberInput::default().mode_range().range_min(Some(b_min)).range_max(Some(b_max)).unit("%").display_decimal_places(2),
				true,
			);
			Ok(vec![brightness.into()])
		}),
	);
	map.insert(
		"contrast".to_string(),
		Box::new(|node_id, index, context| {
			let (document_node, input_name) = node_properties::query_node_and_input_name(node_id, index, context)?;
			let is_use_classic = document_node
				.inputs
				.iter()
				.find_map(|input| match input.as_value() {
					Some(&TaggedValue::Bool(use_classic)) => Some(use_classic),
					_ => None,
				})
				.unwrap_or(false);
			let (c_min, c_max) = if is_use_classic { (-100., 100.) } else { (-50., 100.) };
			let contrast = node_properties::number_widget(
				document_node,
				node_id,
				index,
				input_name,
				NumberInput::default().mode_range().range_min(Some(c_min)).range_max(Some(c_max)).unit("%").display_decimal_places(2),
				true,
			);
			Ok(vec![contrast.into()])
		}),
	);
	map.insert(
		"assign_colors_gradient".to_string(),
		Box::new(|node_id, index, context| {
			let (document_node, input_name) = node_properties::query_node_and_input_name(node_id, index, context)?;
			let gradient_row = node_properties::color_widget(document_node, node_id, index, input_name, ColorInput::default().allow_none(false), true);
			Ok(vec![gradient_row])
		}),
	);
	map.insert(
		"assign_colors_seed".to_string(),
		Box::new(|node_id, index, context| {
			let (document_node, input_name) = node_properties::query_node_and_input_name(node_id, index, context)?;
			let randomize_enabled = node_properties::query_assign_colors_randomize(node_id, context)?;
			let seed_row = node_properties::number_widget(document_node, node_id, index, input_name, NumberInput::default().min(0.).int().disabled(!randomize_enabled), true);
			Ok(vec![seed_row.into()])
		}),
	);
	map.insert(
		"assign_colors_repeat_every".to_string(),
		Box::new(|node_id, index, context| {
			let (document_node, input_name) = node_properties::query_node_and_input_name(node_id, index, context)?;
			let randomize_enabled = node_properties::query_assign_colors_randomize(node_id, context)?;
			let repeat_every_row = node_properties::number_widget(document_node, node_id, index, input_name, NumberInput::default().min(0.).int().disabled(randomize_enabled), true);
			Ok(vec![repeat_every_row.into()])
		}),
	);
	map.insert(
		"mask_stencil".to_string(),
		Box::new(|node_id, index, context| {
			let (document_node, input_name) = node_properties::query_node_and_input_name(node_id, index, context)?;
			let mask = node_properties::color_widget(document_node, node_id, index, input_name, ColorInput::default(), true);
			Ok(vec![mask])
		}),
	);
	map.insert(
		"spline_input".to_string(),
		Box::new(|node_id, index, context| {
			let (document_node, input_name) = node_properties::query_node_and_input_name(node_id, index, context)?;
			Ok(vec![LayoutGroup::Row {
				widgets: node_properties::vec_dvec2_input(document_node, node_id, index, input_name, TextInput::default().centered(true), true),
			}])
		}),
	);
	map.insert(
		"transform_rotation".to_string(),
		Box::new(|node_id, index, context| {
			let (document_node, input_name) = node_properties::query_node_and_input_name(node_id, index, context)?;

			let mut widgets = node_properties::start_widgets(document_node, node_id, index, input_name, super::utility_types::FrontendGraphDataType::Number, true);

			let Some(input) = document_node.inputs.get(index) else {
				return Err("Input not found in transform rotation input override".to_string());
			};
			if let Some(&TaggedValue::F64(val)) = input.as_non_exposed_value() {
				widgets.extend_from_slice(&[
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					NumberInput::new(Some(val.to_degrees()))
						.unit("°")
						.mode(NumberInputMode::Range)
						.range_min(Some(-180.))
						.range_max(Some(180.))
						.on_update(node_properties::update_value(
							|number_input: &NumberInput| TaggedValue::F64(number_input.value.unwrap().to_radians()),
							node_id,
							index,
						))
						.on_commit(node_properties::commit_value)
						.widget_holder(),
				]);
			}

			Ok(vec![LayoutGroup::Row { widgets }])
		}),
	);
	// Skew has a custom override that maps to degrees
	map.insert(
		"transform_skew".to_string(),
		Box::new(|node_id, index, context| {
			let (document_node, input_name) = node_properties::query_node_and_input_name(node_id, index, context)?;

			let mut widgets = node_properties::start_widgets(document_node, node_id, index, input_name, super::utility_types::FrontendGraphDataType::Number, true);

			let Some(input) = document_node.inputs.get(index) else {
				return Err("Input not found in transform skew input override".to_string());
			};
			if let Some(&TaggedValue::DVec2(val)) = input.as_non_exposed_value() {
				let to_skew = |input: &NumberInput| input.value.unwrap().to_radians().tan();
				widgets.extend_from_slice(&[
					Separator::new(SeparatorType::Unrelated).widget_holder(),
					NumberInput::new(Some(val.x.atan().to_degrees()))
						.label("X")
						.unit("°")
						.min(-89.9)
						.max(89.9)
						.on_update(node_properties::update_value(
							move |input: &NumberInput| TaggedValue::DVec2(DVec2::new(to_skew(input), val.y)),
							node_id,
							index,
						))
						.on_commit(node_properties::commit_value)
						.widget_holder(),
					Separator::new(SeparatorType::Related).widget_holder(),
					NumberInput::new(Some(val.y.atan().to_degrees()))
						.label("Y")
						.unit("°")
						.min(-89.9)
						.max(89.9)
						.on_update(node_properties::update_value(
							move |input: &NumberInput| TaggedValue::DVec2(DVec2::new(val.x, to_skew(input))),
							node_id,
							index,
						))
						.on_commit(node_properties::commit_value)
						.widget_holder(),
				]);
			}

			Ok(vec![LayoutGroup::Row { widgets }])
		}),
	);
	map.insert(
		"text_area".to_string(),
		Box::new(|node_id, index, context| {
			let (document_node, input_name) = node_properties::query_node_and_input_name(node_id, index, context)?;
			Ok(vec![LayoutGroup::Row {
				widgets: node_properties::text_area_widget(document_node, node_id, index, input_name, true),
			}])
		}),
	);
	map.insert(
		"text_font".to_string(),
		Box::new(|node_id, index, context| {
			let (document_node, input_name) = node_properties::query_node_and_input_name(node_id, index, context)?;
			let (font, style) = node_properties::font_inputs(document_node, node_id, index, input_name, true);
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
			let (document_node, input_name) = node_properties::query_node_and_input_name(node_id, index, context)?;
			Ok(vec![node_properties::color_widget(
				document_node,
				node_id,
				index,
				input_name,
				ColorInput::default().allow_none(false),
				true,
			)])
		}),
	);
	map
}

pub fn resolve_document_node_type(identifier: &str) -> Option<&DocumentNodeDefinition> {
	DOCUMENT_NODE_TYPES.iter().find(|definition| definition.identifier == identifier)
}

pub fn collect_node_types() -> Vec<FrontendNodeType> {
	// Create a mapping from registry ID to document node identifier
	let id_to_identifier_map: HashMap<String, &'static str> = DOCUMENT_NODE_TYPES
		.iter()
		.filter_map(|definition| {
			if let DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier { name }) = &definition.node_template.document_node.implementation {
				Some((name.to_string(), definition.identifier))
			} else {
				None
			}
		})
		.collect();
	let mut extracted_node_types = Vec::new();

	let node_registry = graphene_core::registry::NODE_REGISTRY.lock().unwrap();
	let node_metadata = graphene_core::registry::NODE_METADATA.lock().unwrap();
	for (id, metadata) in node_metadata.iter() {
		if let Some(implementations) = node_registry.get(id) {
			let identifier = match id_to_identifier_map.get(id) {
				Some(&id) => id.to_string(),
				None => continue,
			};

			// Extract category from metadata (already creates an owned String)
			let category = metadata.category.unwrap_or_default().to_string();

			// Extract input types (already creates owned Strings)
			let input_types = implementations
				.iter()
				.flat_map(|(_, node_io)| node_io.inputs.iter().map(|ty| ty.clone().nested_type().to_string()))
				.collect::<HashSet<String>>()
				.into_iter()
				.collect::<Vec<String>>();

			// Create a FrontendNodeType
			let node_type = FrontendNodeType::with_owned_strings_and_input_types(identifier, category, input_types);

			// Store the created node_type
			extracted_node_types.push(node_type);
		}
	}

	let node_types: Vec<FrontendNodeType> = DOCUMENT_NODE_TYPES
		.iter()
		.filter(|definition| !definition.category.is_empty())
		.map(|definition| {
			let input_types = definition
				.node_template
				.document_node
				.inputs
				.iter()
				.filter_map(|node_input| node_input.as_value().map(|node_value| node_value.ty().nested_type().to_string()))
				.collect::<Vec<String>>();

			FrontendNodeType::with_input_types(definition.identifier, definition.category, input_types)
		})
		.collect();

	// Update categories in extracted_node_types from node_types
	for extracted_node in &mut extracted_node_types {
		if extracted_node.category.is_empty() {
			// Find matching node in node_types and update category if found
			if let Some(matching_node) = node_types.iter().find(|node_type| node_type.name == extracted_node.name) {
				extracted_node.category = matching_node.category.clone();
			}
		}
	}
	let missing_nodes: Vec<FrontendNodeType> = node_types
		.iter()
		.filter(|node| !extracted_node_types.iter().any(|extracted| extracted.name == node.name))
		.cloned()
		.collect();

	// Add the missing nodes to extracted_node_types
	for node in missing_nodes {
		extracted_node_types.push(node);
	}
	// Remove entries with empty categories
	extracted_node_types.retain(|node| !node.category.is_empty());

	extracted_node_types
}

pub fn collect_node_descriptions() -> Vec<(String, String)> {
	DOCUMENT_NODE_TYPES
		.iter()
		.map(|definition| (definition.identifier.to_string(), definition.description.to_string()))
		.collect()
}

impl DocumentNodeDefinition {
	/// Converts the [DocumentNodeDefinition] type to a [NodeTemplate], using the provided `input_override` and falling back to the default inputs.
	/// `input_override` does not have to be the correct length.
	pub fn node_template_input_override(&self, input_override: impl IntoIterator<Item = Option<NodeInput>>) -> NodeTemplate {
		let mut template = self.node_template.clone();
		input_override.into_iter().enumerate().for_each(|(index, input_override)| {
			if let Some(input_override) = input_override {
				// Only value inputs can be overridden, since node inputs change graph structure and must be handled by the network interface
				// assert!(matches!(input_override, NodeInput::Value { .. }), "Only value inputs are supported for input overrides");
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
				nested_node_metadata.persistent_metadata.input_properties.resize_with(input_length, PropertiesRow::default);

				//Recurse over all sub nodes if the current node is a network implementation
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
				node_template.persistent_node_metadata.input_properties.resize_with(input_len, PropertiesRow::default);
				if let DocumentNodeImplementation::Network(node_template_network) = &node_template.document_node.implementation {
					for sub_node_id in node_template_network.nodes.keys().cloned().collect::<Vec<_>>() {
						populate_input_properties(node_template, vec![sub_node_id]);
					}
				}
			}
		}
		populate_input_properties(&mut template, Vec::new());

		// Set the reference to the node definition
		template.persistent_node_metadata.reference = Some(self.identifier.to_string());
		template
	}

	/// Converts the [DocumentNodeDefinition] type to a [NodeTemplate], completely default.
	pub fn default_node_template(&self) -> NodeTemplate {
		self.node_template_input_override(self.node_template.document_node.inputs.clone().into_iter().map(Some))
	}
}

// Previously used by the Imaginate node, but usage was commented out since it did nothing.
// pub fn new_image_network(output_offset: i32, output_node_id: NodeId) -> NodeNetwork {
// 	let mut network = NodeNetwork { ..Default::default() };
// 	network.push_node_to_document_network(
// 		resolve_document_node_type("Input Frame")
// 			.expect("Input Frame node does not exist")
// 			.to_document_node_default_inputs([], DocumentNodeMetadata::position((8, 4))),
// 	);
// 	network.push_node_to_document_network(
// 		resolve_document_node_type("Output")
// 			.expect("Output node does not exist")
// 			.to_document_node([NodeInput::node(output_node_id, 0)], DocumentNodeMetadata::position((output_offset + 8, 4))),
// 	);
// 	network
// }
