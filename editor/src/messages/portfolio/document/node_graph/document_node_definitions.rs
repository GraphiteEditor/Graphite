use super::node_properties;
use super::utility_types::FrontendNodeType;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::node_graph::node_properties::node_no_properties;
use crate::messages::portfolio::document::utility_types::network_interface::{
	DocumentNodeMetadata, DocumentNodePersistentMetadata, NodeNetworkInterface, NodeNetworkMetadata, NodeNetworkPersistentMetadata, NodeTemplate, NodeTypePersistentMetadata,
};
use crate::messages::portfolio::utility_types::PersistentData;
use crate::messages::prelude::Message;
use crate::node_graph_executor::NodeGraphExecutor;

use graph_craft::concrete;
use graph_craft::document::value::*;
use graph_craft::document::*;
use graph_craft::imaginate_input::ImaginateSamplingMethod;
use graph_craft::ProtoNodeIdentifier;
use graphene_core::raster::brush_cache::BrushCache;
use graphene_core::raster::{CellularDistanceFunction, CellularReturnType, Color, DomainWarpType, FractalType, Image, ImageFrame, NoiseType, RedGreenBlue, RedGreenBlueAlpha};
use graphene_core::text::Font;
use graphene_core::transform::Footprint;
use graphene_core::vector::VectorData;
use graphene_core::*;
use graphene_std::wasm_application_io::WasmEditorApi;

#[cfg(feature = "gpu")]
use wgpu_executor::{Bindgroup, CommandBuffer, PipelineLayout, ShaderHandle, ShaderInputFrame, WgpuShaderInput};

use glam::DVec2;
use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet, VecDeque};

pub struct NodePropertiesContext<'a> {
	pub persistent_data: &'a PersistentData,
	pub responses: &'a mut VecDeque<Message>,
	pub executor: &'a mut NodeGraphExecutor,
	pub network_interface: &'a NodeNetworkInterface,
	pub selection_network_path: &'a [NodeId],
	pub document_name: &'a str,
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
	pub properties: &'static (dyn Fn(&DocumentNode, NodeId, &mut NodePropertiesContext) -> Vec<LayoutGroup> + Sync),

	/// User-facing description of the node's functionality.
	pub description: Cow<'static, str>,
}

// We use the once cell for lazy initialization to avoid the overhead of reconstructing the node list every time.
// TODO: make document nodes not require a `'static` lifetime to avoid having to split the construction into const and non-const parts.
static DOCUMENT_NODE_TYPES: once_cell::sync::Lazy<Vec<DocumentNodeDefinition>> = once_cell::sync::Lazy::new(static_nodes);

// TODO: Dynamic node library
/// Defines the "signature" or "header file"-like metadata for the document nodes, but not the implementation (which is defined in the node registry).
/// The [`DocumentNode`] is the instance while these [`DocumentNodeDefinition`]s are the "classes" or "blueprints" from which the instances are built.
fn static_nodes() -> Vec<DocumentNodeDefinition> {
	let mut custom = vec![
		// TODO: Auto-generate this from its proto node macro
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
			properties: &node_properties::node_no_properties,
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
					input_names: vec!["In".to_string()],
					output_names: vec!["Out".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("The identity node passes its data through. You can use this to organize your node graph."),
			properties: &|_document_node, _node_id, _context| node_properties::string_properties("The identity node simply passes its data through"),
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
					input_names: vec!["In".to_string()],
					output_names: vec!["Out".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("The Monitor node is used by the editor to access the data flowing through it."),
			properties: &|_document_node, _node_id, _context| node_properties::string_properties("The Monitor node is used by the editor to access the data flowing through it"),
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
						NodeInput::value(TaggedValue::GraphicGroup(GraphicGroup::EMPTY), true),
						NodeInput::value(TaggedValue::GraphicGroup(GraphicGroup::EMPTY), true),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Graphical Data".to_string(), "Over".to_string()],
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
			properties: &node_properties::node_no_properties,
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
								manual_composition: Some(concrete!(Footprint)),
								inputs: vec![
									NodeInput::network(graphene_core::Type::Fn(Box::new(concrete!(Footprint)), Box::new(concrete!(ArtboardGroup))), 0),
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
						NodeInput::value(TaggedValue::ArtboardGroup(ArtboardGroup::EMPTY), true),
						NodeInput::value(TaggedValue::GraphicGroup(GraphicGroup::EMPTY), true),
						NodeInput::value(TaggedValue::IVec2(glam::IVec2::ZERO), false),
						NodeInput::value(TaggedValue::IVec2(glam::IVec2::new(1920, 1080)), false),
						NodeInput::value(TaggedValue::Color(Color::WHITE), false),
						NodeInput::value(TaggedValue::Bool(false), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec![
						"Artboards".to_string(),
						"Contents".to_string(),
						"Location".to_string(),
						"Dimensions".to_string(),
						"Background".to_string(),
						"Clip".to_string(),
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
			properties: &node_properties::artboard_properties,
		},
		DocumentNodeDefinition {
			identifier: "Load Image",
			category: "Raster: Generator",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(2), 0)],
						nodes: [
							DocumentNode {
								inputs: vec![NodeInput::value(TaggedValue::None, false), NodeInput::scope("editor-api"), NodeInput::network(concrete!(String), 1)],
								manual_composition: Some(concrete!(())),
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::wasm_application_io::LoadResourceNode")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(0), 0)],
								manual_composition: Some(concrete!(())),
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::wasm_application_io::DecodeImageNode")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(1), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::transform::CullNode")),
								manual_composition: Some(concrete!(Footprint)),
								..Default::default()
							},
						]
						.into_iter()
						.enumerate()
						.map(|(id, node)| (NodeId(id as u64), node))
						.collect(),
						..Default::default()
					}),
					inputs: vec![NodeInput::scope("editor-api"), NodeInput::value(TaggedValue::String("graphite:null".to_string()), false)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["api".to_string(), "path".to_string()],
					output_names: vec!["Image Frame".to_string()],
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
			description: Cow::Borrowed("Loads an image from a given url."),
			properties: &node_properties::load_image_properties,
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
								manual_composition: Some(concrete!(())),
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
					output_names: vec!["Image Frame".to_string()],
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
			properties: &node_properties::node_no_properties,
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
								inputs: vec![NodeInput::network(concrete!(ImageFrame<Color>), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<_, ImageFrame>")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::scope("editor-api")],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::wasm_application_io::CreateSurfaceNode")),
								skip_deduplication: true,
								..Default::default()
							},
							DocumentNode {
								manual_composition: Some(concrete!(())),
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
					inputs: vec![NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["In".to_string()],
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
			properties: &node_properties::node_no_properties,
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
								manual_composition: Some(concrete!(())),
								skip_deduplication: true,
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(0), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::MemoNode")),
								manual_composition: Some(concrete!(())),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::network(generic!(T), 0), NodeInput::network(concrete!(Footprint), 1), NodeInput::node(NodeId(1), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::wasm_application_io::RasterizeNode")),
								manual_composition: Some(concrete!(())),
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
						NodeInput::value(TaggedValue::VectorData(VectorData::default()), true),
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
					input_names: vec!["Artwork".to_string(), "Footprint".to_string()],
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
			properties: &node_properties::rasterize_properties,
		},
		DocumentNodeDefinition {
			identifier: "Image Frame",
			category: "Debug",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(1), 0)],
						nodes: vec![
							DocumentNode {
								inputs: vec![NodeInput::network(concrete!(graphene_core::raster::Image<Color>), 0), NodeInput::network(concrete!(DAffine2), 1)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::raster::ImageFrameNode")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(0), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::transform::CullNode")),
								manual_composition: Some(concrete!(Footprint)),
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
						NodeInput::value(TaggedValue::Image(Image::empty()), true),
						NodeInput::value(TaggedValue::DAffine2(DAffine2::IDENTITY), true),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string(), "Transform".to_string()],
					output_names: vec!["Image".to_string()],
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Image Frame".to_string(),
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
			description: Cow::Borrowed("Creates an embedded image with the given transform."),
			properties: &|_document_node, _node_id, _context| node_properties::string_properties("Creates an embedded image with the given transform"),
		},
		DocumentNodeDefinition {
			identifier: "Noise Pattern",
			category: "Raster",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					manual_composition: Some(concrete!(Footprint)),
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::raster::NoisePatternNode")),
					inputs: vec![
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
					input_names: vec![
						"Clip".to_string(),
						"Seed".to_string(),
						"Scale".to_string(),
						"Noise Type".to_string(),
						"Domain Warp Type".to_string(),
						"Domain Warp Amplitude".to_string(),
						"Fractal Type".to_string(),
						"Fractal Octaves".to_string(),
						"Fractal Lacunarity".to_string(),
						"Fractal Gain".to_string(),
						"Fractal Weighted Strength".to_string(),
						"Fractal Ping Pong Strength".to_string(),
						"Cellular Distance Function".to_string(),
						"Cellular Return Type".to_string(),
						"Cellular Jitter".to_string(),
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
			properties: &node_properties::noise_pattern_properties,
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
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string(), "Stencil".to_string()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: &node_properties::mask_properties,
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
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::RedGreenBlue(RedGreenBlue::default()), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string(), "Insertion".to_string(), "Replace".to_string()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: &node_properties::insert_channel_properties,
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
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["None".to_string(), "Red".to_string(), "Green".to_string(), "Blue".to_string(), "Alpha".to_string()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: &node_properties::node_no_properties,
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
									NodeInput::network(concrete!(ImageFrame<Color>), 0),
									NodeInput::value(TaggedValue::RedGreenBlueAlpha(RedGreenBlueAlpha::Red), false),
								],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::raster::adjustments::ExtractChannelNode")),
								manual_composition: Some(generic!(T)),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![
									NodeInput::network(concrete!(ImageFrame<Color>), 0),
									NodeInput::value(TaggedValue::RedGreenBlueAlpha(RedGreenBlueAlpha::Green), false),
								],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::raster::adjustments::ExtractChannelNode")),
								manual_composition: Some(generic!(T)),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![
									NodeInput::network(concrete!(ImageFrame<Color>), 0),
									NodeInput::value(TaggedValue::RedGreenBlueAlpha(RedGreenBlueAlpha::Blue), false),
								],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::raster::adjustments::ExtractChannelNode")),
								manual_composition: Some(generic!(T)),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![
									NodeInput::network(concrete!(ImageFrame<Color>), 0),
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
					inputs: vec![NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string()],
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
			properties: &node_properties::node_no_properties,
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
								NodeInput::network(concrete!(graphene_core::raster::ImageFrame<Color>), 0),
								NodeInput::network(concrete!(graphene_core::raster::ImageFrame<Color>), 1),
								NodeInput::network(concrete!(Vec<graphene_core::vector::brush_stroke::BrushStroke>), 2),
								NodeInput::network(concrete!(BrushCache), 3),
							],
							manual_composition: Some(concrete!(Footprint)),
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
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::BrushStrokes(Vec::new()), false),
						NodeInput::value(TaggedValue::BrushCache(BrushCache::new_proto()), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Background".to_string(), "Bounds".to_string(), "Trace".to_string(), "Cache".to_string()],
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
			properties: &node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "Memoize",
			category: "Debug",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::memo::MemoNode"),
					inputs: vec![NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true)],
					manual_composition: Some(concrete!(())),
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: &node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "Memoize Impure",
			category: "Debug",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::memo::ImpureMemoNode"),
					inputs: vec![NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true)],
					manual_composition: Some(concrete!(Footprint)),
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: &node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "Image",
			category: "",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(0), 0)],
						nodes: vec![DocumentNode {
							inputs: vec![NodeInput::network(concrete!(ImageFrame<Color>), 0)],
							implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::transform::CullNode")),
							manual_composition: Some(concrete!(Footprint)),
							..Default::default()
						}]
						.into_iter()
						.enumerate()
						.map(|(id, node)| (NodeId(id as u64), node))
						.collect(),
						..Default::default()
					}),
					inputs: vec![NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), false)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string()],
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
			properties: &|_document_node, _node_id, _context| node_properties::string_properties("A bitmap image embedded in this node"),
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
								manual_composition: Some(concrete!(())),
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::UniformNode")),
								..Default::default()
							},
							DocumentNode {
								manual_composition: Some(concrete!(())),
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
					input_names: vec!["In".to_string()],
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
			properties: &node_properties::node_no_properties,
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
								manual_composition: Some(concrete!(())),
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
					input_names: vec!["In".to_string()],
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
			properties: &node_properties::node_no_properties,
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
								manual_composition: Some(concrete!(())),
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
					input_names: vec!["In".to_string(), "In".to_string()],
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
			properties: &node_properties::node_no_properties,
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
								manual_composition: Some(concrete!(())),
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
					input_names: vec!["In".to_string(), "In".to_string(), "In".to_string()],
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
			properties: &node_properties::node_no_properties,
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
					input_names: vec!["Shader Handle".to_string(), "String".to_string(), "Bindgroup".to_string(), "Arc Shader Input".to_string()],
					output_names: vec!["Pipeline Layout".to_string()],
					..Default::default()
				},
			},
			properties: &node_properties::node_no_properties,
			description: Cow::Borrowed("TODO"),
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
								manual_composition: Some(concrete!(())),
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
					input_names: vec!["In".to_string()],
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
			properties: &node_properties::node_no_properties,
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
								manual_composition: Some(concrete!(())),
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
					input_names: vec!["In".to_string()],
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
			properties: &node_properties::node_no_properties,
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
								manual_composition: Some(concrete!(())),
								inputs: vec![NodeInput::scope("editor-api")],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::CreateGpuSurfaceNode")),
								..Default::default()
							},
							DocumentNode {
								manual_composition: Some(concrete!(Footprint)),
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
			properties: &node_properties::node_no_properties,
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
								manual_composition: Some(concrete!(Footprint)),
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
					input_names: vec!["Texture".to_string(), "Surface".to_string()],
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
			properties: &node_properties::node_no_properties,
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
								inputs: vec![NodeInput::network(concrete!(ImageFrame<Color>), 0), NodeInput::node(NodeId(0), 0)],
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
					inputs: vec![NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["In".to_string()],
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
			properties: &node_properties::node_no_properties,
		},
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			identifier: "GPU Image",
			category: "Debug: GPU",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_std::executor::MapGpuSingleImageNode"),
					inputs: vec![
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::DocumentNode(DocumentNode::default()), true),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string(), "Node".to_string()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: &node_properties::node_no_properties,
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
					input_names: vec!["Node".to_string()],
					output_names: vec!["Document Node".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: &node_properties::node_no_properties,
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
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::Bool(false), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string(), "Brightness".to_string(), "Contrast".to_string(), "Use Classic".to_string()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: &node_properties::brightness_contrast_properties,
		},
		// Aims for interoperable compatibility with:
		// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=levl%27%20%3D%20Levels-,%27curv%27%20%3D%20Curves,-%27expA%27%20%3D%20Exposure
		// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=Max%20input%20range-,Curves,-Curves%20settings%20files
		DocumentNodeDefinition {
			identifier: "Curves",
			category: "Raster: Adjustment",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::raster::CurvesNode"),
					inputs: vec![
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::Curve(Default::default()), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string(), "Curve".to_string()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: &node_properties::curves_properties,
		},
		(*IMAGINATE_NODE).clone(),
		DocumentNodeDefinition {
			identifier: "Line",
			category: "Vector: Shape",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::vector::generator_nodes::LineNode"),
					manual_composition: Some(concrete!(())),
					inputs: vec![
						NodeInput::value(TaggedValue::None, false),
						NodeInput::value(TaggedValue::DVec2(DVec2::new(0., -50.)), false),
						NodeInput::value(TaggedValue::DVec2(DVec2::new(0., 50.)), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["None".to_string(), "Start".to_string(), "End".to_string()],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: &node_properties::line_properties,
		},
		DocumentNodeDefinition {
			identifier: "Spline",
			category: "Vector: Shape",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::vector::generator_nodes::SplineNode"),
					manual_composition: Some(concrete!(())),
					inputs: vec![
						NodeInput::value(TaggedValue::None, false),
						// TODO: Modify the proto node generation macro to accept this default value, then remove this definition for Spline
						NodeInput::value(TaggedValue::VecDVec2(vec![DVec2::new(0., -50.), DVec2::new(25., 0.), DVec2::new(0., 50.)]), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["None".to_string(), "Points".to_string()],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: &node_properties::spline_properties,
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
								inputs: vec![NodeInput::network(concrete!(VectorData), 0)],
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
						NodeInput::value(TaggedValue::VectorData(VectorData::empty()), true),
						NodeInput::value(TaggedValue::VectorModification(Default::default()), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Vector Data".to_string(), "Modification".to_string()],
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
			properties: &node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "Text",
			category: "Vector",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_std::text::TextNode"),
					manual_composition: Some(concrete!(())),
					inputs: vec![
						NodeInput::scope("editor-api"),
						NodeInput::value(TaggedValue::String("Lorem ipsum".to_string()), false),
						NodeInput::value(
							TaggedValue::Font(Font::new(graphene_core::consts::DEFAULT_FONT_FAMILY.into(), graphene_core::consts::DEFAULT_FONT_STYLE.into())),
							false,
						),
						NodeInput::value(TaggedValue::F64(24.), false),
						NodeInput::value(TaggedValue::F64(1.2), false),
						NodeInput::value(TaggedValue::F64(1.), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec![
						"Editor API".to_string(),
						"Text".to_string(),
						"Font".to_string(),
						"Size".to_string(),
						"Line Height".to_string(),
						"Character Spacing".to_string(),
					],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
			properties: &node_properties::text_properties,
		},
		DocumentNodeDefinition {
			identifier: "Transform",
			category: "General",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					inputs: vec![
						NodeInput::value(TaggedValue::VectorData(VectorData::empty()), true),
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
								inputs: vec![NodeInput::network(concrete!(VectorData), 0)],
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
								manual_composition: Some(concrete!(Footprint)),
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
					input_names: vec![
						"Vector Data".to_string(),
						"Translation".to_string(),
						"Rotation".to_string(),
						"Scale".to_string(),
						"Skew".to_string(),
						"Pivot".to_string(),
					],
					output_names: vec!["Data".to_string()],
					..Default::default()
				},
			},
			properties: &node_properties::transform_properties,
			description: Cow::Borrowed("TODO"),
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
								inputs: vec![NodeInput::network(concrete!(VectorData), 0), NodeInput::network(concrete!(vector::style::Fill), 1)],
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
						NodeInput::value(TaggedValue::GraphicGroup(GraphicGroup::EMPTY), true),
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
					input_names: vec!["Group of Paths".to_string(), "Operation".to_string()],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},
			properties: &node_properties::boolean_operation_properties,
			description: Cow::Borrowed("TODO"),
		},
		DocumentNodeDefinition {
			identifier: "Copy to Points",
			category: "Vector",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					// TODO: Wrap this implementation with a document node that has a cache node so the output is cached?
					implementation: DocumentNodeImplementation::proto("graphene_core::vector::CopyToPointsNode"),
					inputs: vec![
						NodeInput::value(TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
						NodeInput::value(TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
						NodeInput::value(TaggedValue::F64(1.), false),
						NodeInput::value(TaggedValue::F64(1.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::U32(0), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::U32(0), false),
					],
					manual_composition: Some(concrete!(Footprint)),
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec![
						"Points".to_string(),
						"Instance".to_string(),
						"Random Scale Min".to_string(),
						"Random Scale Max".to_string(),
						"Random Scale Bias".to_string(),
						"Random Scale Seed".to_string(),
						"Random Rotation".to_string(),
						"Random Rotation Seed".to_string(),
					],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},
			properties: &node_properties::copy_to_points_properties,
			description: Cow::Borrowed("TODO"),
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
								inputs: vec![NodeInput::network(concrete!(graphene_core::vector::VectorData), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::vector::vector_nodes::SubpathSegmentLengthsNode")),
								manual_composition: Some(generic!(T)),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![
									NodeInput::network(concrete!(graphene_core::vector::VectorData), 0),
									NodeInput::network(concrete!(f64), 1),  // From the document node's parameters
									NodeInput::network(concrete!(f64), 2),  // From the document node's parameters
									NodeInput::network(concrete!(f64), 3),  // From the document node's parameters
									NodeInput::network(concrete!(bool), 4), // From the document node's parameters
									NodeInput::node(NodeId(0), 0),          // From output 0 of SubpathSegmentLengthsNode
								],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::vector::vector_nodes::SamplePointsNode")),
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
						NodeInput::value(TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
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
					input_names: vec![
						"Vector Data".to_string(),
						"Spacing".to_string(),
						"Start Offset".to_string(),
						"Stop Offset".to_string(),
						"Adaptive Spacing".to_string(),
					],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},
			properties: &node_properties::sample_points_properties,
			description: Cow::Borrowed("TODO"),
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
									NodeInput::network(concrete!(graphene_core::vector::VectorData), 0),
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
						NodeInput::value(TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
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
					input_names: vec!["Vector Data".to_string(), "Separation Disk Diameter".to_string(), "Seed".to_string()],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},
			properties: &node_properties::poisson_disk_points_properties,
			description: Cow::Borrowed("TODO"),
		},
		// TODO: This needs to work with resolution-aware (raster with footprint, post-Cull node) data.
		DocumentNodeDefinition {
			identifier: "Index",
			category: "Debug",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::raster::IndexNode"),
					inputs: vec![NodeInput::value(TaggedValue::Segments(vec![ImageFrame::empty()]), true), NodeInput::value(TaggedValue::U32(0), false)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Segmentation".to_string(), "Index".to_string()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			properties: &node_properties::index_properties,
			description: Cow::Borrowed("TODO"),
		},
	];

	type PropertiesLayout = &'static (dyn Fn(&DocumentNode, NodeId, &mut NodePropertiesContext) -> Vec<LayoutGroup> + Sync);
	let properties_overrides = [
		("graphene_core::raster::adjustments::ChannelMixerNode", &node_properties::channel_mixer_properties as PropertiesLayout),
		("graphene_core::vector::FillNode", &node_properties::fill_properties as PropertiesLayout),
		("graphene_core::vector::StrokeNode", &node_properties::stroke_properties as PropertiesLayout),
		("graphene_core::vector::OffsetPathNode", &node_properties::offset_path_properties as PropertiesLayout),
		(
			"graphene_core::raster::adjustments::SelectiveColorNode",
			&node_properties::selective_color_properties as PropertiesLayout,
		),
		("graphene_core::raster::ExposureNode", &node_properties::exposure_properties as PropertiesLayout),
		("graphene_core::vector::generator_nodes::RectangleNode", &node_properties::rectangle_properties as PropertiesLayout),
		("graphene_core::vector::AssignColorsNode", &node_properties::assign_colors_properties as PropertiesLayout),
	]
	.into_iter()
	.collect::<HashMap<_, _>>();

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
	'outer: for (id, metadata) in graphene_core::registry::NODE_METADATA.lock().unwrap().drain() {
		use graphene_core::registry::*;

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
		} = metadata;
		let Some(implementations) = &node_registry.get(&id) else { continue };
		let valid_inputs: HashSet<_> = implementations.iter().map(|(_, node_io)| node_io.call_argument.clone()).collect();
		let first_node_io = implementations.first().map(|(_, node_io)| node_io).unwrap_or(const { &NodeIOTypes::empty() });
		let mut input_type = &first_node_io.call_argument;
		if valid_inputs.len() > 1 {
			input_type = &const { generic!(T) };
		}
		let output_type = &first_node_io.return_value;

		let inputs = fields
			.iter()
			.zip(first_node_io.inputs.iter())
			.enumerate()
			.map(|(index, (field, ty))| {
				let exposed = if index == 0 { *ty != fn_type!(()) } else { field.exposed };

				match field.value_source {
					ValueSource::None => {}
					ValueSource::Default(data) => return NodeInput::value(TaggedValue::from_primitive_string(data, ty).unwrap_or(TaggedValue::None), exposed),
					ValueSource::Scope(data) => return NodeInput::scope(Cow::Borrowed(data)),
				};

				if let Some(type_default) = TaggedValue::from_type(ty) {
					return NodeInput::value(type_default, exposed);
				}
				NodeInput::value(TaggedValue::None, true)
			})
			.collect();

		let properties = match properties_overrides.get(id.as_str()) {
			Some(properties_function) => *properties_function,
			None => {
				let field_types: Vec<_> = fields.iter().zip(first_node_io.inputs.iter()).map(|(field, ty)| (field.clone(), ty.clone())).collect();
				let properties = move |document_node: &DocumentNode, node_id: NodeId, context: &mut NodePropertiesContext| {
					let rows: Vec<_> = field_types
						.iter()
						.enumerate()
						.skip(1)
						.filter(|(_, (field, _))| !matches!(&field.value_source, ValueSource::Scope(_)))
						.flat_map(|(index, (field, ty))| {
							let number_options = (field.number_min, field.number_max, field.number_mode_range);

							node_properties::property_from_type(document_node, node_id, index, field.name, ty, context, number_options)
						})
						.collect();

					if rows.is_empty() {
						return node_no_properties(document_node, node_id, context);
					}

					rows
				};
				Box::leak(Box::new(properties)) as PropertiesLayout
			}
		};

		let node = DocumentNodeDefinition {
			identifier: display_name,
			node_template: NodeTemplate {
				document_node: DocumentNode {
					inputs,
					manual_composition: Some(input_type.clone()),
					implementation: DocumentNodeImplementation::ProtoNode(id.into()),
					visible: true,
					skip_deduplication: false,
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: fields.iter().map(|f| f.name.to_string()).collect(),
					output_names: vec![output_type.to_string()],
					has_primary_output: true,
					locked: false,
					..Default::default()
				},
			},
			category: category.unwrap_or("UNCATEGORIZED"),
			description: Cow::Borrowed(description),
			properties,
		};
		custom.push(node);
	}
	custom
}

pub static IMAGINATE_NODE: Lazy<DocumentNodeDefinition> = Lazy::new(|| DocumentNodeDefinition {
	identifier: "Imaginate",
	category: "Raster: Generator",
	node_template: NodeTemplate {
		document_node: DocumentNode {
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![NodeInput::node(NodeId(1), 0)],
				nodes: [
					DocumentNode {
						inputs: vec![NodeInput::network(concrete!(ImageFrame<Color>), 0)],
						implementation: DocumentNodeImplementation::proto("graphene_core::memo::MonitorNode"),
						manual_composition: Some(concrete!(())),
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
				NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
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
			input_names: vec![
				"Input Image".to_string(),
				"Editor Api".to_string(),
				"Controller".to_string(),
				"Seed".to_string(),
				"Resolution".to_string(),
				"Samples".to_string(),
				"Sampling Method".to_string(),
				"Prompt Guidance".to_string(),
				"Prompt".to_string(),
				"Negative Prompt".to_string(),
				"Adapt Input Image".to_string(),
				"Image Creativity".to_string(),
				"Inpaint".to_string(),
				"Mask Blur".to_string(),
				"Mask Starting Fill".to_string(),
				"Improve Faces".to_string(),
				"Tiling".to_string(),
			],
			output_names: vec!["Image".to_string()],
			..Default::default()
		},
	},
	properties: &node_properties::imaginate_properties,
	description: Cow::Borrowed("TODO"),
});

pub fn resolve_document_node_type(identifier: &str) -> Option<&DocumentNodeDefinition> {
	DOCUMENT_NODE_TYPES.iter().find(|definition| definition.identifier == identifier)
}

pub fn collect_node_types() -> Vec<FrontendNodeType> {
	DOCUMENT_NODE_TYPES
		.iter()
		.filter(|definition| !definition.category.is_empty())
		.map(|definition| FrontendNodeType::new(definition.identifier, definition.category))
		.collect()
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
