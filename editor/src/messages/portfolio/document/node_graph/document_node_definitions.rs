use super::node_properties;
use super::utility_types::FrontendNodeType;
use crate::messages::layout::utility_types::widget_prelude::*;
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
use graphene_core::raster::{
	BlendMode, CellularDistanceFunction, CellularReturnType, Color, DomainWarpType, FractalType, Image, ImageFrame, LuminanceCalculation, NoiseType, RedGreenBlue, RedGreenBlueAlpha, RelativeAbsolute,
	SelectiveColorChoice,
};
use graphene_core::text::Font;
use graphene_core::transform::Footprint;
use graphene_core::vector::VectorData;
use graphene_core::*;
use graphene_std::application_io::RenderConfig;
use graphene_std::wasm_application_io::WasmEditorApi;

use glam::DVec2;

#[cfg(feature = "gpu")]
use wgpu_executor::{Bindgroup, CommandBuffer, PipelineLayout, ShaderHandle, ShaderInputFrame, WgpuShaderInput};

use once_cell::sync::Lazy;
use std::collections::VecDeque;

pub struct NodePropertiesContext<'a> {
	pub persistent_data: &'a PersistentData,
	pub responses: &'a mut VecDeque<Message>,
	pub executor: &'a mut NodeGraphExecutor,
	pub network_interface: &'a NodeNetworkInterface,
	pub selection_network_path: &'a [NodeId],
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
	pub properties: fn(&DocumentNode, NodeId, &mut NodePropertiesContext) -> Vec<LayoutGroup>,
}

// We use the once cell for lazy initialization to avoid the overhead of reconstructing the node list every time.
// TODO: make document nodes not require a `'static` lifetime to avoid having to split the construction into const and non-const parts.
static DOCUMENT_NODE_TYPES: once_cell::sync::Lazy<Vec<DocumentNodeDefinition>> = once_cell::sync::Lazy::new(static_nodes);

// TODO: Dynamic node library
/// Defines the "signature" or "header file"-like metadata for the document nodes, but not the implementation (which is defined in the node registry).
/// The [`DocumentNode`] is the instance while these [`DocumentNodeDefinition`]s are the "classes" or "blueprints" from which the instances are built.
fn static_nodes() -> Vec<DocumentNodeDefinition> {
	vec![
		DocumentNodeDefinition {
			identifier: "Bool Value",
			category: "Value",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::ops::IdentityNode"),
					inputs: vec![NodeInput::value(TaggedValue::Bool(true), false)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Bool".to_string()],
					output_names: vec!["Out".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::boolean_properties,
		},
		DocumentNodeDefinition {
			identifier: "Number Value",
			category: "Value",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::ops::IdentityNode"),
					inputs: vec![NodeInput::value(TaggedValue::F64(0.), false)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Number".to_string()],
					output_names: vec!["Out".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::number_properties,
		},
		DocumentNodeDefinition {
			identifier: "Percentage Value",
			category: "Value",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::ops::IdentityNode"),
					inputs: vec![NodeInput::value(TaggedValue::F64(0.), false)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Percentage".to_string()],
					output_names: vec!["Out".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::percentage_properties,
		},
		DocumentNodeDefinition {
			identifier: "Color Value",
			category: "Value",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::ops::IdentityNode"),
					inputs: vec![NodeInput::value(TaggedValue::OptionalColor(None), false)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Color".to_string()],
					output_names: vec!["Out".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::color_properties,
		},
		DocumentNodeDefinition {
			identifier: "Gradient Value",
			category: "Value",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::ops::IdentityNode"),
					inputs: vec![NodeInput::value(TaggedValue::GradientStops(vector::style::GradientStops::default()), false)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Gradient".to_string()],
					output_names: vec!["Out".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::gradient_properties,
		},
		DocumentNodeDefinition {
			identifier: "Vector2 Value",
			category: "Value",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::ops::construct_vector2::ConstructVector2"),
					inputs: vec![
						NodeInput::value(TaggedValue::None, false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["None".to_string(), "X".to_string(), "Y".to_string()],
					output_names: vec!["Out".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::vector2_properties,
		},
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
			properties: |_document_node, _node_id, _context| node_properties::string_properties("The identity node simply returns the input"),
		},
		DocumentNodeDefinition {
			identifier: "Monitor",
			category: "Debug",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::memo::MonitorNode<_, _, _>"),
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
			properties: |_document_node, _node_id, _context| node_properties::string_properties("The Monitor node stores the value of its last evaluation"),
		},
		DocumentNodeDefinition {
			identifier: "Group",
			category: "General",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::ToGraphicGroupNode"),
					inputs: vec![NodeInput::value(TaggedValue::VectorData(VectorData::empty()), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Element".to_string()],
					output_names: vec!["Graphic Group".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::node_no_properties,
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
								implementation: DocumentNodeImplementation::proto("graphene_core::ToGraphicElementNode"),
								..Default::default()
							},
							// Primary (bottom) input type coercion
							DocumentNode {
								inputs: vec![NodeInput::network(generic!(T), 0)],
								implementation: DocumentNodeImplementation::proto("graphene_core::ToGraphicGroupNode"),
								..Default::default()
							},
							// The monitor node is used to display a thumbnail in the UI
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(0), 0)],
								implementation: DocumentNodeImplementation::proto("graphene_core::memo::MonitorNode<_, _, _>"),
								manual_composition: Some(generic!(T)),
								skip_deduplication: true,
								..Default::default()
							},
							DocumentNode {
								manual_composition: Some(concrete!(Footprint)),
								inputs: vec![NodeInput::node(NodeId(1), 0), NodeInput::node(NodeId(2), 0)],
								implementation: DocumentNodeImplementation::proto("graphene_core::ConstructLayerNode<_, _>"),
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
										display_name: "To Graphic Element".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(-14, -1)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Group".to_string(),
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
										display_name: "ConstructLayer".to_string(),
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
			properties: node_properties::node_no_properties,
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
								manual_composition: Some(concrete!(Footprint)),
								inputs: vec![
									NodeInput::network(concrete!(TaggedValue), 1),
									NodeInput::value(TaggedValue::String(String::from("Artboard")), false),
									NodeInput::network(concrete!(TaggedValue), 2),
									NodeInput::network(concrete!(TaggedValue), 3),
									NodeInput::network(concrete!(TaggedValue), 4),
									NodeInput::network(concrete!(TaggedValue), 5),
								],
								implementation: DocumentNodeImplementation::proto("graphene_core::ConstructArtboardNode<_, _, _, _, _, _>"),
								..Default::default()
							},
							// The monitor node is used to display a thumbnail in the UI.
							// TODO: Check if thumbnail is reversed
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(0), 0)],
								implementation: DocumentNodeImplementation::proto("graphene_core::memo::MonitorNode<_, _, _>"),
								manual_composition: Some(generic!(T)),
								skip_deduplication: true,
								..Default::default()
							},
							DocumentNode {
								manual_composition: Some(concrete!(Footprint)),
								inputs: vec![
									NodeInput::network(graphene_core::Type::Fn(Box::new(concrete!(Footprint)), Box::new(concrete!(ArtboardGroup))), 0),
									NodeInput::node(NodeId(1), 0),
								],
								implementation: DocumentNodeImplementation::proto("graphene_core::AddArtboardNode<_, _>"),
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
										display_name: "Add to Artboards".to_string(),
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
			properties: node_properties::artboard_properties,
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
								inputs: vec![NodeInput::scope("editor-api"), NodeInput::network(concrete!(String), 1)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::wasm_application_io::LoadResourceNode<_>")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(0), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::wasm_application_io::DecodeImageNode")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(1), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::transform::CullNode<_>")),
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
			properties: node_properties::load_image_properties,
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
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::MemoNode<_, _>")),
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
			properties: node_properties::node_no_properties,
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
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<_, ImageFrame<SRGBA8>>")),
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
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::MemoNode<_, _>")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(0), 0), NodeInput::node(NodeId(2), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::wasm_application_io::DrawImageFrameNode<_>")),
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
			properties: node_properties::node_no_properties,
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
								skip_deduplication: true,
								..Default::default()
							},
							DocumentNode {
								manual_composition: Some(concrete!(())),
								inputs: vec![NodeInput::node(NodeId(0), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::MemoNode<_, _>")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::network(generic!(T), 0), NodeInput::network(concrete!(Footprint), 1), NodeInput::node(NodeId(1), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::wasm_application_io::RasterizeNode<_, _>")),
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
			properties: node_properties::rasterize_properties,
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
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::raster::ImageFrameNode<_, _>")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(0), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::transform::CullNode<_>")),
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
			properties: |_document_node, _node_id, _context| node_properties::string_properties("Creates an embedded image with the given transform"),
		},
		DocumentNodeDefinition {
			identifier: "Noise Pattern",
			category: "Raster",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					manual_composition: Some(concrete!(Footprint)),
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::raster::NoisePatternNode<_, _, _, _, _, _, _, _, _, _, _, _, _, _, _>")),
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
			properties: node_properties::noise_pattern_properties,
		},
		// TODO: This needs to work with resolution-aware (raster with footprint, post-Cull node) data.
		DocumentNodeDefinition {
			identifier: "Mask",
			category: "Raster",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_std::raster::MaskImageNode<_, _, _>"),
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
			properties: node_properties::mask_properties,
		},
		// TODO: This needs to work with resolution-aware (raster with footprint, post-Cull node) data.
		DocumentNodeDefinition {
			identifier: "Insert Channel",
			category: "Raster",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_std::raster::InsertChannelNode<_, _, _, _>"),
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
			properties: node_properties::insert_channel_properties,
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
			properties: node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "Unwrap",
			category: "Debug",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::ops::UnwrapNode"),
					inputs: vec![NodeInput::value(TaggedValue::OptionalColor(None), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Value".to_string()],
					output_names: vec!["Value".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::node_no_properties,
		},
		// TODO: Consolidate this into the regular Blend node once we can make its generic types all compatible, and not break the brush tool which uses that Blend node
		DocumentNodeDefinition {
			identifier: "Blend Colors",
			category: "Raster",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::raster::BlendColorsNode<_, _, _>"),
					inputs: vec![
						NodeInput::value(TaggedValue::Color(Color::TRANSPARENT), true),
						NodeInput::value(TaggedValue::Color(Color::TRANSPARENT), true),
						NodeInput::value(TaggedValue::BlendMode(BlendMode::Normal), false),
						NodeInput::value(TaggedValue::F64(100.), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Over".to_string(), "Under".to_string(), "Blend Mode".to_string(), "Opacity".to_string()],
					output_names: vec!["Combined".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::blend_color_properties,
		},
		// TODO: This needs to work with resolution-aware (raster with footprint, post-Cull node) data.
		DocumentNodeDefinition {
			identifier: "Blend",
			category: "Raster",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::raster::BlendNode<_, _, _, _>"),
					inputs: vec![
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::BlendMode(BlendMode::Normal), false),
						NodeInput::value(TaggedValue::F64(100.), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string(), "Second".to_string(), "Blend Mode".to_string(), "Opacity".to_string()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::blend_properties,
		},
		DocumentNodeDefinition {
			identifier: "Levels",
			category: "Raster: Adjustment",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::raster::LevelsNode<_, _, _, _, _>"),
					inputs: vec![
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(50.), false),
						NodeInput::value(TaggedValue::F64(100.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(100.), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec![
						"Image".to_string(),
						"Shadows".to_string(),
						"Midtones".to_string(),
						"Highlights".to_string(),
						"Output Minimums".to_string(),
						"Output Maximums".to_string(),
					],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::levels_properties,
		},
		DocumentNodeDefinition {
			identifier: "Black & White",
			category: "Raster: Adjustment",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::raster::BlackAndWhiteNode<_, _, _, _, _, _, _>"),
					inputs: vec![
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::Color(Color::BLACK), false),
						NodeInput::value(TaggedValue::F64(40.), false),
						NodeInput::value(TaggedValue::F64(60.), false),
						NodeInput::value(TaggedValue::F64(40.), false),
						NodeInput::value(TaggedValue::F64(60.), false),
						NodeInput::value(TaggedValue::F64(20.), false),
						NodeInput::value(TaggedValue::F64(80.), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec![
						"Image".to_string(),
						"Tint".to_string(),
						"Reds".to_string(),
						"Yellows".to_string(),
						"Greens".to_string(),
						"Cyans".to_string(),
						"Blues".to_string(),
						"Magentas".to_string(),
					],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::black_and_white_properties,
		},
		DocumentNodeDefinition {
			identifier: "Color Channel",
			category: "Raster: Adjustment",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::ops::IdentityNode"),
					inputs: vec![NodeInput::value(TaggedValue::RedGreenBlue(RedGreenBlue::Red), false)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Channel".to_string()],
					output_names: vec!["Out".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::color_channel_properties,
		},
		DocumentNodeDefinition {
			identifier: "Color Channel",
			category: "Raster: Adjustment",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::ops::IdentityNode"),
					inputs: vec![NodeInput::value(TaggedValue::RedGreenBlue(RedGreenBlue::Red), false)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Channel".to_string()],
					output_names: vec!["Out".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::color_channel_properties,
		},
		DocumentNodeDefinition {
			identifier: "Blend Mode Value",
			category: "Value",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::ops::IdentityNode"),
					inputs: vec![NodeInput::value(TaggedValue::BlendMode(BlendMode::default()), false)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Blend Mode".to_string()],
					output_names: vec!["Out".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::blend_mode_value_properties,
		},
		DocumentNodeDefinition {
			identifier: "Luminance",
			category: "Raster",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::raster::LuminanceNode<_>"),
					inputs: vec![
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::LuminanceCalculation(LuminanceCalculation::default()), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string(), "Luminance Calc".to_string()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::luminance_properties,
		},
		DocumentNodeDefinition {
			identifier: "Extract Channel",
			category: "Raster",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::raster::ExtractChannelNode<_>"),
					inputs: vec![
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::RedGreenBlueAlpha(RedGreenBlueAlpha::default()), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string(), "From".to_string()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::extract_channel_properties,
		},
		DocumentNodeDefinition {
			identifier: "Extract Opaque",
			category: "Raster",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::raster::ExtractOpaqueNode<>"),
					inputs: vec![
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::RedGreenBlueAlpha(RedGreenBlueAlpha::Red), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::node_no_properties,
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
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::raster::ExtractChannelNode<_>")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![
									NodeInput::network(concrete!(ImageFrame<Color>), 0),
									NodeInput::value(TaggedValue::RedGreenBlueAlpha(RedGreenBlueAlpha::Green), false),
								],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::raster::ExtractChannelNode<_>")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![
									NodeInput::network(concrete!(ImageFrame<Color>), 0),
									NodeInput::value(TaggedValue::RedGreenBlueAlpha(RedGreenBlueAlpha::Blue), false),
								],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::raster::ExtractChannelNode<_>")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![
									NodeInput::network(concrete!(ImageFrame<Color>), 0),
									NodeInput::value(TaggedValue::RedGreenBlueAlpha(RedGreenBlueAlpha::Alpha), false),
								],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::raster::ExtractChannelNode<_>")),
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
										display_name: "RedNode".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "GreenNode".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "BlueNode".to_string(),
										node_type_metadata: NodeTypePersistentMetadata::node(IVec2::new(0, 0)),
										..Default::default()
									},
									..Default::default()
								},
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "AlphaNode".to_string(),
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
			properties: node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "Brush",
			category: "Raster",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(1), 0)],
						nodes: vec![
							DocumentNode {
								inputs: vec![
									NodeInput::network(concrete!(graphene_core::raster::ImageFrame<Color>), 0),
									NodeInput::network(concrete!(graphene_core::raster::ImageFrame<Color>), 1),
									NodeInput::network(concrete!(Vec<graphene_core::vector::brush_stroke::BrushStroke>), 2),
									NodeInput::network(concrete!(BrushCache), 3),
								],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::brush::BrushNode<_, _, _>")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(0), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::transform::CullNode<_>")),
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
			properties: node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "Extract Vector Points",
			category: "Vector",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_std::brush::VectorPointsNode"),
					inputs: vec![NodeInput::value(TaggedValue::VectorData(VectorData::empty()), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["VectorData".to_string()],
					output_names: vec!["Vector Points".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "Memoize",
			category: "Debug",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::memo::MemoNode<_, _>"),
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
			properties: node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "MemoizeImpure",
			category: "Debug",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::memo::ImpureMemoNode<_, _, _>"),
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
			properties: node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "Image",
			category: "Ignore",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(0), 0)],
						nodes: vec![DocumentNode {
							inputs: vec![NodeInput::network(concrete!(ImageFrame<Color>), 0)],
							implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::transform::CullNode<_>")),
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
			properties: |_document_node, _node_id, _context| node_properties::string_properties("A bitmap image embedded in this node"),
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
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<_, &WgpuExecutor>")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::network(generic!(T), 0), NodeInput::node(NodeId(0), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::UniformNode<_>")),
								..Default::default()
							},
							DocumentNode {
								manual_composition: Some(concrete!(())),
								inputs: vec![NodeInput::node(NodeId(1), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::MemoNode<_, _>")),
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
			properties: node_properties::node_no_properties,
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
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<_, &WgpuExecutor>")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::network(concrete!(Vec<u8>), 0), NodeInput::node(NodeId(0), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::StorageNode<_>")),
								..Default::default()
							},
							DocumentNode {
								manual_composition: Some(concrete!(())),
								inputs: vec![NodeInput::node(NodeId(1), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::MemoNode<_, _>")),
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
			properties: node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "CreateOutputBuffer",
			category: "Debug: GPU",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(2), 0)],
						nodes: [
							DocumentNode {
								inputs: vec![NodeInput::scope("editor-api")],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<_, &WgpuExecutor>")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::network(concrete!(usize), 0), NodeInput::node(NodeId(0), 0), NodeInput::network(concrete!(Type), 1)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::CreateOutputBufferNode<_, _>")),
								..Default::default()
							},
							DocumentNode {
								manual_composition: Some(concrete!(())),
								inputs: vec![NodeInput::node(NodeId(1), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::MemoNode<_, _>")),
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
					output_names: vec!["OutputBuffer".to_string()],
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
			properties: node_properties::node_no_properties,
		},
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			identifier: "CreateComputePass",
			category: "Debug: GPU",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(2), 0)],
						nodes: [
							DocumentNode {
								inputs: vec![NodeInput::scope("editor-api")],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<_, &WgpuExecutor>")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![
									NodeInput::network(concrete!(PipelineLayout), 0),
									NodeInput::node(NodeId(0), 0),
									NodeInput::network(concrete!(WgpuShaderInput), 2),
									NodeInput::network(concrete!(gpu_executor::ComputePassDimensions), 3),
								],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::CreateComputePassNode<_, _, _>")),
								..Default::default()
							},
							DocumentNode {
								manual_composition: Some(concrete!(())),
								inputs: vec![NodeInput::node(NodeId(1), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::MemoNode<_, _>")),
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
					output_names: vec!["CommandBuffer".to_string()],
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
			properties: node_properties::node_no_properties,
		},
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			identifier: "CreatePipelineLayout",
			category: "Debug: GPU",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("wgpu_executor::CreatePipelineLayoutNode<_, _, _>"),
					inputs: vec![
						NodeInput::network(concrete!(ShaderHandle), 0),
						NodeInput::network(concrete!(String), 1),
						NodeInput::network(concrete!(Bindgroup), 2),
						NodeInput::network(concrete!(Arc<WgpuShaderInput>), 3),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["ShaderHandle".to_string(), "String".to_string(), "Bindgroup".to_string(), "ArcShaderInput".to_string()],
					output_names: vec!["PipelineLayout".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::node_no_properties,
		},
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			identifier: "ExecuteComputePipeline",
			category: "Debug: GPU",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(2), 0)],
						nodes: [
							DocumentNode {
								inputs: vec![NodeInput::scope("editor-api")],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<_, &WgpuExecutor>")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::network(concrete!(CommandBuffer), 0), NodeInput::node(NodeId(0), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::ExecuteComputePipelineNode<_>")),
								..Default::default()
							},
							DocumentNode {
								manual_composition: Some(concrete!(())),
								inputs: vec![NodeInput::node(NodeId(1), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::MemoNode<_, _>")),
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
					output_names: vec!["PipelineResult".to_string()],
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
			properties: node_properties::node_no_properties,
		},
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			identifier: "ReadOutputBuffer",
			category: "Debug: GPU",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(2), 0)],
						nodes: [
							DocumentNode {
								inputs: vec![NodeInput::scope("editor-api")],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<_, &WgpuExecutor>")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::network(concrete!(Arc<WgpuShaderInput>), 0), NodeInput::node(NodeId(0), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::ReadOutputBufferNode<_, _>")),
								..Default::default()
							},
							DocumentNode {
								manual_composition: Some(concrete!(())),
								inputs: vec![NodeInput::node(NodeId(1), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::MemoNode<_, _>")),
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
			properties: node_properties::node_no_properties,
		},
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			identifier: "CreateGpuSurface",
			category: "Debug: GPU",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(1), 0)],
						nodes: [
							DocumentNode {
								manual_composition: Some(concrete!(Footprint)),
								inputs: vec![NodeInput::scope("editor-api")],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::CreateGpuSurfaceNode<_>")),
								..Default::default()
							},
							DocumentNode {
								manual_composition: Some(concrete!(Footprint)),
								inputs: vec![NodeInput::node(NodeId(0), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::ImpureMemoNode<_, _, _>")),
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
					output_names: vec!["GpuSurface".to_string()],
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Create Gpu Surface".to_string(),
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
			properties: node_properties::node_no_properties,
		},
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			identifier: "RenderTexture",
			category: "Debug: GPU",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(1), 0)],
						nodes: [
							DocumentNode {
								inputs: vec![NodeInput::scope("editor-api")],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<_, &WgpuExecutor>")),
								..Default::default()
							},
							DocumentNode {
								manual_composition: Some(concrete!(Footprint)),
								inputs: vec![
									NodeInput::network(concrete!(ShaderInputFrame), 0),
									NodeInput::network(concrete!(Arc<wgpu_executor::Surface>), 1),
									NodeInput::node(NodeId(0), 0),
								],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::RenderTextureNode<_, _, _>")),
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
					output_names: vec!["RenderedTexture".to_string()],
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
			properties: node_properties::node_no_properties,
		},
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			identifier: "UploadTexture",
			category: "Debug: GPU",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(2), 0)],
						nodes: [
							DocumentNode {
								inputs: vec![NodeInput::scope("editor-api")],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<_, &WgpuExecutor>")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::network(concrete!(ImageFrame<Color>), 0), NodeInput::node(NodeId(0), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::UploadTextureNode<_>")),
								..Default::default()
							},
							DocumentNode {
								manual_composition: Some(concrete!(Footprint)),
								inputs: vec![NodeInput::node(NodeId(1), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::ImpureMemoNode<_, _, _>")),
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
			properties: node_properties::node_no_properties,
		},
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			identifier: "GpuImage",
			category: "Debug: GPU",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_std::executor::MapGpuSingleImageNode<_>"),
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
			properties: node_properties::node_no_properties,
		},
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			identifier: "Blend (GPU)",
			category: "Debug: GPU",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_std::executor::BlendGpuImageNode<_, _, _>"),
					inputs: vec![
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::BlendMode(BlendMode::Normal), false),
						NodeInput::value(TaggedValue::F64(100.), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string(), "Second".to_string(), "Blend Mode".to_string(), "Opacity".to_string()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::blend_properties,
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
					output_names: vec!["DocumentNode".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::node_no_properties,
		},
		#[cfg(feature = "quantization")]
		DocumentNodeDefinition {
			identifier: "Generate Quantization",
			category: "Debug: Quantization",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_std::quantization::GenerateQuantizationNode<_, _>"),
					inputs: vec![
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::U32(100), false),
						NodeInput::value(TaggedValue::U32(0), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string(), "Samples".to_string(), "Fn index".to_string()],
					output_names: vec!["Quantization".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::quantize_properties,
		},
		#[cfg(feature = "quantization")]
		DocumentNodeDefinition {
			identifier: "Quantize Image",
			category: "Debug: Quantization",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::quantization::QuantizeNode<_>"),
					inputs: vec![
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::Quantization(core::array::from_fn(|_| Default::default())), true),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string(), "Quantization".to_string()],
					output_names: vec!["Encoded".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::quantize_properties,
		},
		#[cfg(feature = "quantization")]
		DocumentNodeDefinition {
			identifier: "DeQuantize Image",
			category: "Debug: Quantization",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::quantization::DeQuantizeNode<_>"),
					inputs: vec![
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::Quantization(core::array::from_fn(|_| Default::default())), true),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Encoded".to_string(), "Quantization".to_string()],
					output_names: vec!["Decoded".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::quantize_properties,
		},
		DocumentNodeDefinition {
			identifier: "Invert",
			category: "Raster: Adjustment",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::raster::InvertNode"),
					inputs: vec![NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "Hue/Saturation",
			category: "Raster: Adjustment",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::raster::HueSaturationNode<_, _, _>"),
					inputs: vec![
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string(), "Hue Shift".to_string(), "Saturation Shift".to_string(), "Lightness Shift".to_string()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::hue_saturation_properties,
		},
		DocumentNodeDefinition {
			identifier: "Brightness/Contrast",
			category: "Raster: Adjustment",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::raster::BrightnessContrastNode<_, _, _>"),
					inputs: vec![
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::Bool(false), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string(), "Brightness".to_string(), "Contrast".to_string(), "Use Legacy".to_string()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::brightness_contrast_properties,
		},
		DocumentNodeDefinition {
			identifier: "Curves",
			category: "Raster: Adjustment",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::raster::CurvesNode<_>"),
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
			properties: node_properties::curves_properties,
		},
		DocumentNodeDefinition {
			identifier: "Threshold",
			category: "Raster: Adjustment",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::raster::ThresholdNode<_, _, _>"),
					inputs: vec![
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::F64(50.), false),
						NodeInput::value(TaggedValue::F64(100.), false),
						NodeInput::value(TaggedValue::LuminanceCalculation(LuminanceCalculation::SRGB), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string(), "Min Luminance".to_string(), "Max Luminance".to_string(), "Luminance Calc".to_string()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::threshold_properties,
		},
		DocumentNodeDefinition {
			identifier: "Gradient Map",
			category: "Raster: Adjustment",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::raster::GradientMapNode<_, _>"),
					inputs: vec![
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::GradientStops(vector::style::GradientStops::default()), false),
						NodeInput::value(TaggedValue::Bool(false), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string(), "Gradient".to_string()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::gradient_map_properties,
		},
		DocumentNodeDefinition {
			identifier: "Vibrance",
			category: "Raster: Adjustment",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::raster::VibranceNode<_>"),
					inputs: vec![NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true), NodeInput::value(TaggedValue::F64(0.), false)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string(), "Vibrance".to_string()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::vibrance_properties,
		},
		DocumentNodeDefinition {
			identifier: "Channel Mixer",
			category: "Raster: Adjustment",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::raster::ChannelMixerNode<_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _>"),
					inputs: vec![
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						// Monochrome toggle
						NodeInput::value(TaggedValue::Bool(false), false),
						// Monochrome
						NodeInput::value(TaggedValue::F64(40.), false),
						NodeInput::value(TaggedValue::F64(40.), false),
						NodeInput::value(TaggedValue::F64(20.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(100.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(100.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(100.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						// Display-only properties (not used within the node)
						NodeInput::value(TaggedValue::RedGreenBlue(RedGreenBlue::default()), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec![
						"Image".to_string(),
						// Monochrome toggle
						"Monochrome".to_string(),
						// Monochrome
						"Red".to_string(),
						"Green".to_string(),
						"Blue".to_string(),
						"Constant".to_string(),
						"(Red) Red".to_string(),
						"(Red) Green".to_string(),
						"(Red) Blue".to_string(),
						"(Red) Constant".to_string(),
						"(Green) Red".to_string(),
						"(Green) Green".to_string(),
						"(Green) Blue".to_string(),
						"(Green) Constant".to_string(),
						"(Blue) Red".to_string(),
						"(Blue) Green".to_string(),
						"(Blue) Blue".to_string(),
						"(Blue) Constant".to_string(),
						// Display-only properties (not used within the node)
						"Output Channel".to_string(),
					],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::channel_mixer_properties,
		},
		DocumentNodeDefinition {
			identifier: "Selective Color",
			category: "Raster: Adjustment",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto(
						"graphene_core::raster::SelectiveColorNode<_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _>",
					),
					inputs: vec![
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						// Mode
						NodeInput::value(TaggedValue::RelativeAbsolute(RelativeAbsolute::default()), false),
						// Reds
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						// Yellows
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						// Greens
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						// Cyans
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						// Blues
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						// Magentas
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						// Whites
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						// Neutrals
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						// Blacks
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						// Display-only properties (not used within the node)
						NodeInput::value(TaggedValue::SelectiveColorChoice(SelectiveColorChoice::default()), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec![
						"Image".to_string(),
						"Mode".to_string(),
						"(Reds) Cyan".to_string(),
						"(Reds) Magenta".to_string(),
						"(Reds) Yellow".to_string(),
						"(Reds) Black".to_string(),
						"(Yellows) Cyan".to_string(),
						"(Yellows) Magenta".to_string(),
						"(Yellows) Yellow".to_string(),
						"(Yellows) Black".to_string(),
						"(Greens) Cyan".to_string(),
						"(Greens) Magenta".to_string(),
						"(Greens) Yellow".to_string(),
						"(Greens) Black".to_string(),
						"(Cyans) Cyan".to_string(),
						"(Cyans) Magenta".to_string(),
						"(Cyans) Yellow".to_string(),
						"(Cyans) Black".to_string(),
						"(Blues) Cyan".to_string(),
						"(Blues) Magenta".to_string(),
						"(Blues) Yellow".to_string(),
						"(Blues) Black".to_string(),
						"(Magentas) Cyan".to_string(),
						"(Magentas) Magenta".to_string(),
						"(Magentas) Yellow".to_string(),
						"(Magentas) Black".to_string(),
						"(Whites) Cyan".to_string(),
						"(Whites) Magenta".to_string(),
						"(Whites) Yellow".to_string(),
						"(Whites) Black".to_string(),
						"(Neutrals) Cyan".to_string(),
						"(Neutrals) Magenta".to_string(),
						"(Neutrals) Yellow".to_string(),
						"(Neutrals) Black".to_string(),
						"(Blacks) Cyan".to_string(),
						"(Blacks) Magenta".to_string(),
						"(Blacks) Yellow".to_string(),
						"(Blacks) Black".to_string(),
						"Colors".to_string(),
					],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::selective_color_properties,
		},
		DocumentNodeDefinition {
			identifier: "Opacity",
			category: "Style",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::raster::OpacityNode<_>"),
					inputs: vec![NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true), NodeInput::value(TaggedValue::F64(100.), false)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string(), "Factor".to_string()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::opacity_properties,
		},
		DocumentNodeDefinition {
			identifier: "Blend Mode",
			category: "Style",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::raster::BlendModeNode<_>"),
					inputs: vec![
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::BlendMode(BlendMode::Normal), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string(), "Blend Mode".to_string()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::blend_mode_properties,
		},
		DocumentNodeDefinition {
			identifier: "Posterize",
			category: "Raster: Adjustment",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::raster::PosterizeNode<_>"),
					inputs: vec![NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true), NodeInput::value(TaggedValue::F64(4.), false)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string(), "Levels".to_string()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::posterize_properties,
		},
		DocumentNodeDefinition {
			identifier: "Exposure",
			category: "Raster: Adjustment",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::raster::ExposureNode<_, _, _>"),
					inputs: vec![
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(1.), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string(), "Exposure".to_string(), "Offset".to_string(), "Gamma Correction".to_string()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::exposure_properties,
		},
		DocumentNodeDefinition {
			identifier: "Add",
			category: "Math",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::ops::AddNode<_>"),
					inputs: vec![NodeInput::value(TaggedValue::F64(0.), true), NodeInput::value(TaggedValue::F64(0.), false)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Primary".to_string(), "Addend".to_string()],
					output_names: vec!["Output".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::add_properties,
		},
		DocumentNodeDefinition {
			identifier: "Subtract",
			category: "Math",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::ops::SubtractNode<_>"),
					inputs: vec![NodeInput::value(TaggedValue::F64(0.), true), NodeInput::value(TaggedValue::F64(0.), false)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Primary".to_string(), "Subtrahend".to_string()],
					output_names: vec!["Output".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::subtract_properties,
		},
		DocumentNodeDefinition {
			identifier: "Divide",
			category: "Math",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::ops::DivideNode<_>"),
					inputs: vec![NodeInput::value(TaggedValue::F64(0.), true), NodeInput::value(TaggedValue::F64(1.), false)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Primary".to_string(), "Divisor".to_string()],
					output_names: vec!["Output".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::divide_properties,
		},
		DocumentNodeDefinition {
			identifier: "Multiply",
			category: "Math",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::ops::MultiplyNode<_>"),
					inputs: vec![NodeInput::value(TaggedValue::F64(0.), true), NodeInput::value(TaggedValue::F64(1.), false)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Primary".to_string(), "Multiplicand".to_string()],
					output_names: vec!["Output".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::multiply_properties,
		},
		DocumentNodeDefinition {
			identifier: "Exponent",
			category: "Math",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::ops::ExponentNode<_>"),
					inputs: vec![NodeInput::value(TaggedValue::F64(0.), true), NodeInput::value(TaggedValue::F64(2.), false)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Primary".to_string(), "Power".to_string()],
					output_names: vec!["Output".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::exponent_properties,
		},
		DocumentNodeDefinition {
			identifier: "Floor",
			category: "Math",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::ops::FloorNode"),
					inputs: vec![NodeInput::value(TaggedValue::F64(0.), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Primary".to_string()],
					output_names: vec!["Output".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "Ceil",
			category: "Math",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::ops::CeilingNode"),
					inputs: vec![NodeInput::value(TaggedValue::F64(0.), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Primary".to_string()],
					output_names: vec!["Output".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "Round",
			category: "Math",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::ops::RoundNode"),
					inputs: vec![NodeInput::value(TaggedValue::F64(0.), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Primary".to_string()],
					output_names: vec!["Output".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "Absolute Value",
			category: "Math",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::ops::AbsoluteValue"),
					inputs: vec![NodeInput::value(TaggedValue::F64(0.), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Primary".to_string()],
					output_names: vec!["Output".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "Logarithm",
			category: "Math",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::ops::LogarithmNode<_>"),
					inputs: vec![NodeInput::value(TaggedValue::F64(0.), true), NodeInput::value(TaggedValue::F64(0.), false)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Primary".to_string(), "Base".to_string()],
					output_names: vec!["Output".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::log_properties,
		},
		DocumentNodeDefinition {
			identifier: "Natural Logarithm",
			category: "Math",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::ops::NaturalLogarithmNode"),
					inputs: vec![NodeInput::value(TaggedValue::F64(0.), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Primary".to_string()],
					output_names: vec!["Output".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "Sine",
			category: "Math",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::ops::SineNode"),
					inputs: vec![NodeInput::value(TaggedValue::F64(0.), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Primary".to_string()],
					output_names: vec!["Output".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "Cosine",
			category: "Math",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::ops::CosineNode"),
					inputs: vec![NodeInput::value(TaggedValue::F64(0.), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Primary".to_string()],
					output_names: vec!["Output".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "Tangent",
			category: "Math",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::ops::TangentNode"),
					inputs: vec![NodeInput::value(TaggedValue::F64(0.), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Primary".to_string()],
					output_names: vec!["Output".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "Max",
			category: "Math",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::ops::MaximumNode<_>"),
					inputs: vec![NodeInput::value(TaggedValue::F64(0.), true), NodeInput::value(TaggedValue::F64(0.), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Operand A".to_string(), "Operand B".to_string()],
					output_names: vec!["Output".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::max_properties,
		},
		DocumentNodeDefinition {
			identifier: "Min",
			category: "Math",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::ops::MinimumNode<_>"),
					inputs: vec![NodeInput::value(TaggedValue::F64(0.), true), NodeInput::value(TaggedValue::F64(0.), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Operand A".to_string(), "Operand B".to_string()],
					output_names: vec!["Output".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::min_properties,
		},
		DocumentNodeDefinition {
			identifier: "Equals",
			category: "Math",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::ops::EqualsNode<_>"),
					inputs: vec![NodeInput::value(TaggedValue::F64(0.), true), NodeInput::value(TaggedValue::F64(0.), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Operand A".to_string(), "Operand B".to_string()],
					output_names: vec!["Output".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::eq_properties,
		},
		DocumentNodeDefinition {
			identifier: "Modulo",
			category: "Math",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::ops::ModuloNode<_>"),
					inputs: vec![NodeInput::value(TaggedValue::F64(0.), true), NodeInput::value(TaggedValue::F64(0.), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Primary".to_string(), "Modulus".to_string()],
					output_names: vec!["Output".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::modulo_properties,
		},
		DocumentNodeDefinition {
			identifier: "Log to Console",
			category: "Debug",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::logic::LogToConsoleNode"),
					inputs: vec![NodeInput::value(TaggedValue::String("Not Connected to a value yet".into()), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Input".to_string()],
					output_names: vec!["Output".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "Or",
			category: "Math",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::logic::LogicOrNode<_>"),
					inputs: vec![NodeInput::value(TaggedValue::Bool(false), true), NodeInput::value(TaggedValue::Bool(false), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Operand A".to_string(), "Operand B".to_string()],
					output_names: vec!["Output".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::logic_operator_properties,
		},
		DocumentNodeDefinition {
			identifier: "And",
			category: "Math",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::logic::LogicAndNode<_>"),
					inputs: vec![NodeInput::value(TaggedValue::Bool(false), true), NodeInput::value(TaggedValue::Bool(false), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Operand A".to_string(), "Operand B".to_string()],
					output_names: vec!["Output".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::logic_operator_properties,
		},
		DocumentNodeDefinition {
			identifier: "XOR",
			category: "Math",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::logic::LogicXorNode<_>"),
					inputs: vec![NodeInput::value(TaggedValue::Bool(false), true), NodeInput::value(TaggedValue::Bool(false), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Operand A".to_string(), "Operand B".to_string()],
					output_names: vec!["Output".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::logic_operator_properties,
		},
		DocumentNodeDefinition {
			identifier: "Not",
			category: "Math",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::logic::LogicNotNode"),
					inputs: vec![NodeInput::value(TaggedValue::Bool(false), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Input".to_string()],
					output_names: vec!["Output".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::node_no_properties,
		},
		(*IMAGINATE_NODE).clone(),
		DocumentNodeDefinition {
			identifier: "Circle",
			category: "Vector: Generator",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(1), 0)],
						nodes: vec![
							DocumentNode {
								inputs: vec![NodeInput::network(concrete!(()), 0), NodeInput::network(concrete!(f64), 1)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::vector::generator_nodes::CircleGenerator<_>")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(0), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::transform::CullNode<_>")),
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
					inputs: vec![NodeInput::value(TaggedValue::None, false), NodeInput::value(TaggedValue::F64(50.), false)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["None".to_string(), "Radius".to_string()],
					output_names: vec!["Vector".to_string()],
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [
								DocumentNodeMetadata {
									persistent_metadata: DocumentNodePersistentMetadata {
										display_name: "Circle Generator".to_string(),
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
			properties: node_properties::circle_properties,
		},
		DocumentNodeDefinition {
			identifier: "Ellipse",
			category: "Vector: Generator",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::vector::generator_nodes::EllipseGenerator<_, _>"),
					inputs: vec![
						NodeInput::value(TaggedValue::None, false),
						NodeInput::value(TaggedValue::F64(50.), false),
						NodeInput::value(TaggedValue::F64(25.), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["None".to_string(), "Radius X".to_string(), "Radius Y".to_string()],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::ellipse_properties,
		},
		DocumentNodeDefinition {
			identifier: "Rectangle",
			category: "Vector: Generator",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::vector::generator_nodes::RectangleGenerator<_, _, _, _, _>"),
					inputs: vec![
						NodeInput::value(TaggedValue::None, false),
						NodeInput::value(TaggedValue::F64(100.), false),
						NodeInput::value(TaggedValue::F64(100.), false),
						NodeInput::value(TaggedValue::Bool(false), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::Bool(true), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec![
						"None".to_string(),
						"Size X".to_string(),
						"Size Y".to_string(),
						"Individual Corner Radii".to_string(),
						"Corner Radius".to_string(),
						"Clamped".to_string(),
					],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::rectangle_properties,
		},
		DocumentNodeDefinition {
			identifier: "Regular Polygon",
			category: "Vector: Generator",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::vector::generator_nodes::RegularPolygonGenerator<_, _>"),
					inputs: vec![
						NodeInput::value(TaggedValue::None, false),
						NodeInput::value(TaggedValue::U32(6), false),
						NodeInput::value(TaggedValue::F64(50.), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["None".to_string(), "Sides".to_string(), "Radius".to_string()],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::regular_polygon_properties,
		},
		DocumentNodeDefinition {
			identifier: "Star",
			category: "Vector: Generator",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::vector::generator_nodes::StarGenerator<_, _, _>"),
					inputs: vec![
						NodeInput::value(TaggedValue::None, false),
						NodeInput::value(TaggedValue::U32(5), false),
						NodeInput::value(TaggedValue::F64(50.), false),
						NodeInput::value(TaggedValue::F64(25.), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["None".to_string(), "Sides".to_string(), "Radius".to_string(), "Inner Radius".to_string()],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::star_properties,
		},
		DocumentNodeDefinition {
			identifier: "Line",
			category: "Vector: Generator",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::vector::generator_nodes::LineGenerator<_, _>"),
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
			properties: node_properties::line_properties,
		},
		DocumentNodeDefinition {
			identifier: "Spline",
			category: "Vector: Generator",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::vector::generator_nodes::SplineGenerator<_>"),
					inputs: vec![
						NodeInput::value(TaggedValue::None, false),
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
			properties: node_properties::spline_properties,
		},
		DocumentNodeDefinition {
			identifier: "Shape", // TODO: What is this and is it used? What is the difference between this and "Path"?
			category: "Ignore",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::vector::generator_nodes::PathGenerator<_>"),
					inputs: vec![NodeInput::value(TaggedValue::PointIds(vec![]), false), NodeInput::value(TaggedValue::PointIds(vec![]), false)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Path Data".to_string(), "Colinear Manipulators".to_string()],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::node_no_properties,
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
								implementation: DocumentNodeImplementation::proto("graphene_core::memo::MonitorNode<_, _, _>"),
								manual_composition: Some(generic!(T)),
								skip_deduplication: true,
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(0), 0), NodeInput::network(concrete!(graphene_core::vector::VectorModification), 1)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::vector::PathModify<_>")),
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
			properties: node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "Sample",
			category: "Debug",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_std::raster::SampleNode<_>"),
					inputs: vec![NodeInput::value(TaggedValue::None, false), NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true)],
					manual_composition: Some(concrete!(Footprint)),
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["None".to_string(), "Raster Data".to_string()],
					output_names: vec!["Raster".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "Mandelbrot",
			category: "Raster: Generator",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_std::raster::MandelbrotNode"),
					inputs: vec![],
					manual_composition: Some(concrete!(Footprint)),
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec![],
					output_names: vec!["Raster".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "Cull",
			category: "Debug",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::transform::CullNode<_>"),
					inputs: vec![NodeInput::value(TaggedValue::VectorData(VectorData::empty()), true)],
					manual_composition: Some(concrete!(Footprint)),
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Vector Data".to_string()],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "Text",
			category: "Vector",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::text::TextGeneratorNode<_, _, _>"),
					inputs: vec![
						NodeInput::scope("editor-api"),
						NodeInput::value(TaggedValue::String("Lorem ipsum".to_string()), false),
						NodeInput::value(
							TaggedValue::Font(Font::new(graphene_core::consts::DEFAULT_FONT_FAMILY.into(), graphene_core::consts::DEFAULT_FONT_STYLE.into())),
							false,
						),
						NodeInput::value(TaggedValue::F64(24.), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Editor API".to_string(), "Text".to_string(), "Font".to_string(), "Size".to_string()],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::text_properties,
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
								implementation: DocumentNodeImplementation::proto("graphene_core::memo::MonitorNode<_, _, _>"),
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
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::transform::TransformNode<_, _, _, _, _, _>")),
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
			properties: node_properties::transform_properties,
		},
		DocumentNodeDefinition {
			identifier: "Set Transform",
			category: "General",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::transform::SetTransformNode<_>"),
					inputs: vec![
						NodeInput::value(TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
						NodeInput::value(TaggedValue::DAffine2(DAffine2::IDENTITY), true),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Data".to_string(), "Transform".to_string()],
					output_names: vec!["Data".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "Assign Colors",
			category: "Vector: Style",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::vector::AssignColorsNode<_, _, _, _, _, _, _>"),
					inputs: vec![
						NodeInput::value(TaggedValue::GraphicGroup(graphene_core::GraphicGroup::default()), true),
						NodeInput::value(TaggedValue::Bool(true), false),
						NodeInput::value(TaggedValue::Bool(false), false),
						NodeInput::value(TaggedValue::GradientStops(vector::style::GradientStops::default()), false),
						NodeInput::value(TaggedValue::Bool(false), false),
						NodeInput::value(TaggedValue::Bool(false), false),
						NodeInput::value(TaggedValue::U32(0), false),
						NodeInput::value(TaggedValue::U32(0), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec![
						"Vector Group".to_string(),
						"Fill".to_string(),
						"Stroke".to_string(),
						"Gradient".to_string(),
						"Reverse".to_string(),
						"Randomize".to_string(),
						"Seed".to_string(),
						"Repeat Every".to_string(),
					],
					output_names: vec!["Vector Group".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::assign_colors_properties,
		},
		DocumentNodeDefinition {
			identifier: "Fill",
			category: "Vector: Style",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::Network(NodeNetwork {
						exports: vec![NodeInput::node(NodeId(0), 0)],
						nodes: vec![DocumentNode {
							inputs: vec![NodeInput::network(concrete!(VectorData), 0), NodeInput::network(concrete!(vector::style::Fill), 1)],
							implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::vector::SetFillNode<_>")),
							..Default::default()
						}]
						.into_iter()
						.enumerate()
						.map(|(id, node)| (NodeId(id as u64), node))
						.collect(),
						..Default::default()
					}),
					inputs: vec![
						NodeInput::value(TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
						NodeInput::value(TaggedValue::Fill(vector::style::Fill::Solid(Color::BLACK)), false),
						NodeInput::value(TaggedValue::OptionalColor(Some(Color::BLACK)), false),
						NodeInput::value(TaggedValue::Gradient(Default::default()), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					network_metadata: Some(NodeNetworkMetadata {
						persistent_metadata: NodeNetworkPersistentMetadata {
							node_metadata: [DocumentNodeMetadata {
								persistent_metadata: DocumentNodePersistentMetadata {
									display_name: "Set Fill".to_string(),
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
					input_names: vec![
						"Vector Data".to_string(),
						"Fill".to_string(),
						// These backup values aren't exposed to the user, but are used to store the previous fill choices so the user can flip back from Solid to Gradient (or vice versa) without losing their settings
						"Backup Color".to_string(),
						"Backup Gradient".to_string(),
					],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::fill_properties,
		},
		DocumentNodeDefinition {
			identifier: "Stroke",
			category: "Vector: Style",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::vector::SetStrokeNode<_, _, _, _, _, _, _>"),
					inputs: vec![
						NodeInput::value(TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
						NodeInput::value(TaggedValue::OptionalColor(Some(Color::BLACK)), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::VecF64(Vec::new()), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::LineCap(graphene_core::vector::style::LineCap::default()), false),
						NodeInput::value(TaggedValue::LineJoin(graphene_core::vector::style::LineJoin::default()), false),
						NodeInput::value(TaggedValue::F64(4.), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec![
						"Vector Data".to_string(),
						"Color".to_string(),
						"Weight".to_string(),
						"Dash Lengths".to_string(),
						"Dash Offset".to_string(),
						"Line Cap".to_string(),
						"Line Join".to_string(),
						"Miter Limit".to_string(),
					],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::stroke_properties,
		},
		DocumentNodeDefinition {
			identifier: "Bounding Box",
			category: "Vector",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::vector::BoundingBoxNode"),
					inputs: vec![NodeInput::value(TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Vector Data".to_string()],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "Solidify Stroke",
			category: "Vector",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::vector::SolidifyStrokeNode"),
					inputs: vec![NodeInput::value(TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Vector Data".to_string()],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "Repeat",
			category: "Vector",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::vector::RepeatNode<_, _, _>"),
					inputs: vec![
						NodeInput::value(TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
						NodeInput::value(TaggedValue::DVec2((100., 100.).into()), false),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::U32(5), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Instance".to_string(), "Direction".to_string(), "Angle".to_string(), "Instances".to_string()],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::repeat_properties,
		},
		DocumentNodeDefinition {
			identifier: "Circular Repeat",
			category: "Vector",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::vector::CircularRepeatNode<_, _, _>"),
					inputs: vec![
						NodeInput::value(TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
						NodeInput::value(TaggedValue::F64(0.), false),
						NodeInput::value(TaggedValue::F64(5.), false),
						NodeInput::value(TaggedValue::U32(5), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Instance".to_string(), "Angle Offset".to_string(), "Radius".to_string(), "Instances".to_string()],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::circular_repeat_properties,
		},
		DocumentNodeDefinition {
			identifier: "Boolean Operation",
			category: "Vector",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					inputs: vec![
						NodeInput::value(TaggedValue::GraphicGroup(GraphicGroup::EMPTY), true),
						NodeInput::value(TaggedValue::BooleanOperation(vector::misc::BooleanOperation::Union), false),
					],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::vector::BooleanOperationNode<_>")),
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Group of Paths".to_string(), "Operation".to_string()],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::boolean_operation_properties,
		},
		DocumentNodeDefinition {
			identifier: "Copy to Points",
			category: "Vector",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					// TODO: Wrap this implementation with a document node that has a cache node so the output is cached?
					implementation: DocumentNodeImplementation::proto("graphene_core::vector::CopyToPoints<_, _, _, _, _, _, _, _>"),
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
			properties: node_properties::copy_to_points_properties,
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
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::vector::LengthsOfSegmentsOfSubpaths")),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![
									NodeInput::network(concrete!(graphene_core::vector::VectorData), 0),
									NodeInput::network(concrete!(f64), 1),  // From the document node's parameters
									NodeInput::network(concrete!(f64), 2),  // From the document node's parameters
									NodeInput::network(concrete!(f64), 3),  // From the document node's parameters
									NodeInput::network(concrete!(bool), 4), // From the document node's parameters
									NodeInput::node(NodeId(0), 0),          // From output 0 of Lengths of Segments of Subpaths
								],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::vector::SamplePoints<_, _, _, _, _, _>")),
								manual_composition: Some(concrete!(Footprint)),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(1), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::ImpureMemoNode<_, _, _>")),
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
										display_name: "Lengths of Segments of Subpaths".to_string(),
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
										display_name: "MemoizeImpure".to_string(),
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
			properties: node_properties::sample_points_properties,
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
								implementation: DocumentNodeImplementation::proto("graphene_core::vector::PoissonDiskPoints<_, _>"),
								..Default::default()
							},
							DocumentNode {
								inputs: vec![NodeInput::node(NodeId(0), 0)],
								implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::ImpureMemoNode<_, _, _>")),
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
										display_name: "MemoizeImpure".to_string(),
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
			properties: node_properties::poisson_disk_points_properties,
		},
		DocumentNodeDefinition {
			identifier: "Splines from Points",
			category: "Vector",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::vector::SplinesFromPointsNode"),
					inputs: vec![NodeInput::value(TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Vector Data".to_string()],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "Area",
			category: "Vector",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::vector::AreaNode<_>"),
					inputs: vec![NodeInput::value(TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true)],
					manual_composition: Some(concrete!(())),
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Vector Data".to_string()],
					output_names: vec!["Output".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "Centroid",
			category: "Vector",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::vector::CentroidNode<_, _>"),
					inputs: vec![
						NodeInput::value(TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
						NodeInput::value(TaggedValue::CentroidType(graphene_core::vector::misc::CentroidType::Area), false),
					],
					manual_composition: Some(concrete!(())),
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Vector Data".to_string(), "Centroid Type".to_string()],
					output_names: vec!["Output".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::centroid_properties,
		},
		DocumentNodeDefinition {
			identifier: "Morph",
			category: "Vector",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::vector::MorphNode<_, _, _, _>"),
					inputs: vec![
						NodeInput::value(TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
						NodeInput::value(TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
						NodeInput::value(TaggedValue::U32(0), false),
						NodeInput::value(TaggedValue::F64(0.5), false),
					],
					manual_composition: Some(concrete!(Footprint)),
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Source".to_string(), "Target".to_string(), "Start Index".to_string(), "Time".to_string()],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::morph_properties,
		},
		// TODO: This needs to work with resolution-aware (raster with footprint, post-Cull node) data.
		DocumentNodeDefinition {
			identifier: "Image Segmentation",
			category: "Ignore",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_std::image_segmentation::ImageSegmentationNode<_>"),
					inputs: vec![
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string(), "Mask".to_string()],
					output_names: vec!["Segments".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::node_no_properties,
		},
		DocumentNodeDefinition {
			identifier: "Index",
			category: "Debug",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::raster::IndexNode<_>"),
					inputs: vec![NodeInput::value(TaggedValue::Segments(vec![ImageFrame::empty()]), true), NodeInput::value(TaggedValue::U32(0), false)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Segmentation".to_string(), "Index".to_string()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::index_properties,
		},
		// Applies the given color to each pixel of an image but maintains the alpha value
		DocumentNodeDefinition {
			identifier: "Color Fill",
			category: "Raster",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::raster::adjustments::ColorFillNode<_>"),
					inputs: vec![
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::Color(Color::BLACK), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string(), "Color".to_string()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::color_fill_properties,
		},
		DocumentNodeDefinition {
			identifier: "Color Overlay",
			category: "Raster",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_core::raster::adjustments::ColorOverlayNode<_, _, _>"),
					inputs: vec![
						NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
						NodeInput::value(TaggedValue::Color(Color::BLACK), false),
						NodeInput::value(TaggedValue::BlendMode(BlendMode::Normal), false),
						NodeInput::value(TaggedValue::F64(100.), false),
					],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string(), "Color".to_string(), "Blend Mode".to_string(), "Opacity".to_string()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::color_overlay_properties,
		},
		DocumentNodeDefinition {
			identifier: "Image Color Palette",
			category: "Raster",
			node_template: NodeTemplate {
				document_node: DocumentNode {
					implementation: DocumentNodeImplementation::proto("graphene_std::image_color_palette::ImageColorPaletteNode<_>"),
					inputs: vec![NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true), NodeInput::value(TaggedValue::U32(8), true)],
					..Default::default()
				},
				persistent_node_metadata: DocumentNodePersistentMetadata {
					input_names: vec!["Image".to_string(), "Max Size".to_string()],
					output_names: vec!["Colors".to_string()],
					..Default::default()
				},
			},
			properties: node_properties::image_color_palette,
		},
	]
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
						implementation: DocumentNodeImplementation::proto("graphene_core::memo::MonitorNode<_, _, _>"),
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
						implementation: DocumentNodeImplementation::proto("graphene_std::raster::ImaginateNode<_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _>"),
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
	properties: node_properties::imaginate_properties,
});

pub fn resolve_document_node_type(identifier: &str) -> Option<&DocumentNodeDefinition> {
	DOCUMENT_NODE_TYPES.iter().find(|definition| definition.identifier == identifier)
}

pub fn collect_node_types() -> Vec<FrontendNodeType> {
	DOCUMENT_NODE_TYPES
		.iter()
		.filter(|definition| !definition.category.eq_ignore_ascii_case("ignore"))
		.map(|definition| FrontendNodeType::new(definition.identifier, definition.category))
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

pub fn wrap_network_in_scope(mut network: NodeNetwork, editor_api: Arc<WasmEditorApi>) -> NodeNetwork {
	network.generate_node_paths(&[]);

	let inner_network = DocumentNode {
		implementation: DocumentNodeImplementation::Network(network),
		inputs: vec![],
		..Default::default()
	};

	// TODO: Replace with "Output" definition?
	// let render_node = resolve_document_node_type("Output")
	// 	.expect("Output node type not found")
	// 	.node_template_input_override(vec![Some(NodeInput::node(NodeId(1), 0)), Some(NodeInput::node(NodeId(0), 1))])
	// 	.document_node;

	let render_node = graph_craft::document::DocumentNode {
		inputs: vec![NodeInput::node(NodeId(0), 0), NodeInput::node(NodeId(2), 0)],
		implementation: graph_craft::document::DocumentNodeImplementation::Network(NodeNetwork {
			exports: vec![NodeInput::node(NodeId(2), 0)],
			nodes: [
				DocumentNode {
					inputs: vec![NodeInput::scope("editor-api")],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::CreateGpuSurfaceNode")),
					skip_deduplication: true,
					..Default::default()
				},
				DocumentNode {
					manual_composition: Some(concrete!(())),
					inputs: vec![NodeInput::node(NodeId(0), 0)],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::MemoNode<_, _>")),
					..Default::default()
				},
				// TODO: Add conversion step
				DocumentNode {
					manual_composition: Some(concrete!(RenderConfig)),
					inputs: vec![
						NodeInput::scope("editor-api"),
						NodeInput::network(graphene_core::Type::Fn(Box::new(concrete!(Footprint)), Box::new(generic!(T))), 0),
						NodeInput::node(NodeId(1), 0),
					],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::wasm_application_io::RenderNode<_, _, _>")),
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
	};

	// wrap the inner network in a scope
	let nodes = vec![
		inner_network,
		render_node,
		DocumentNode {
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::IdentityNode"),
			inputs: vec![NodeInput::value(TaggedValue::EditorApi(editor_api), false)],
			..Default::default()
		},
	];

	NodeNetwork {
		exports: vec![NodeInput::node(NodeId(1), 0)],
		nodes: nodes.into_iter().enumerate().map(|(id, node)| (NodeId(id as u64), node)).collect(),
		scope_injections: [("editor-api".to_string(), (NodeId(2), concrete!(&WasmEditorApi)))].into_iter().collect(),
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
