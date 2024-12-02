use super::node_properties;
use super::utility_types::FrontendNodeType;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::utility_types::network_interface::{
	DocumentNodeMetadata, DocumentNodePersistentMetadata, NodeNetworkInterface, NodeNetworkMetadata, NodeNetworkPersistentMetadata, NodeTemplate, NodeTypePersistentMetadata, PropertiesRow,
	WidgetOverride,
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
	pub network_interface: &'a mut NodeNetworkInterface,
	pub selection_network_path: &'a [NodeId],
	pub document_name: &'a str,
}

impl NodePropertiesContext<'_> {
	pub fn call_widget_override(&mut self, node_id: &NodeId, index: usize) -> Option<Vec<LayoutGroup>> {
		//let current_override = //Get mutable reference from transient metadata
		//let mut widget_override = std::mem::replace(&mut WidgetOverrideLambda(Box::new()), current_override);
		// let layout = widget_override.0(node_id, context);
		//let current_override = //Get mutable reference from transient metadata (now empty)
		//let empty_widget_override = std::mem::replace(&mut widget_override, current_override) // Put back the taken override
		// Some(layout)
		let Some(input_properties_row) = self.network_interface.input_properties_row(node_id, index, self.selection_network_path) else {
			log::error!("Could not get input properties row in call_widget_override");
			return None;
		};
		let Some(widget_override_lambda) = input_properties_row.widget_override.as_ref().and_then(|widget_override| INPUT_OVERRIDES.get(widget_override)) else {
			log::error!("Could not get widget override lambda in call_widget_override");
			return None;
		};
		widget_override_lambda(*node_id, index, self)
			.map_err(|error| {
				log::error!("Error in widget override lambda: {}", error);
			})
			.ok()
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
					input_properties: vec![PropertiesRow::with_override(
						"In",
						WidgetOverride::String("The identity node simply passes its data through.".to_string()),
					)],
					output_names: vec!["Out".to_string()],
					..Default::default()
				},
			},

			description: Cow::Borrowed("The identity node passes its data through. You can use this to organize your node graph."),
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
					input_properties: vec![PropertiesRow::with_override(
						"In",
						WidgetOverride::String("The Monitor node is used by the editor to access the data flowing through it.".to_string()),
					)],
					output_names: vec!["Out".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("The Monitor node is used by the editor to access the data flowing through it."),
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
					input_properties: vec![
						PropertiesRow::with_override("Artboards", WidgetOverride::Hidden),
						PropertiesRow::with_override("Contents", WidgetOverride::Hidden),
						"Location".into(),
						"Dimensions".into(),
						"Background".into(),
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
					input_properties: vec![PropertiesRow::with_override("api", WidgetOverride::Hidden), "path".into()],
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
					input_properties: vec![PropertiesRow::with_override(
						"Image",
						WidgetOverride::String("Creates an embedded image with the given transform.".to_string()),
					)],
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
					input_properties: vec![
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
					input_properties: vec![PropertiesRow::with_override("Image", WidgetOverride::Hidden), "Stencil".into()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
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
					input_properties: vec![
						PropertiesRow::with_override("Image", WidgetOverride::Hidden),
						PropertiesRow::with_override("Insertion", WidgetOverride::Hidden),
						"Replace".into(),
					],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
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
					input_properties: vec!["None".into(), "Red".into(), "Green".into(), "Blue".into(), "Alpha".into()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
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
					input_properties: vec!["Image".into()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
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
					input_properties: vec!["Image".into()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
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
					input_properties: vec![PropertiesRow::with_override("Image", WidgetOverride::String("A bitmap image is embedded in this node".to_string()))],
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
					input_properties: vec!["Image".into(), "Node".into()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
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
					input_properties: vec![
						PropertiesRow::with_override("Image", WidgetOverride::Hidden),
						PropertiesRow::with_override("Brightness", WidgetOverride::Custom("brightness".to_string())),
						PropertiesRow::with_override("Brightness", WidgetOverride::Custom("contrast".to_string())),
						"Use Classic".into(),
					],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
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
					input_properties: vec![PropertiesRow::with_override("Image", WidgetOverride::Hidden), "Curve".into()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
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
					input_properties: vec!["None".into(), "Start".into(), "End".into()],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
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
					input_properties: vec!["None".into(), "Points".into()],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
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
					input_properties: vec!["Editor API".into(), "Text".into(), "Font".into(), "Size".into(), "Line Height".into(), "Character Spacing".into()],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},
			description: Cow::Borrowed("TODO"),
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
					input_properties: vec!["Vector Data".into(), "Translation".into(), "Rotation".into(), "Scale".into(), "Skew".into(), "Pivot".into()],
					output_names: vec!["Data".to_string()],
					..Default::default()
				},
			},

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
					input_properties: vec!["Group of Paths".into(), "Operation".into()],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},

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
					input_properties: vec![
						"Points".into(),
						"Instance".into(),
						"Random Scale Min".into(),
						"Random Scale Max".into(),
						"Random Scale Bias".into(),
						"Random Scale Seed".into(),
						"Random Rotation".into(),
						"Random Rotation Seed".into(),
					],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},

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
					input_properties: vec!["Vector Data".into(), "Spacing".into(), "Start Offset".into(), "Stop Offset".into(), "Adaptive Spacing".into()],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},

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
					input_properties: vec!["Vector Data".into(), "Separation Disk Diameter".into(), "Seed".into()],
					output_names: vec!["Vector".to_string()],
					..Default::default()
				},
			},

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
					input_properties: vec!["Segmentation".into(), "Index".into()],
					output_names: vec!["Image".to_string()],
					..Default::default()
				},
			},

			description: Cow::Borrowed("TODO"),
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
					// TODO: Store information for input overrides in the node macro
					input_properties: fields.iter().map(|f| f.name.into()).collect(),
					output_names: vec![output_type.to_string()],
					has_primary_output: true,
					locked: false,
					..Default::default()
				},
			},
			category: category.unwrap_or("UNCATEGORIZED"),
			description: Cow::Borrowed(description),
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
});

static INPUT_OVERRIDES: once_cell::sync::Lazy<HashMap<String, Box<dyn Fn(NodeId, usize, &mut NodePropertiesContext) -> Result<Vec<LayoutGroup>, String> + Send + Sync>>> =
	once_cell::sync::Lazy::new(static_input_properties);

/// Defines the logic for inputs to display a custom properties panel widget.
fn static_input_properties() -> HashMap<String, Box<dyn Fn(NodeId, usize, &mut NodePropertiesContext) -> Result<Vec<LayoutGroup>, String> + Send + Sync>> {
	let mut map: HashMap<String, Box<dyn Fn(NodeId, usize, &mut NodePropertiesContext) -> Result<Vec<LayoutGroup>, String> + Send + Sync>> = HashMap::new();
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
			Ok(node_properties::string_properties(string.to_string()))
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
			let (_, coherent_noise_active, _, _, _, _) = node_properties::query_noise_pattern_state(node_id, context)?;
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
			Ok(vec![domain_warp_type.into()])
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
			Ok(vec![fractal_type_row.into()])
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
			Ok(vec![cellular_distance_function_row.into()])
		}),
	);
	map.insert(
		"noise_properties_cellular_return_type".to_string(),
		Box::new(|node_id, index, context| {
			let (document_node, input_name) = node_properties::query_node_and_input_name(node_id, index, context)?;
			let (_, coherent_noise_active, cellular_noise_active, _, _, _) = node_properties::query_noise_pattern_state(node_id, context)?;
			let cellular_return_type = node_properties::cellular_return_type(document_node, node_id, index, input_name, true, !coherent_noise_active || !cellular_noise_active);
			Ok(vec![cellular_return_type.into()])
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
	map
}

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
