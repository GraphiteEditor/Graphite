use super::node_properties;
use super::utility_types::{FrontendGraphDataType, FrontendNodeType};
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::utility_types::document_metadata::DocumentMetadata;
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
#[cfg(feature = "gpu")]
use wgpu_executor::{Bindgroup, CommandBuffer, PipelineLayout, ShaderHandle, ShaderInputFrame, WgpuShaderInput};

use once_cell::sync::Lazy;
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct DocumentInputType {
	pub name: &'static str,
	pub data_type: FrontendGraphDataType,
	pub default: NodeInput,
}

impl DocumentInputType {
	pub fn new(name: &'static str, data_type: FrontendGraphDataType, default: NodeInput) -> Self {
		Self { name, data_type, default }
	}

	pub fn value(name: &'static str, tagged_value: TaggedValue, exposed: bool) -> Self {
		let data_type = FrontendGraphDataType::with_type(&tagged_value.ty());
		let default = NodeInput::value(tagged_value, exposed);
		Self { name, data_type, default }
	}

	pub const fn none() -> Self {
		Self {
			name: "None",
			data_type: FrontendGraphDataType::General,
			default: NodeInput::value(TaggedValue::None, false),
		}
	}
}

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct DocumentOutputType {
	pub name: &'static str,
	pub data_type: FrontendGraphDataType,
}

impl DocumentOutputType {
	pub const fn new(name: &'static str, data_type: FrontendGraphDataType) -> Self {
		Self { name, data_type }
	}
}

pub struct NodePropertiesContext<'a> {
	pub persistent_data: &'a PersistentData,
	pub responses: &'a mut VecDeque<Message>,
	pub nested_path: &'a [NodeId],
	pub executor: &'a mut NodeGraphExecutor,
	pub document_network: &'a NodeNetwork,
	pub metadata: &'a mut DocumentMetadata,
}

/// Acts as a description for a [DocumentNode] before it gets instantiated as one.
#[derive(Clone)]
pub struct DocumentNodeDefinition {
	pub name: &'static str,
	pub category: &'static str,
	pub is_layer: bool,
	pub implementation: DocumentNodeImplementation,
	pub inputs: Vec<DocumentInputType>,
	pub outputs: Vec<DocumentOutputType>,
	pub has_primary_output: bool,
	pub properties: fn(&DocumentNode, NodeId, &mut NodePropertiesContext) -> Vec<LayoutGroup>,
	pub manual_composition: Option<graphene_core::Type>,
}

impl Default for DocumentNodeDefinition {
	fn default() -> Self {
		Self {
			name: Default::default(),
			category: Default::default(),
			is_layer: false,
			implementation: Default::default(),
			inputs: Default::default(),
			outputs: Default::default(),
			has_primary_output: true,
			properties: node_properties::node_no_properties,
			manual_composition: Default::default(),
		}
	}
}

// We use the once cell for lazy initialization to avoid the overhead of reconstructing the node list every time.
// TODO: make document nodes not require a `'static` lifetime to avoid having to split the construction into const and non-const parts.
static DOCUMENT_NODE_TYPES: once_cell::sync::Lazy<Vec<DocumentNodeDefinition>> = once_cell::sync::Lazy::new(static_nodes);

fn monitor_node() -> DocumentNode {
	DocumentNode {
		name: "Monitor".to_string(),
		inputs: Vec::new(),
		implementation: DocumentNodeImplementation::proto("graphene_core::memo::MonitorNode<_, _, _>"),
		manual_composition: Some(generic!(T)),
		skip_deduplication: true,
		..Default::default()
	}
}

// TODO: Dynamic node library
/// Defines the "signature" or "header file"-like metadata for the document nodes, but not the implementation (which is defined in the node registry).
/// The [`DocumentNode`] is the instance while these [`DocumentNodeDefinition`]s are the "classes" or "blueprints" from which the instances are built.
fn static_nodes() -> Vec<DocumentNodeDefinition> {
	vec![
		DocumentNodeDefinition {
			name: "Boolean",
			category: "Inputs",
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::IdentityNode"),
			inputs: vec![DocumentInputType::value("Bool", TaggedValue::Bool(true), false)],
			outputs: vec![DocumentOutputType::new("Out", FrontendGraphDataType::General)],
			properties: node_properties::boolean_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Number",
			category: "Inputs",
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::IdentityNode"),
			inputs: vec![DocumentInputType::value("Number", TaggedValue::F64(0.), false)],
			outputs: vec![DocumentOutputType::new("Out", FrontendGraphDataType::Number)],
			properties: node_properties::number_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Color",
			category: "Inputs",
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::IdentityNode"),
			inputs: vec![DocumentInputType::value("Color", TaggedValue::OptionalColor(None), false)],
			outputs: vec![DocumentOutputType::new("Out", FrontendGraphDataType::General)],
			properties: node_properties::color_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Vector2",
			category: "Inputs",
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::ConstructVector2<_, _>"),
			inputs: vec![
				DocumentInputType::none(),
				DocumentInputType::value("X", TaggedValue::F64(0.), false),
				DocumentInputType::value("Y", TaggedValue::F64(0.), false),
			],
			outputs: vec![DocumentOutputType::new("Out", FrontendGraphDataType::Number)],
			properties: node_properties::vector2_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Identity",
			category: "Structural",
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::IdentityNode"),
			inputs: vec![DocumentInputType {
				name: "In",
				data_type: FrontendGraphDataType::General,
				default: NodeInput::value(TaggedValue::None, true),
			}],
			outputs: vec![DocumentOutputType::new("Out", FrontendGraphDataType::General)],
			properties: |_document_node, _node_id, _context| node_properties::string_properties("The identity node simply returns the input"),
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Monitor",
			category: "Structural",
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::IdentityNode"),
			inputs: vec![DocumentInputType {
				name: "In",
				data_type: FrontendGraphDataType::General,
				default: NodeInput::value(TaggedValue::None, true),
			}],
			outputs: vec![DocumentOutputType::new("Out", FrontendGraphDataType::General)],
			properties: |_document_node, _node_id, _context| node_properties::string_properties("The Monitor node stores the value of its last evaluation"),
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Merge",
			category: "General",
			is_layer: true,
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![NodeInput::node(NodeId(3), 0)],
				nodes: [
					// Secondary (left) input type coercion
					(
						NodeId(0),
						DocumentNode {
							name: "To Graphic Element".to_string(),
							inputs: vec![NodeInput::network(generic!(T), 1)],
							implementation: DocumentNodeImplementation::proto("graphene_core::ToGraphicElementNode"),
							metadata: DocumentNodeMetadata { position: glam::IVec2::new(-14, -1) }, // To Graphic Element
							..Default::default()
						},
					),
					// Primary (bottom) input type coercion
					(
						NodeId(1),
						DocumentNode {
							name: "To Graphic Group".to_string(),
							inputs: vec![NodeInput::network(generic!(T), 0)],
							implementation: DocumentNodeImplementation::proto("graphene_core::ToGraphicGroupNode"),
							metadata: DocumentNodeMetadata { position: glam::IVec2::new(-14, -3) }, // To Graphic Group
							..Default::default()
						},
					),
					// The monitor node is used to display a thumbnail in the UI
					(
						NodeId(2),
						DocumentNode {
							inputs: vec![NodeInput::node(NodeId(0), 0)],
							metadata: DocumentNodeMetadata { position: glam::IVec2::new(-7, -1) }, // Monitor
							..monitor_node()
						},
					),
					(
						NodeId(3),
						DocumentNode {
							name: "ConstructLayer".to_string(),
							manual_composition: Some(concrete!(Footprint)),
							inputs: vec![NodeInput::node(NodeId(1), 0), NodeInput::node(NodeId(2), 0)],
							implementation: DocumentNodeImplementation::proto("graphene_core::ConstructLayerNode<_, _>"),
							metadata: DocumentNodeMetadata { position: glam::IVec2::new(1, -3) }, // ConstructLayer
							..Default::default()
						},
					),
				]
				.into(),
				imports_metadata: (NodeId(generate_uuid()), (-26, -4).into()),
				exports_metadata: (NodeId(generate_uuid()), (8, -4).into()),
				..Default::default()
			}),
			inputs: vec![
				DocumentInputType::value("Graphical Data", TaggedValue::GraphicGroup(GraphicGroup::EMPTY), true),
				DocumentInputType::value("Over", TaggedValue::GraphicGroup(GraphicGroup::EMPTY), true),
			],
			outputs: vec![DocumentOutputType::new("Out", FrontendGraphDataType::Graphic)],
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Artboard",
			category: "General",
			is_layer: true,
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![NodeInput::node(NodeId(2), 0)],
				nodes: [
					(
						NodeId(0),
						DocumentNode {
							name: "To Artboard".to_string(),
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
							metadata: DocumentNodeMetadata { position: glam::IVec2::new(-10, -3) }, // To Artboard
							..Default::default()
						},
					),
					// The monitor node is used to display a thumbnail in the UI.
					// TODO: Check if thumbnail is reversed
					(
						NodeId(1),
						DocumentNode {
							inputs: vec![NodeInput::node(NodeId(0), 0)],
							metadata: DocumentNodeMetadata { position: glam::IVec2::new(-2, -3) }, // Monitor
							..monitor_node()
						},
					),
					(
						NodeId(2),
						DocumentNode {
							name: "Add to Artboards".to_string(),
							manual_composition: Some(concrete!(Footprint)),
							inputs: vec![
								NodeInput::network(graphene_core::Type::Fn(Box::new(concrete!(Footprint)), Box::new(concrete!(ArtboardGroup))), 0),
								NodeInput::node(NodeId(1), 0),
							],
							implementation: DocumentNodeImplementation::proto("graphene_core::AddArtboardNode<_, _>"),
							metadata: DocumentNodeMetadata { position: glam::IVec2::new(6, -4) }, // Add to Artboards
							..Default::default()
						},
					),
				]
				.into(),
				imports_metadata: (NodeId(generate_uuid()), (-21, -5).into()),
				exports_metadata: (NodeId(generate_uuid()), (14, -5).into()),
				..Default::default()
			}),
			inputs: vec![
				DocumentInputType::value("Artboards", TaggedValue::ArtboardGroup(ArtboardGroup::EMPTY), true),
				DocumentInputType::value("Contents", TaggedValue::GraphicGroup(GraphicGroup::EMPTY), true),
				DocumentInputType::value("Location", TaggedValue::IVec2(glam::IVec2::ZERO), false),
				DocumentInputType::value("Dimensions", TaggedValue::IVec2(glam::IVec2::new(1920, 1080)), false),
				DocumentInputType::value("Background", TaggedValue::Color(Color::WHITE), false),
				DocumentInputType::value("Clip", TaggedValue::Bool(false), false),
			],
			outputs: vec![DocumentOutputType::new("Out", FrontendGraphDataType::Artboard)],
			properties: node_properties::artboard_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Load Image",
			category: "Structural",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![NodeInput::node(NodeId(2), 0)],
				nodes: [
					DocumentNode {
						name: "Load Resource".to_string(),
						inputs: vec![NodeInput::scope("editor-api"), NodeInput::network(concrete!(String), 1)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::wasm_application_io::LoadResourceNode<_>")),
						..Default::default()
					},
					DocumentNode {
						name: "Decode Image".to_string(),
						inputs: vec![NodeInput::node(NodeId(0), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::wasm_application_io::DecodeImageNode")),
						..Default::default()
					},
					DocumentNode {
						name: "Cull".to_string(),
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
			inputs: vec![
				DocumentInputType {
					name: "api",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::scope("editor-api"),
				},
				DocumentInputType {
					name: "path",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::value(TaggedValue::String("graphite:null".to_string()), false),
				},
			],
			outputs: vec![DocumentOutputType {
				name: "Image Frame",
				data_type: FrontendGraphDataType::Raster,
			}],
			properties: node_properties::load_image_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Create Canvas",
			category: "Structural",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![NodeInput::node(NodeId(1), 0)],
				nodes: [
					DocumentNode {
						name: "Create Canvas".to_string(),
						inputs: vec![NodeInput::scope("editor-api")],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::wasm_application_io::CreateSurfaceNode")),
						skip_deduplication: true,
						..Default::default()
					},
					DocumentNode {
						name: "Cache".to_string(),
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
			outputs: vec![DocumentOutputType {
				name: "Canvas",
				data_type: FrontendGraphDataType::General,
			}],
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Draw Canvas",
			category: "Structural",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![NodeInput::node(NodeId(3), 0)],
				nodes: [
					DocumentNode {
						name: "Convert Image Frame".to_string(),
						inputs: vec![NodeInput::network(concrete!(ImageFrame<Color>), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<_, ImageFrame<SRGBA8>>")),
						..Default::default()
					},
					DocumentNode {
						name: "Create Canvas".to_string(),
						inputs: vec![NodeInput::scope("editor-api")],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::wasm_application_io::CreateSurfaceNode")),
						skip_deduplication: true,
						..Default::default()
					},
					DocumentNode {
						name: "Cache".to_string(),
						manual_composition: Some(concrete!(())),
						inputs: vec![NodeInput::node(NodeId(1), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::MemoNode<_, _>")),
						..Default::default()
					},
					DocumentNode {
						name: "Draw Canvas".to_string(),
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
			inputs: vec![DocumentInputType {
				name: "In",
				data_type: FrontendGraphDataType::Raster,
				default: NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
			}],
			outputs: vec![DocumentOutputType {
				name: "Canvas",
				data_type: FrontendGraphDataType::General,
			}],
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Rasterize",
			category: "Raster",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![NodeInput::node(NodeId(2), 0)],
				nodes: [
					DocumentNode {
						name: "Create Canvas".to_string(),
						inputs: vec![NodeInput::scope("editor-api")],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::wasm_application_io::CreateSurfaceNode")),
						skip_deduplication: true,
						..Default::default()
					},
					DocumentNode {
						name: "Cache".to_string(),
						manual_composition: Some(concrete!(())),
						inputs: vec![NodeInput::node(NodeId(0), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::MemoNode<_, _>")),
						..Default::default()
					},
					DocumentNode {
						name: "Rasterize".to_string(),
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
				DocumentInputType {
					name: "Artwork",
					data_type: FrontendGraphDataType::Raster,
					default: NodeInput::value(TaggedValue::VectorData(VectorData::default()), true),
				},
				DocumentInputType {
					name: "Footprint",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::value(
						TaggedValue::Footprint(Footprint {
							transform: DAffine2::from_scale_angle_translation(DVec2::new(100., 100.), 0., DVec2::new(0., 0.)),
							resolution: UVec2::new(100, 100),
							..Default::default()
						}),
						false,
					),
				},
			],
			properties: node_properties::rasterize_properties,
			outputs: vec![DocumentOutputType {
				name: "Canvas",
				data_type: FrontendGraphDataType::General,
			}],
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Image Frame",
			category: "General",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![NodeInput::node(NodeId(1), 0)],
				nodes: vec![
					DocumentNode {
						name: "Image Frame".to_string(),
						inputs: vec![NodeInput::network(concrete!(graphene_core::raster::Image<Color>), 0), NodeInput::network(concrete!(DAffine2), 1)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::raster::ImageFrameNode<_, _>")),
						..Default::default()
					},
					DocumentNode {
						name: "Cull".to_string(),
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
				DocumentInputType::value("Image", TaggedValue::Image(Image::empty()), true),
				DocumentInputType::value("Transform", TaggedValue::DAffine2(DAffine2::IDENTITY), true),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: |_document_node, _node_id, _context| node_properties::string_properties("Creates an embedded image with the given transform"),
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Noise Pattern",
			category: "General",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![NodeInput::node(NodeId(1), 0)],
				nodes: vec![
					DocumentNode {
						name: "Noise Pattern".to_string(),
						inputs: vec![
							NodeInput::network(concrete!(()), 0),
							NodeInput::network(concrete!(UVec2), 1),
							NodeInput::network(concrete!(u32), 2),
							NodeInput::network(concrete!(f64), 3),
							NodeInput::network(concrete!(graphene_core::raster::NoiseType), 4),
							NodeInput::network(concrete!(graphene_core::raster::FractalType), 5),
							NodeInput::network(concrete!(f64), 6),
							NodeInput::network(concrete!(graphene_core::raster::FractalType), 7),
							NodeInput::network(concrete!(u32), 8),
							NodeInput::network(concrete!(f64), 9),
							NodeInput::network(concrete!(f64), 10),
							NodeInput::network(concrete!(f64), 11),
							NodeInput::network(concrete!(f64), 12),
							NodeInput::network(concrete!(graphene_core::raster::CellularDistanceFunction), 13),
							NodeInput::network(concrete!(graphene_core::raster::CellularReturnType), 14),
							NodeInput::network(concrete!(f64), 15),
						],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::raster::NoisePatternNode<_, _, _, _, _, _, _, _, _, _, _, _, _, _, _>")),
						..Default::default()
					},
					// TODO: Make noise pattern node resolution aware and remove the cull node
					DocumentNode {
						name: "Cull".to_string(),
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
				DocumentInputType::value("None", TaggedValue::None, false),
				// All
				DocumentInputType::value("Dimensions", TaggedValue::UVec2((512, 512).into()), false),
				DocumentInputType::value("Seed", TaggedValue::U32(0), false),
				DocumentInputType::value("Scale", TaggedValue::F64(10.), false),
				DocumentInputType::value("Noise Type", TaggedValue::NoiseType(NoiseType::default()), false),
				// Domain Warp
				DocumentInputType::value("Domain Warp Type", TaggedValue::DomainWarpType(DomainWarpType::default()), false),
				DocumentInputType::value("Domain Warp Amplitude", TaggedValue::F64(100.), false),
				// Fractal
				DocumentInputType::value("Fractal Type", TaggedValue::FractalType(FractalType::default()), false),
				DocumentInputType::value("Fractal Octaves", TaggedValue::U32(3), false),
				DocumentInputType::value("Fractal Lacunarity", TaggedValue::F64(2.), false),
				DocumentInputType::value("Fractal Gain", TaggedValue::F64(0.5), false),
				DocumentInputType::value("Fractal Weighted Strength", TaggedValue::F64(0.), false), // 0-1 range
				DocumentInputType::value("Fractal Ping Pong Strength", TaggedValue::F64(2.), false),
				// Cellular
				DocumentInputType::value("Cellular Distance Function", TaggedValue::CellularDistanceFunction(CellularDistanceFunction::default()), false),
				DocumentInputType::value("Cellular Return Type", TaggedValue::CellularReturnType(CellularReturnType::default()), false),
				DocumentInputType::value("Cellular Jitter", TaggedValue::F64(1.), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::noise_pattern_properties,
			..Default::default()
		},
		// TODO: This needs to work with resolution-aware (raster with footprint, post-Cull node) data.
		DocumentNodeDefinition {
			name: "Mask",
			category: "Raster",
			implementation: DocumentNodeImplementation::proto("graphene_std::raster::MaskImageNode<_, _, _>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Stencil", TaggedValue::ImageFrame(ImageFrame::empty()), true),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::mask_properties,
			..Default::default()
		},
		// TODO: This needs to work with resolution-aware (raster with footprint, post-Cull node) data.
		DocumentNodeDefinition {
			name: "Insert Channel",
			category: "Raster",
			implementation: DocumentNodeImplementation::proto("graphene_std::raster::InsertChannelNode<_, _, _, _>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Insertion", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Replace", TaggedValue::RedGreenBlue(RedGreenBlue::default()), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::insert_channel_properties,
			..Default::default()
		},
		// TODO: This needs to work with resolution-aware (raster with footprint, post-Cull node) data.
		DocumentNodeDefinition {
			name: "Combine Channels",
			category: "Raster",
			implementation: DocumentNodeImplementation::proto("graphene_std::raster::CombineChannelsNode"),
			inputs: vec![
				DocumentInputType::value("None", TaggedValue::None, false),
				DocumentInputType::value("Red", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Green", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Blue", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Alpha", TaggedValue::ImageFrame(ImageFrame::empty()), true),
			],
			outputs: vec![DocumentOutputType {
				name: "Image",
				data_type: FrontendGraphDataType::Raster,
			}],
			..Default::default()
		},
		// TODO: This needs to work with resolution-aware (raster with footprint, post-Cull node) data.
		DocumentNodeDefinition {
			name: "Blend",
			category: "Raster",
			implementation: DocumentNodeImplementation::proto("graphene_core::raster::BlendNode<_, _, _, _>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Second", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("BlendMode", TaggedValue::BlendMode(BlendMode::Normal), false),
				DocumentInputType::value("Opacity", TaggedValue::F64(100.), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::blend_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Levels",
			category: "Raster",
			implementation: DocumentNodeImplementation::proto("graphene_core::raster::LevelsNode<_, _, _, _, _>"),
			inputs: vec![
				DocumentInputType {
					name: "Image",
					data_type: FrontendGraphDataType::Raster,
					default: NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
				},
				DocumentInputType {
					name: "Shadows",
					data_type: FrontendGraphDataType::Number,
					default: NodeInput::value(TaggedValue::F64(0.), false),
				},
				DocumentInputType {
					name: "Midtones",
					data_type: FrontendGraphDataType::Number,
					default: NodeInput::value(TaggedValue::F64(50.), false),
				},
				DocumentInputType {
					name: "Highlights",
					data_type: FrontendGraphDataType::Number,
					default: NodeInput::value(TaggedValue::F64(100.), false),
				},
				DocumentInputType {
					name: "Output Minimums",
					data_type: FrontendGraphDataType::Number,
					default: NodeInput::value(TaggedValue::F64(0.), false),
				},
				DocumentInputType {
					name: "Output Maximums",
					data_type: FrontendGraphDataType::Number,
					default: NodeInput::value(TaggedValue::F64(100.), false),
				},
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::levels_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Black & White",
			category: "Raster",
			implementation: DocumentNodeImplementation::proto("graphene_core::raster::BlackAndWhiteNode<_, _, _, _, _, _, _>"),
			inputs: vec![
				DocumentInputType {
					name: "Image",
					data_type: FrontendGraphDataType::Raster,
					default: NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
				},
				DocumentInputType {
					name: "Tint",
					data_type: FrontendGraphDataType::Number,
					default: NodeInput::value(TaggedValue::Color(Color::BLACK), false),
				},
				DocumentInputType {
					name: "Reds",
					data_type: FrontendGraphDataType::Number,
					default: NodeInput::value(TaggedValue::F64(40.), false),
				},
				DocumentInputType {
					name: "Yellows",
					data_type: FrontendGraphDataType::Number,
					default: NodeInput::value(TaggedValue::F64(60.), false),
				},
				DocumentInputType {
					name: "Greens",
					data_type: FrontendGraphDataType::Number,
					default: NodeInput::value(TaggedValue::F64(40.), false),
				},
				DocumentInputType {
					name: "Cyans",
					data_type: FrontendGraphDataType::Number,
					default: NodeInput::value(TaggedValue::F64(60.), false),
				},
				DocumentInputType {
					name: "Blues",
					data_type: FrontendGraphDataType::Number,
					default: NodeInput::value(TaggedValue::F64(20.), false),
				},
				DocumentInputType {
					name: "Magentas",
					data_type: FrontendGraphDataType::Number,
					default: NodeInput::value(TaggedValue::F64(80.), false),
				},
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::black_and_white_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Color Channel",
			category: "Raster",
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::IdentityNode"),
			inputs: vec![DocumentInputType::value("Channel", TaggedValue::RedGreenBlue(RedGreenBlue::default()), false)],
			outputs: vec![DocumentOutputType::new("Out", FrontendGraphDataType::General)],
			properties: node_properties::color_channel_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Blend Mode Value",
			category: "Inputs",
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::IdentityNode"),
			inputs: vec![DocumentInputType::value("Blend Mode", TaggedValue::BlendMode(BlendMode::default()), false)],
			outputs: vec![DocumentOutputType::new("Out", FrontendGraphDataType::General)],
			properties: node_properties::blend_mode_value_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Luminance",
			category: "Raster",
			implementation: DocumentNodeImplementation::proto("graphene_core::raster::LuminanceNode<_>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Luminance Calc", TaggedValue::LuminanceCalculation(LuminanceCalculation::default()), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::luminance_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Extract Channel",
			category: "Raster",
			implementation: DocumentNodeImplementation::proto("graphene_core::raster::ExtractChannelNode<_>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("From", TaggedValue::RedGreenBlueAlpha(RedGreenBlueAlpha::default()), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::extract_channel_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Extract Opaque",
			category: "Raster",
			implementation: DocumentNodeImplementation::proto("graphene_core::raster::ExtractOpaqueNode<>"),
			inputs: vec![DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true)],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Split Channels",
			category: "Raster",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![
					NodeInput::node(NodeId(0), 0),
					NodeInput::node(NodeId(1), 0),
					NodeInput::node(NodeId(2), 0),
					NodeInput::node(NodeId(3), 0),
				],
				nodes: [
					DocumentNode {
						name: "RedNode".to_string(),
						inputs: vec![
							NodeInput::network(concrete!(ImageFrame<Color>), 0),
							NodeInput::value(TaggedValue::RedGreenBlueAlpha(RedGreenBlueAlpha::Red), false),
						],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::raster::ExtractChannelNode<_>")),
						..Default::default()
					},
					DocumentNode {
						name: "GreenNode".to_string(),
						inputs: vec![
							NodeInput::network(concrete!(ImageFrame<Color>), 0),
							NodeInput::value(TaggedValue::RedGreenBlueAlpha(RedGreenBlueAlpha::Green), false),
						],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::raster::ExtractChannelNode<_>")),
						..Default::default()
					},
					DocumentNode {
						name: "BlueNode".to_string(),
						inputs: vec![
							NodeInput::network(concrete!(ImageFrame<Color>), 0),
							NodeInput::value(TaggedValue::RedGreenBlueAlpha(RedGreenBlueAlpha::Blue), false),
						],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::raster::ExtractChannelNode<_>")),
						..Default::default()
					},
					DocumentNode {
						name: "AlphaNode".to_string(),
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
			inputs: vec![DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true)],
			outputs: vec![
				DocumentOutputType::new("Red", FrontendGraphDataType::Raster),
				DocumentOutputType::new("Green", FrontendGraphDataType::Raster),
				DocumentOutputType::new("Blue", FrontendGraphDataType::Raster),
				DocumentOutputType::new("Alpha", FrontendGraphDataType::Raster),
			],
			has_primary_output: false,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Brush",
			category: "Brush",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![NodeInput::node(NodeId(1), 0)],
				nodes: vec![
					DocumentNode {
						name: "Brush".to_string(),
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
						name: "Cull".to_string(),
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
				DocumentInputType::value("Background", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Bounds", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Trace", TaggedValue::BrushStrokes(Vec::new()), false),
				DocumentInputType::value("Cache", TaggedValue::BrushCache(BrushCache::new_proto()), false),
			],
			outputs: vec![DocumentOutputType {
				name: "Image",
				data_type: FrontendGraphDataType::Raster,
			}],
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Extract Vector Points",
			category: "Brush",
			implementation: DocumentNodeImplementation::proto("graphene_std::brush::VectorPointsNode"),
			inputs: vec![DocumentInputType::value("VectorData", TaggedValue::VectorData(VectorData::empty()), true)],
			outputs: vec![DocumentOutputType {
				name: "Vector Points",
				data_type: FrontendGraphDataType::General,
			}],
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Memoize",
			category: "Structural",
			implementation: DocumentNodeImplementation::proto("graphene_core::memo::MemoNode<_, _>"),
			inputs: vec![DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true)],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			manual_composition: Some(concrete!(())),
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "MemoizeImpure",
			category: "Structural",
			implementation: DocumentNodeImplementation::proto("graphene_core::memo::ImpureMemoNode<_, _, _>"),
			inputs: vec![DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true)],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			manual_composition: Some(concrete!(Footprint)),
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Image",
			category: "Ignore",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![NodeInput::node(NodeId(0), 0)],
				nodes: vec![DocumentNode {
					name: "Cull".to_string(),
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
			inputs: vec![DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), false)],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: |_document_node, _node_id, _context| node_properties::string_properties("A bitmap image embedded in this node"),
			..Default::default()
		},
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			name: "Uniform",
			category: "Gpu",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![NodeInput::node(NodeId(2), 0)],
				nodes: [
					DocumentNode {
						name: "Extract Executor".to_string(),
						inputs: vec![NodeInput::scope("editor-api")],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<_, &WgpuExecutor>")),
						..Default::default()
					},
					DocumentNode {
						name: "Create Uniform".to_string(),
						inputs: vec![NodeInput::network(generic!(T), 0), NodeInput::node(NodeId(0), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::UniformNode<_>")),
						..Default::default()
					},
					DocumentNode {
						name: "Cache".to_string(),
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
			inputs: vec![DocumentInputType {
				name: "In",
				data_type: FrontendGraphDataType::General,
				default: NodeInput::value(TaggedValue::F64(0.), true),
			}],
			outputs: vec![DocumentOutputType {
				name: "Uniform",
				data_type: FrontendGraphDataType::General,
			}],
			..Default::default()
		},
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			name: "Storage",
			category: "Gpu",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![NodeInput::node(NodeId(2), 0)],
				nodes: [
					DocumentNode {
						name: "Extract Executor".to_string(),
						inputs: vec![NodeInput::scope("editor-api")],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<_, &WgpuExecutor>")),
						..Default::default()
					},
					DocumentNode {
						name: "Create Storage".to_string(),
						inputs: vec![NodeInput::network(concrete!(Vec<u8>), 0), NodeInput::node(NodeId(0), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::StorageNode<_>")),
						..Default::default()
					},
					DocumentNode {
						name: "Cache".to_string(),
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
			inputs: vec![DocumentInputType {
				name: "In",
				data_type: FrontendGraphDataType::General,
				default: NodeInput::value(TaggedValue::None, true),
			}],
			outputs: vec![DocumentOutputType {
				name: "Storage",
				data_type: FrontendGraphDataType::General,
			}],
			..Default::default()
		},
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			name: "CreateOutputBuffer",
			category: "Gpu",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![NodeInput::node(NodeId(2), 0)],
				nodes: [
					DocumentNode {
						name: "Extract Executor".to_string(),
						inputs: vec![NodeInput::scope("editor-api")],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<_, &WgpuExecutor>")),
						..Default::default()
					},
					DocumentNode {
						name: "Create Output Buffer".to_string(),
						inputs: vec![NodeInput::network(concrete!(usize), 0), NodeInput::node(NodeId(0), 0), NodeInput::network(concrete!(Type), 1)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::CreateOutputBufferNode<_, _>")),
						..Default::default()
					},
					DocumentNode {
						name: "Cache".to_string(),
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
				DocumentInputType {
					name: "In",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::value(TaggedValue::None, true),
				},
				DocumentInputType {
					name: "In",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::value(TaggedValue::None, true),
				},
			],
			outputs: vec![DocumentOutputType {
				name: "OutputBuffer",
				data_type: FrontendGraphDataType::General,
			}],
			properties: node_properties::node_no_properties,
			..Default::default()
		},
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			name: "CreateComputePass",
			category: "Gpu",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![NodeInput::node(NodeId(2), 0)],
				nodes: [
					DocumentNode {
						name: "Extract Executor".to_string(),
						inputs: vec![NodeInput::scope("editor-api")],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<_, &WgpuExecutor>")),
						..Default::default()
					},
					DocumentNode {
						name: "Create Compute Pass".to_string(),
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
						name: "Cache".to_string(),
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
				DocumentInputType {
					name: "In",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::network(concrete!(PipelineLayout), 0),
				},
				DocumentInputType {
					name: "In",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::network(concrete!(WgpuShaderInput), 2),
				},
				DocumentInputType {
					name: "In",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::network(concrete!(gpu_executor::ComputePassDimensions), 3),
				},
			],
			outputs: vec![DocumentOutputType {
				name: "CommandBuffer",
				data_type: FrontendGraphDataType::General,
			}],
			properties: node_properties::node_no_properties,
			..Default::default()
		},
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			name: "CreatePipelineLayout",
			category: "Gpu",
			implementation: DocumentNodeImplementation::proto("wgpu_executor::CreatePipelineLayoutNode<_, _, _>"),
			inputs: vec![
				DocumentInputType {
					name: "ShaderHandle",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::network(concrete!(ShaderHandle), 0),
				},
				DocumentInputType {
					name: "String",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::network(concrete!(String), 1),
				},
				DocumentInputType {
					name: "Bindgroup",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::network(concrete!(Bindgroup), 2),
				},
				DocumentInputType {
					name: "ArcShaderInput",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::network(concrete!(Arc<WgpuShaderInput>), 3),
				},
			],
			outputs: vec![DocumentOutputType {
				name: "PipelineLayout",
				data_type: FrontendGraphDataType::General,
			}],
			properties: node_properties::node_no_properties,
			..Default::default()
		},
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			name: "ExecuteComputePipeline",
			category: "Gpu",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![NodeInput::node(NodeId(2), 0)],
				nodes: [
					DocumentNode {
						name: "Extract Executor".to_string(),
						inputs: vec![NodeInput::scope("editor-api")],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<_, &WgpuExecutor>")),
						..Default::default()
					},
					DocumentNode {
						name: "Execute Compute Pipeline".to_string(),
						inputs: vec![NodeInput::network(concrete!(CommandBuffer), 0), NodeInput::node(NodeId(0), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::ExecuteComputePipelineNode<_>")),
						..Default::default()
					},
					DocumentNode {
						name: "Cache".to_string(),
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
			inputs: vec![DocumentInputType {
				name: "In",
				data_type: FrontendGraphDataType::General,
				default: NodeInput::value(TaggedValue::None, true),
			}],
			outputs: vec![DocumentOutputType {
				name: "PipelineResult",
				data_type: FrontendGraphDataType::General,
			}],
			..Default::default()
		},
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			name: "ReadOutputBuffer",
			category: "Gpu",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![NodeInput::node(NodeId(2), 0)],
				nodes: [
					DocumentNode {
						name: "Extract Executor".to_string(),
						inputs: vec![NodeInput::scope("editor-api")],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<_, &WgpuExecutor>")),
						..Default::default()
					},
					DocumentNode {
						name: "Read Output Buffer".to_string(),
						inputs: vec![NodeInput::network(concrete!(Arc<WgpuShaderInput>), 0), NodeInput::node(NodeId(0), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::ReadOutputBufferNode<_, _>")),
						..Default::default()
					},
					DocumentNode {
						name: "Cache".to_string(),
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
			inputs: vec![DocumentInputType {
				name: "In",
				data_type: FrontendGraphDataType::General,
				default: NodeInput::value(TaggedValue::None, true),
			}],
			outputs: vec![DocumentOutputType {
				name: "Buffer",
				data_type: FrontendGraphDataType::General,
			}],
			..Default::default()
		},
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			name: "CreateGpuSurface",
			category: "Gpu",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![NodeInput::node(NodeId(1), 0)],
				nodes: [
					DocumentNode {
						name: "Create Gpu Surface".to_string(),
						manual_composition: Some(concrete!(Footprint)),
						inputs: vec![NodeInput::scope("editor-api")],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::CreateGpuSurfaceNode<_>")),
						..Default::default()
					},
					DocumentNode {
						name: "Cache".to_string(),
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
			outputs: vec![DocumentOutputType {
				name: "GpuSurface",
				data_type: FrontendGraphDataType::General,
			}],
			..Default::default()
		},
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			name: "RenderTexture",
			category: "Gpu",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![NodeInput::node(NodeId(1), 0)],
				nodes: [
					DocumentNode {
						name: "Extract Executor".to_string(),
						inputs: vec![NodeInput::scope("editor-api")],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<_, &WgpuExecutor>")),
						..Default::default()
					},
					DocumentNode {
						name: "Render Texture".to_string(),
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
			inputs: vec![
				DocumentInputType {
					name: "Texture",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::value(TaggedValue::None, true),
				},
				DocumentInputType {
					name: "Surface",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::value(TaggedValue::None, true),
				},
			],
			outputs: vec![DocumentOutputType {
				name: "RenderedTexture",
				data_type: FrontendGraphDataType::General,
			}],
			..Default::default()
		},
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			name: "UploadTexture",
			category: "Gpu",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![NodeInput::node(NodeId(2), 0)],
				nodes: [
					DocumentNode {
						name: "Extract Executor".to_string(),
						inputs: vec![NodeInput::scope("editor-api")],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<_, &WgpuExecutor>")),
						..Default::default()
					},
					DocumentNode {
						name: "Upload Texture".to_string(),
						inputs: vec![NodeInput::network(concrete!(ImageFrame<Color>), 0), NodeInput::node(NodeId(0), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::UploadTextureNode<_>")),
						..Default::default()
					},
					DocumentNode {
						name: "Cache".to_string(),
						manual_composition: Some(concrete!(())),
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
			inputs: vec![DocumentInputType {
				name: "In",
				data_type: FrontendGraphDataType::General,
				default: NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
			}],
			outputs: vec![DocumentOutputType {
				name: "Texture",
				data_type: FrontendGraphDataType::General,
			}],
			..Default::default()
		},
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			name: "GpuImage",
			category: "Raster",
			implementation: DocumentNodeImplementation::proto("graphene_std::executor::MapGpuSingleImageNode<_>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType {
					name: "Node",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::value(TaggedValue::DocumentNode(DocumentNode::default()), true),
				},
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			..Default::default()
		},
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			name: "Blend (GPU)",
			category: "Raster",
			implementation: DocumentNodeImplementation::proto("graphene_std::executor::BlendGpuImageNode<_, _, _>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Second", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Blend Mode", TaggedValue::BlendMode(BlendMode::Normal), false),
				DocumentInputType::value("Opacity", TaggedValue::F64(100.), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::blend_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Extract",
			category: "Macros",
			implementation: DocumentNodeImplementation::Extract,
			inputs: vec![DocumentInputType {
				name: "Node",
				data_type: FrontendGraphDataType::General,
				default: NodeInput::value(TaggedValue::DocumentNode(DocumentNode::default()), true),
			}],
			outputs: vec![DocumentOutputType::new("DocumentNode", FrontendGraphDataType::General)],
			..Default::default()
		},
		#[cfg(feature = "quantization")]
		DocumentNodeDefinition {
			name: "Generate Quantization",
			category: "Quantization",
			implementation: DocumentNodeImplementation::proto("graphene_std::quantization::GenerateQuantizationNode<_, _>"),
			inputs: vec![
				DocumentInputType {
					name: "Image",
					data_type: FrontendGraphDataType::Raster,
					default: NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
				},
				DocumentInputType {
					name: "samples",
					data_type: FrontendGraphDataType::Number,
					default: NodeInput::value(TaggedValue::U32(100), false),
				},
				DocumentInputType {
					name: "Fn index",
					data_type: FrontendGraphDataType::Number,
					default: NodeInput::value(TaggedValue::U32(0), false),
				},
			],
			outputs: vec![DocumentOutputType::new("Quantization", FrontendGraphDataType::General)],
			properties: node_properties::quantize_properties,
			..Default::default()
		},
		#[cfg(feature = "quantization")]
		DocumentNodeDefinition {
			name: "Quantize Image",
			category: "Quantization",
			implementation: DocumentNodeImplementation::proto("graphene_core::quantization::QuantizeNode<_>"),
			inputs: vec![
				DocumentInputType {
					name: "Image",
					data_type: FrontendGraphDataType::Raster,
					default: NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
				},
				DocumentInputType {
					name: "Quantization",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::value(TaggedValue::Quantization(core::array::from_fn(|_| Default::default())), true),
				},
			],
			outputs: vec![DocumentOutputType::new("Encoded", FrontendGraphDataType::Raster)],
			properties: node_properties::quantize_properties,
			..Default::default()
		},
		#[cfg(feature = "quantization")]
		DocumentNodeDefinition {
			name: "DeQuantize Image",
			category: "Quantization",
			implementation: DocumentNodeImplementation::proto("graphene_core::quantization::DeQuantizeNode<_>"),
			inputs: vec![
				DocumentInputType {
					name: "Encoded",
					data_type: FrontendGraphDataType::Raster,
					default: NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
				},
				DocumentInputType {
					name: "Quantization",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::value(TaggedValue::Quantization(core::array::from_fn(|_| Default::default())), true),
				},
			],
			outputs: vec![DocumentOutputType::new("Decoded", FrontendGraphDataType::Raster)],
			properties: node_properties::quantize_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Invert RGB",
			category: "Raster",
			implementation: DocumentNodeImplementation::proto("graphene_core::raster::InvertRGBNode"),
			inputs: vec![DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true)],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Hue/Saturation",
			category: "Raster",
			implementation: DocumentNodeImplementation::proto("graphene_core::raster::HueSaturationNode<_, _, _>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Hue Shift", TaggedValue::F64(0.), false),
				DocumentInputType::value("Saturation Shift", TaggedValue::F64(0.), false),
				DocumentInputType::value("Lightness Shift", TaggedValue::F64(0.), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::adjust_hsl_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Brightness/Contrast",
			category: "Raster",
			implementation: DocumentNodeImplementation::proto("graphene_core::raster::BrightnessContrastNode<_, _, _>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Brightness", TaggedValue::F64(0.), false),
				DocumentInputType::value("Contrast", TaggedValue::F64(0.), false),
				DocumentInputType::value("Use Legacy", TaggedValue::Bool(false), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::brightness_contrast_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Curves",
			category: "Raster",
			implementation: DocumentNodeImplementation::proto("graphene_core::raster::CurvesNode<_>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Curve", TaggedValue::Curve(Default::default()), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::curves_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Threshold",
			category: "Raster",
			implementation: DocumentNodeImplementation::proto("graphene_core::raster::ThresholdNode<_, _, _>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Min Luminance", TaggedValue::F64(50.), false),
				DocumentInputType::value("Max Luminance", TaggedValue::F64(100.), false),
				DocumentInputType::value("Luminance Calc", TaggedValue::LuminanceCalculation(LuminanceCalculation::SRGB), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::adjust_threshold_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Vibrance",
			category: "Raster",
			implementation: DocumentNodeImplementation::proto("graphene_core::raster::VibranceNode<_>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Vibrance", TaggedValue::F64(0.), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::adjust_vibrance_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Channel Mixer",
			category: "Raster",
			implementation: DocumentNodeImplementation::proto("graphene_core::raster::ChannelMixerNode<_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				// Monochrome toggle
				DocumentInputType::value("Monochrome", TaggedValue::Bool(false), false),
				// Monochrome
				DocumentInputType::value("Red", TaggedValue::F64(40.), false),
				DocumentInputType::value("Green", TaggedValue::F64(40.), false),
				DocumentInputType::value("Blue", TaggedValue::F64(20.), false),
				DocumentInputType::value("Constant", TaggedValue::F64(0.), false),
				// Red output channel
				DocumentInputType::value("(Red) Red", TaggedValue::F64(100.), false),
				DocumentInputType::value("(Red) Green", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Red) Blue", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Red) Constant", TaggedValue::F64(0.), false),
				// Green output channel
				DocumentInputType::value("(Green) Red", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Green) Green", TaggedValue::F64(100.), false),
				DocumentInputType::value("(Green) Blue", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Green) Constant", TaggedValue::F64(0.), false),
				// Blue output channel
				DocumentInputType::value("(Blue) Red", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Blue) Green", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Blue) Blue", TaggedValue::F64(100.), false),
				DocumentInputType::value("(Blue) Constant", TaggedValue::F64(0.), false),
				// Display-only properties (not used within the node)
				DocumentInputType::value("Output Channel", TaggedValue::RedGreenBlue(RedGreenBlue::default()), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::adjust_channel_mixer_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Selective Color",
			category: "Raster",
			implementation: DocumentNodeImplementation::proto(
				"graphene_core::raster::SelectiveColorNode<_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _>",
			),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				// Mode
				DocumentInputType::value("Mode", TaggedValue::RelativeAbsolute(RelativeAbsolute::default()), false),
				// Reds
				DocumentInputType::value("(Reds) Cyan", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Reds) Magenta", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Reds) Yellow", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Reds) Black", TaggedValue::F64(0.), false),
				// Yellows
				DocumentInputType::value("(Yellows) Cyan", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Yellows) Magenta", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Yellows) Yellow", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Yellows) Black", TaggedValue::F64(0.), false),
				// Greens
				DocumentInputType::value("(Greens) Cyan", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Greens) Magenta", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Greens) Yellow", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Greens) Black", TaggedValue::F64(0.), false),
				// Cyans
				DocumentInputType::value("(Cyans) Cyan", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Cyans) Magenta", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Cyans) Yellow", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Cyans) Black", TaggedValue::F64(0.), false),
				// Blues
				DocumentInputType::value("(Blues) Cyan", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Blues) Magenta", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Blues) Yellow", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Blues) Black", TaggedValue::F64(0.), false),
				// Magentas
				DocumentInputType::value("(Magentas) Cyan", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Magentas) Magenta", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Magentas) Yellow", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Magentas) Black", TaggedValue::F64(0.), false),
				// Whites
				DocumentInputType::value("(Whites) Cyan", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Whites) Magenta", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Whites) Yellow", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Whites) Black", TaggedValue::F64(0.), false),
				// Neutrals
				DocumentInputType::value("(Neutrals) Cyan", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Neutrals) Magenta", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Neutrals) Yellow", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Neutrals) Black", TaggedValue::F64(0.), false),
				// Blacks
				DocumentInputType::value("(Blacks) Cyan", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Blacks) Magenta", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Blacks) Yellow", TaggedValue::F64(0.), false),
				DocumentInputType::value("(Blacks) Black", TaggedValue::F64(0.), false),
				// Display-only properties (not used within the node)
				DocumentInputType::value("Colors", TaggedValue::SelectiveColorChoice(SelectiveColorChoice::default()), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::adjust_selective_color_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Opacity",
			category: "Raster",
			implementation: DocumentNodeImplementation::proto("graphene_core::raster::OpacityNode<_>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Factor", TaggedValue::F64(100.), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::opacity_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Blend Mode",
			category: "Raster",
			implementation: DocumentNodeImplementation::proto("graphene_core::raster::BlendModeNode<_>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Blend Mode", TaggedValue::BlendMode(BlendMode::Normal), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::blend_mode_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Posterize",
			category: "Raster",
			implementation: DocumentNodeImplementation::proto("graphene_core::raster::PosterizeNode<_>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Levels", TaggedValue::F64(4.), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::posterize_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Exposure",
			category: "Raster",
			implementation: DocumentNodeImplementation::proto("graphene_core::raster::ExposureNode<_, _, _>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Exposure", TaggedValue::F64(0.), false),
				DocumentInputType::value("Offset", TaggedValue::F64(0.), false),
				DocumentInputType::value("Gamma Correction", TaggedValue::F64(1.), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::exposure_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Add",
			category: "Math",
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::AddNode<_>"),
			inputs: vec![
				DocumentInputType::value("Primary", TaggedValue::F64(0.), true),
				DocumentInputType::value("Addend", TaggedValue::F64(0.), false),
			],
			outputs: vec![DocumentOutputType::new("Output", FrontendGraphDataType::Number)],
			properties: node_properties::add_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Subtract",
			category: "Math",
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::SubtractNode<_>"),
			inputs: vec![
				DocumentInputType::value("Primary", TaggedValue::F64(0.), true),
				DocumentInputType::value("Subtrahend", TaggedValue::F64(0.), false),
			],
			outputs: vec![DocumentOutputType::new("Output", FrontendGraphDataType::Number)],
			properties: node_properties::subtract_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Divide",
			category: "Math",
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::DivideNode<_>"),
			inputs: vec![
				DocumentInputType::value("Primary", TaggedValue::F64(0.), true),
				DocumentInputType::value("Divisor", TaggedValue::F64(1.), false),
			],
			outputs: vec![DocumentOutputType::new("Output", FrontendGraphDataType::Number)],
			properties: node_properties::divide_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Multiply",
			category: "Math",
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::MultiplyNode<_>"),
			inputs: vec![
				DocumentInputType::value("Primary", TaggedValue::F64(0.), true),
				DocumentInputType::value("Multiplicand", TaggedValue::F64(1.), false),
			],
			outputs: vec![DocumentOutputType::new("Output", FrontendGraphDataType::Number)],
			properties: node_properties::multiply_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Exponent",
			category: "Math",
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::ExponentNode<_>"),
			inputs: vec![
				DocumentInputType::value("Primary", TaggedValue::F64(0.), true),
				DocumentInputType::value("Power", TaggedValue::F64(2.), false),
			],
			outputs: vec![DocumentOutputType::new("Output", FrontendGraphDataType::Number)],
			properties: node_properties::exponent_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Floor",
			category: "Math",
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::FloorNode"),
			inputs: vec![DocumentInputType::value("Primary", TaggedValue::F64(0.), true)],
			outputs: vec![DocumentOutputType::new("Output", FrontendGraphDataType::Number)],
			properties: node_properties::node_no_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Ceil",
			category: "Math",
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::CeilingNode"),
			inputs: vec![DocumentInputType::value("Primary", TaggedValue::F64(0.), true)],
			outputs: vec![DocumentOutputType::new("Output", FrontendGraphDataType::Number)],
			properties: node_properties::node_no_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Round",
			category: "Math",
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::RoundNode"),
			inputs: vec![DocumentInputType::value("Primary", TaggedValue::F64(0.), true)],
			outputs: vec![DocumentOutputType::new("Output", FrontendGraphDataType::Number)],
			properties: node_properties::node_no_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Absolute Value",
			category: "Math",
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::AbsoluteValue"),
			inputs: vec![DocumentInputType::value("Primary", TaggedValue::F64(0.), true)],
			outputs: vec![DocumentOutputType::new("Output", FrontendGraphDataType::Number)],
			properties: node_properties::node_no_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Logarithm",
			category: "Math",
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::LogarithmNode<_>"),
			inputs: vec![
				DocumentInputType::value("Primary", TaggedValue::F64(0.), true),
				DocumentInputType::value("Base", TaggedValue::F64(0.), true),
			],
			outputs: vec![DocumentOutputType::new("Output", FrontendGraphDataType::Number)],
			properties: node_properties::log_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Natural Logarithm",
			category: "Math",
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::NaturalLogarithmNode"),
			inputs: vec![DocumentInputType::value("Primary", TaggedValue::F64(0.), true)],
			outputs: vec![DocumentOutputType::new("Output", FrontendGraphDataType::Number)],
			properties: node_properties::node_no_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Sine",
			category: "Math",
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::SineNode"),
			inputs: vec![DocumentInputType::value("Primary", TaggedValue::F64(0.), true)],
			outputs: vec![DocumentOutputType::new("Output", FrontendGraphDataType::Number)],
			properties: node_properties::node_no_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Cosine",
			category: "Math",
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::CosineNode"),
			inputs: vec![DocumentInputType::value("Primary", TaggedValue::F64(0.), true)],
			outputs: vec![DocumentOutputType::new("Output", FrontendGraphDataType::Number)],
			properties: node_properties::node_no_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Tangent",
			category: "Math",
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::TangentNode"),
			inputs: vec![DocumentInputType::value("Primary", TaggedValue::F64(0.), true)],
			outputs: vec![DocumentOutputType::new("Output", FrontendGraphDataType::Number)],
			properties: node_properties::node_no_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Max",
			category: "Math",
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::MaximumNode<_>"),
			inputs: vec![
				DocumentInputType::value("Operand A", TaggedValue::F64(0.), true),
				DocumentInputType::value("Operand B", TaggedValue::F64(0.), true),
			],
			outputs: vec![DocumentOutputType::new("Output", FrontendGraphDataType::Number)],
			properties: node_properties::max_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Min",
			category: "Math",
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::MinimumNode<_>"),
			inputs: vec![
				DocumentInputType::value("Operand A", TaggedValue::F64(0.), true),
				DocumentInputType::value("Operand B", TaggedValue::F64(0.), true),
			],
			outputs: vec![DocumentOutputType::new("Output", FrontendGraphDataType::Number)],
			properties: node_properties::min_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Equals",
			category: "Math",
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::EqualsNode<_>"),
			inputs: vec![
				DocumentInputType::value("Operand A", TaggedValue::F64(0.), true),
				DocumentInputType::value("Operand B", TaggedValue::F64(0.), true),
			],
			outputs: vec![DocumentOutputType::new("Output", FrontendGraphDataType::Number)],
			properties: node_properties::eq_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Modulo",
			category: "Math",
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::ModuloNode<_>"),
			inputs: vec![
				DocumentInputType::value("Primary", TaggedValue::F64(0.), true),
				DocumentInputType::value("Modulus", TaggedValue::F64(0.), false),
			],
			outputs: vec![DocumentOutputType::new("Output", FrontendGraphDataType::Number)],
			properties: node_properties::modulo_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Log to Console",
			category: "Logic",
			implementation: DocumentNodeImplementation::proto("graphene_core::logic::LogToConsoleNode"),
			inputs: vec![DocumentInputType::value("Input", TaggedValue::String("Not Connected to a value yet".into()), true)],
			outputs: vec![DocumentOutputType::new("Output", FrontendGraphDataType::General)],
			properties: node_properties::node_no_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Or",
			category: "Logic",
			implementation: DocumentNodeImplementation::proto("graphene_core::logic::LogicOrNode<_>"),
			inputs: vec![
				DocumentInputType::value("Operand A", TaggedValue::Bool(false), true),
				DocumentInputType::value("Operand B", TaggedValue::Bool(false), true),
			],
			outputs: vec![DocumentOutputType::new("Output", FrontendGraphDataType::General)],
			properties: node_properties::logic_operator_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "And",
			category: "Logic",
			implementation: DocumentNodeImplementation::proto("graphene_core::logic::LogicAndNode<_>"),
			inputs: vec![
				DocumentInputType::value("Operand A", TaggedValue::Bool(false), true),
				DocumentInputType::value("Operand B", TaggedValue::Bool(false), true),
			],
			outputs: vec![DocumentOutputType::new("Output", FrontendGraphDataType::General)],
			properties: node_properties::logic_operator_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "XOR",
			category: "Logic",
			implementation: DocumentNodeImplementation::proto("graphene_core::logic::LogicXorNode<_>"),
			inputs: vec![
				DocumentInputType::value("Operand A", TaggedValue::Bool(false), true),
				DocumentInputType::value("Operand B", TaggedValue::Bool(false), true),
			],
			outputs: vec![DocumentOutputType::new("Output", FrontendGraphDataType::General)],
			properties: node_properties::logic_operator_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Not",
			category: "Logic",
			implementation: DocumentNodeImplementation::proto("graphene_core::logic::LogicNotNode"),
			inputs: vec![DocumentInputType::value("Input", TaggedValue::Bool(false), true)],
			outputs: vec![DocumentOutputType::new("Output", FrontendGraphDataType::General)],
			properties: node_properties::node_no_properties,
			..Default::default()
		},
		(*IMAGINATE_NODE).clone(),
		DocumentNodeDefinition {
			name: "Circle",
			category: "Vector",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![NodeInput::node(NodeId(1), 0)],
				nodes: vec![
					DocumentNode {
						name: "Circle Generator".to_string(),
						inputs: vec![NodeInput::network(concrete!(()), 0), NodeInput::network(concrete!(f64), 1)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::vector::generator_nodes::CircleGenerator<_>")),
						..Default::default()
					},
					DocumentNode {
						name: "Cull".to_string(),
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
			inputs: vec![DocumentInputType::none(), DocumentInputType::value("Radius", TaggedValue::F64(50.), false)],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::VectorData)],
			properties: node_properties::circle_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Ellipse",
			category: "Vector",
			implementation: DocumentNodeImplementation::proto("graphene_core::vector::generator_nodes::EllipseGenerator<_, _>"),
			inputs: vec![
				DocumentInputType::none(),
				DocumentInputType::value("Radius X", TaggedValue::F64(50.), false),
				DocumentInputType::value("Radius Y", TaggedValue::F64(25.), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::VectorData)],
			properties: node_properties::ellipse_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Rectangle",
			category: "Vector",
			implementation: DocumentNodeImplementation::proto("graphene_core::vector::generator_nodes::RectangleGenerator<_, _, _, _, _>"),
			inputs: vec![
				DocumentInputType::none(),
				DocumentInputType::value("Size X", TaggedValue::F64(100.), false),
				DocumentInputType::value("Size Y", TaggedValue::F64(100.), false),
				DocumentInputType::value("Individual Corner Radii", TaggedValue::Bool(false), false),
				DocumentInputType::value("Corner Radius", TaggedValue::F64(0.), false),
				DocumentInputType::value("Clamped", TaggedValue::Bool(true), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::VectorData)],
			properties: node_properties::rectangle_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Regular Polygon",
			category: "Vector",
			implementation: DocumentNodeImplementation::proto("graphene_core::vector::generator_nodes::RegularPolygonGenerator<_, _>"),
			inputs: vec![
				DocumentInputType::none(),
				DocumentInputType::value("Sides", TaggedValue::U32(6), false),
				DocumentInputType::value("Radius", TaggedValue::F64(50.), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::VectorData)],
			properties: node_properties::regular_polygon_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Star",
			category: "Vector",
			implementation: DocumentNodeImplementation::proto("graphene_core::vector::generator_nodes::StarGenerator<_, _, _>"),
			inputs: vec![
				DocumentInputType::none(),
				DocumentInputType::value("Sides", TaggedValue::U32(5), false),
				DocumentInputType::value("Radius", TaggedValue::F64(50.), false),
				DocumentInputType::value("Inner Radius", TaggedValue::F64(25.), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::VectorData)],
			properties: node_properties::star_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Line",
			category: "Vector",
			implementation: DocumentNodeImplementation::proto("graphene_core::vector::generator_nodes::LineGenerator<_, _>"),
			inputs: vec![
				DocumentInputType::none(),
				DocumentInputType::value("Start", TaggedValue::DVec2(DVec2::new(0., -50.)), false),
				DocumentInputType::value("End", TaggedValue::DVec2(DVec2::new(0., 50.)), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::VectorData)],
			properties: node_properties::line_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Spline",
			category: "Vector",
			implementation: DocumentNodeImplementation::proto("graphene_core::vector::generator_nodes::SplineGenerator<_>"),
			inputs: vec![
				DocumentInputType::none(),
				DocumentInputType::value("Points", TaggedValue::VecDVec2(vec![DVec2::new(0., -50.), DVec2::new(25., 0.), DVec2::new(0., 50.)]), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::VectorData)],
			properties: node_properties::spline_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Shape",
			category: "Vector",
			implementation: DocumentNodeImplementation::proto("graphene_core::vector::generator_nodes::PathGenerator<_>"),
			inputs: vec![
				DocumentInputType::value("Path Data", TaggedValue::Subpaths(vec![]), false),
				DocumentInputType::value("Colinear Manipulators", TaggedValue::PointIds(vec![]), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::VectorData)],
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Path",
			category: "Vector",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![NodeInput::node(NodeId(1), 0)],
				nodes: vec![
					DocumentNode {
						inputs: vec![NodeInput::network(concrete!(VectorData), 0)],
						..monitor_node()
					},
					DocumentNode {
						name: "Path Modify".to_string(),
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
				DocumentInputType::value("Vector Data", TaggedValue::VectorData(VectorData::empty()), true),
				DocumentInputType::value("Modification", TaggedValue::VectorModification(Default::default()), false),
			],
			outputs: vec![DocumentOutputType::new("Vector Data", FrontendGraphDataType::VectorData)],
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Sample",
			category: "Structural",
			implementation: DocumentNodeImplementation::proto("graphene_std::raster::SampleNode<_>"),
			manual_composition: Some(concrete!(Footprint)),
			inputs: vec![DocumentInputType::value("Raster Data", TaggedValue::ImageFrame(ImageFrame::empty()), true)],
			outputs: vec![DocumentOutputType::new("Raster", FrontendGraphDataType::Raster)],
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Mandelbrot",
			category: "Generators",
			implementation: DocumentNodeImplementation::proto("graphene_std::raster::MandelbrotNode"),
			manual_composition: Some(concrete!(Footprint)),
			inputs: vec![],
			outputs: vec![DocumentOutputType::new("Raster", FrontendGraphDataType::Raster)],
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Cull",
			category: "Vector",
			implementation: DocumentNodeImplementation::proto("graphene_core::transform::CullNode<_>"),
			manual_composition: Some(concrete!(Footprint)),
			inputs: vec![DocumentInputType::value("Vector Data", TaggedValue::VectorData(VectorData::empty()), true)],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::VectorData)],
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Text",
			category: "Vector",
			implementation: DocumentNodeImplementation::proto("graphene_core::text::TextGeneratorNode<_, _, _>"),
			inputs: vec![
				DocumentInputType::new("Editor API", FrontendGraphDataType::General, NodeInput::scope("editor-api")),
				DocumentInputType::value("Text", TaggedValue::String("Lorem ipsum".to_string()), false),
				DocumentInputType::value(
					"Font",
					TaggedValue::Font(Font::new(graphene_core::consts::DEFAULT_FONT_FAMILY.into(), graphene_core::consts::DEFAULT_FONT_STYLE.into())),
					false,
				),
				DocumentInputType::value("Size", TaggedValue::F64(24.), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::VectorData)],
			properties: node_properties::text_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Transform",
			category: "Transform",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![NodeInput::node(NodeId(1), 0)],
				nodes: [
					DocumentNode {
						inputs: vec![NodeInput::network(concrete!(VectorData), 0)],
						..monitor_node()
					},
					DocumentNode {
						name: "Transform".to_string(),
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
			inputs: vec![
				DocumentInputType::value("Vector Data", TaggedValue::VectorData(VectorData::empty()), true),
				DocumentInputType::value("Translation", TaggedValue::DVec2(DVec2::ZERO), false),
				DocumentInputType::value("Rotation", TaggedValue::F64(0.), false),
				DocumentInputType::value("Scale", TaggedValue::DVec2(DVec2::ONE), false),
				DocumentInputType::value("Skew", TaggedValue::DVec2(DVec2::ZERO), false),
				DocumentInputType::value("Pivot", TaggedValue::DVec2(DVec2::splat(0.5)), false),
			],
			outputs: vec![DocumentOutputType::new("Data", FrontendGraphDataType::VectorData)],
			properties: node_properties::transform_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "SetTransform",
			category: "Transform",
			implementation: DocumentNodeImplementation::proto("graphene_core::transform::SetTransformNode<_>"),
			inputs: vec![
				DocumentInputType::value("Data", TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
				DocumentInputType::value("Transform", TaggedValue::DAffine2(DAffine2::IDENTITY), true),
			],
			outputs: vec![DocumentOutputType::new("Data", FrontendGraphDataType::VectorData)],
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Fill",
			category: "Vector",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![NodeInput::node(NodeId(0), 0)],
				nodes: vec![DocumentNode {
					name: "Set Fill".to_string(),
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
				DocumentInputType::value("Vector Data", TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
				DocumentInputType::value("Fill", TaggedValue::Fill(vector::style::Fill::Solid(Color::BLACK)), false),
				// These backup values aren't exposed to the user, but are used to store the previous fill choices so the user can flip back from Solid to Gradient (or vice versa) without losing their settings
				DocumentInputType::value("Backup Color", TaggedValue::OptionalColor(Some(Color::BLACK)), false),
				DocumentInputType::value("Backup Gradient", TaggedValue::Gradient(Default::default()), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::VectorData)],
			properties: node_properties::fill_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Stroke",
			category: "Vector",
			implementation: DocumentNodeImplementation::proto("graphene_core::vector::SetStrokeNode<_, _, _, _, _, _, _>"),
			inputs: vec![
				DocumentInputType::value("Vector Data", TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
				DocumentInputType::value("Color", TaggedValue::OptionalColor(Some(Color::BLACK)), false),
				DocumentInputType::value("Weight", TaggedValue::F64(0.), false),
				DocumentInputType::value("Dash Lengths", TaggedValue::VecF64(Vec::new()), false),
				DocumentInputType::value("Dash Offset", TaggedValue::F64(0.), false),
				DocumentInputType::value("Line Cap", TaggedValue::LineCap(graphene_core::vector::style::LineCap::default()), false),
				DocumentInputType::value("Line Join", TaggedValue::LineJoin(graphene_core::vector::style::LineJoin::default()), false),
				DocumentInputType::value("Miter Limit", TaggedValue::F64(4.), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::VectorData)],
			properties: node_properties::stroke_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Bounding Box",
			category: "Vector",
			implementation: DocumentNodeImplementation::proto("graphene_core::vector::BoundingBoxNode"),
			inputs: vec![DocumentInputType::value("Vector Data", TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true)],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::VectorData)],
			properties: node_properties::node_no_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Solidify Stroke",
			category: "Vector",
			implementation: DocumentNodeImplementation::proto("graphene_core::vector::SolidifyStrokeNode"),
			inputs: vec![DocumentInputType::value("Vector Data", TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true)],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::VectorData)],
			properties: node_properties::node_no_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Repeat",
			category: "Vector",
			implementation: DocumentNodeImplementation::proto("graphene_core::vector::RepeatNode<_, _, _>"),
			inputs: vec![
				DocumentInputType::value("Instance", TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
				DocumentInputType::value("Direction", TaggedValue::DVec2((100., 100.).into()), false),
				DocumentInputType::value("Angle", TaggedValue::F64(0.), false),
				DocumentInputType::value("Instances", TaggedValue::U32(5), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::VectorData)],
			properties: node_properties::repeat_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Circular Repeat",
			category: "Vector",
			implementation: DocumentNodeImplementation::proto("graphene_core::vector::CircularRepeatNode<_, _, _>"),
			inputs: vec![
				DocumentInputType::value("Instance", TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
				DocumentInputType::value("Angle Offset", TaggedValue::F64(0.), false),
				DocumentInputType::value("Radius", TaggedValue::F64(5.), false),
				DocumentInputType::value("Instances", TaggedValue::U32(5), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::VectorData)],
			properties: node_properties::circular_repeat_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Binary Boolean Operation",
			category: "Vector",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![NodeInput::node(NodeId(1), 0)],
				nodes: [
					DocumentNode {
						name: "BinaryBooleanOperation".to_string(),
						inputs: vec![
							NodeInput::network(concrete!(graphene_core::vector::VectorData), 0),
							NodeInput::network(concrete!(graphene_core::vector::VectorData), 1),
							NodeInput::network(concrete!(vector::misc::BooleanOperation), 2),
						],
						implementation: DocumentNodeImplementation::proto("graphene_std::vector::BinaryBooleanOperationNode<_, _>"),
						metadata: DocumentNodeMetadata { position: glam::IVec2::new(-17, -3) },
						..Default::default()
					},
					DocumentNode {
						name: "MemoizeImpure".to_string(),
						inputs: vec![NodeInput::node(NodeId(0), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::ImpureMemoNode<_, _, _>")),
						metadata: DocumentNodeMetadata { position: glam::IVec2::new(-10, -3) },
						manual_composition: Some(concrete!(Footprint)),
						..Default::default()
					},
				]
				.into_iter()
				.enumerate()
				.map(|(id, node)| (NodeId(id as u64), node))
				.collect(),
				imports_metadata: (NodeId(generate_uuid()), (-25, -4).into()),
				exports_metadata: (NodeId(generate_uuid()), (-2, -4).into()),
				..Default::default()
			}),
			inputs: vec![
				DocumentInputType::value("Upper Vector Data", TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
				DocumentInputType::value("Lower Vector Data", TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
				DocumentInputType::value("Operation", TaggedValue::BooleanOperation(vector::misc::BooleanOperation::Union), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::VectorData)],
			properties: node_properties::binary_boolean_operation_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Boolean Operation",
			category: "Vector",
			is_layer: true,
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![NodeInput::node(NodeId(5), 0)],
				nodes: [
					// Primary (bottom) input type coercion
					DocumentNode {
						name: "ToGraphicGroup".to_string(),
						inputs: vec![NodeInput::network(generic!(T), 0)],
						implementation: DocumentNodeImplementation::proto("graphene_core::ToGraphicGroupNode"),
						metadata: DocumentNodeMetadata { position: glam::IVec2::new(-9, -3) }, // To Graphic Group
						..Default::default()
					},
					// Secondary (left) input type coercion
					DocumentNode {
						name: "BooleanOperation".to_string(),
						inputs: vec![NodeInput::network(generic!(T), 1), NodeInput::network(concrete!(vector::misc::BooleanOperation), 2)],
						implementation: DocumentNodeImplementation::proto("graphene_std::vector::BooleanOperationNode<_>"),
						metadata: DocumentNodeMetadata { position: glam::IVec2::new(-16, -1) },
						..Default::default()
					},
					DocumentNode {
						name: "ToGraphicElement".to_string(),
						inputs: vec![NodeInput::node(NodeId(1), 0)],
						implementation: DocumentNodeImplementation::proto("graphene_core::ToGraphicElementNode"),
						metadata: DocumentNodeMetadata { position: glam::IVec2::new(-9, -1) }, // To Graphic Element
						..Default::default()
					},
					DocumentNode {
						name: "MemoizeImpure".to_string(),
						inputs: vec![NodeInput::node(NodeId(2), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::ImpureMemoNode<_, _, _>")),
						metadata: DocumentNodeMetadata { position: glam::IVec2::new(-2, -1) },
						manual_composition: Some(concrete!(Footprint)),
						..Default::default()
					},
					// The monitor node is used to display a thumbnail in the UI
					DocumentNode {
						inputs: vec![NodeInput::node(NodeId(3), 0)],
						metadata: DocumentNodeMetadata { position: glam::IVec2::new(5, -1) }, // Monitor
						..monitor_node()
					},
					DocumentNode {
						name: "ConstructLayer".to_string(),
						manual_composition: Some(concrete!(Footprint)),
						inputs: vec![NodeInput::node(NodeId(0), 0), NodeInput::node(NodeId(4), 0)],
						implementation: DocumentNodeImplementation::proto("graphene_core::ConstructLayerNode<_, _>"),
						metadata: DocumentNodeMetadata { position: glam::IVec2::new(12, -3) }, // ConstructLayer
						..Default::default()
					},
				]
				.into_iter()
				.enumerate()
				.map(|(id, node)| (NodeId(id as u64), node))
				.collect(),
				imports_metadata: (NodeId(generate_uuid()), (-24, -4).into()),
				exports_metadata: (NodeId(generate_uuid()), (19, -4).into()),
				..Default::default()
			}),
			inputs: vec![
				DocumentInputType::value("Graphical Data", TaggedValue::GraphicGroup(GraphicGroup::EMPTY), true),
				DocumentInputType::value("Vector Data", TaggedValue::GraphicGroup(GraphicGroup::EMPTY), true),
				DocumentInputType::value("Operation", TaggedValue::BooleanOperation(vector::misc::BooleanOperation::Union), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Graphic)],
			properties: node_properties::boolean_operation_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Copy to Points",
			category: "Vector",
			// TODO: Wrap this implementation with a document node that has a cache node so the output is cached?
			implementation: DocumentNodeImplementation::proto("graphene_core::vector::CopyToPoints<_, _, _, _, _, _>"),
			manual_composition: Some(concrete!(Footprint)),
			inputs: vec![
				DocumentInputType::value("Points", TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
				DocumentInputType::value("Instance", TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
				DocumentInputType::value("Random Scale Min", TaggedValue::F64(1.), false),
				DocumentInputType::value("Random Scale Max", TaggedValue::F64(1.), false),
				DocumentInputType::value("Random Scale Bias", TaggedValue::F64(0.), false),
				DocumentInputType::value("Random Rotation", TaggedValue::F64(0.), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::VectorData)],
			properties: node_properties::copy_to_points_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Sample Points",
			category: "Vector",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![NodeInput::node(NodeId(2), 0)], // Taken from output 0 of Sample Points
				nodes: [
					DocumentNode {
						name: "Lengths of Segments of Subpaths".to_string(),
						inputs: vec![NodeInput::network(concrete!(graphene_core::vector::VectorData), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::vector::LengthsOfSegmentsOfSubpaths")),
						..Default::default()
					},
					DocumentNode {
						name: "Sample Points".to_string(),
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
						name: "MemoizeImpure".to_string(),
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
				DocumentInputType::value("Vector Data", TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
				DocumentInputType::value("Spacing", TaggedValue::F64(100.), false),
				DocumentInputType::value("Start Offset", TaggedValue::F64(0.), false),
				DocumentInputType::value("Stop Offset", TaggedValue::F64(0.), false),
				DocumentInputType::value("Adaptive Spacing", TaggedValue::Bool(false), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::VectorData)],
			properties: node_properties::sample_points_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Poisson-Disk Points",
			category: "Vector",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				exports: vec![NodeInput::node(NodeId(1), 0)],
				nodes: [
					DocumentNode {
						name: "Poisson-Disk Points".to_string(),
						inputs: vec![NodeInput::network(concrete!(graphene_core::vector::VectorData), 0), NodeInput::network(concrete!(f64), 1)],
						implementation: DocumentNodeImplementation::proto("graphene_core::vector::PoissonDiskPoints<_>"),
						..Default::default()
					},
					DocumentNode {
						name: "MemoizeImpure".to_string(),
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
				DocumentInputType::value("Vector Data", TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
				DocumentInputType::value("Separation Disk Diameter", TaggedValue::F64(10.), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::VectorData)],
			properties: node_properties::poisson_disk_points_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Splines from Points",
			category: "Vector",
			implementation: DocumentNodeImplementation::proto("graphene_core::vector::SplinesFromPointsNode"),
			inputs: vec![DocumentInputType::value("Vector Data", TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true)],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::VectorData)],
			properties: node_properties::node_no_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Area",
			category: "Vector",
			implementation: DocumentNodeImplementation::proto("graphene_core::vector::AreaNode<_>"),
			inputs: vec![DocumentInputType::value("Vector Data", TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true)],
			outputs: vec![DocumentOutputType::new("Output", FrontendGraphDataType::Number)],
			properties: node_properties::node_no_properties,
			manual_composition: Some(concrete!(())),
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Centroid",
			category: "Vector",
			implementation: DocumentNodeImplementation::proto("graphene_core::vector::CentroidNode<_, _>"),
			inputs: vec![
				DocumentInputType::value("Vector Data", TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
				DocumentInputType::value("Centroid Type", TaggedValue::CentroidType(graphene_core::vector::misc::CentroidType::Area), false),
			],
			outputs: vec![DocumentOutputType::new("Output", FrontendGraphDataType::Number)],
			properties: node_properties::centroid_properties,
			manual_composition: Some(concrete!(())),
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Morph",
			category: "Vector",
			implementation: DocumentNodeImplementation::proto("graphene_core::vector::MorphNode<_, _, _, _>"),
			inputs: vec![
				DocumentInputType::value("Source", TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
				DocumentInputType::value("Target", TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
				DocumentInputType::value("Start Index", TaggedValue::U32(0), false),
				DocumentInputType::value("Time", TaggedValue::F64(0.5), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::VectorData)],
			manual_composition: Some(concrete!(Footprint)),
			properties: node_properties::morph_properties,
			..Default::default()
		},
		// TODO: This needs to work with resolution-aware (raster with footprint, post-Cull node) data.
		DocumentNodeDefinition {
			name: "Image Segmentation",
			category: "Raster",
			implementation: DocumentNodeImplementation::proto("graphene_std::image_segmentation::ImageSegmentationNode<_>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Mask", TaggedValue::ImageFrame(ImageFrame::empty()), true),
			],
			outputs: vec![DocumentOutputType::new("Segments", FrontendGraphDataType::Raster)],
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Index",
			category: "Raster",
			implementation: DocumentNodeImplementation::proto("graphene_core::raster::IndexNode<_>"),
			inputs: vec![
				DocumentInputType::value("Segmentation", TaggedValue::Segments(vec![ImageFrame::empty()]), true),
				DocumentInputType::value("Index", TaggedValue::U32(0), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::index_properties,
			..Default::default()
		},
		// Applies the given color to each pixel of an image but maintains the alpha value
		DocumentNodeDefinition {
			name: "Color Fill",
			category: "Raster",
			implementation: DocumentNodeImplementation::proto("graphene_core::raster::adjustments::ColorFillNode<_>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Color", TaggedValue::Color(Color::BLACK), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::color_fill_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Color Overlay",
			category: "Raster",
			implementation: DocumentNodeImplementation::proto("graphene_core::raster::adjustments::ColorOverlayNode<_, _, _>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Color", TaggedValue::Color(Color::BLACK), false),
				DocumentInputType::value("Blend Mode", TaggedValue::BlendMode(BlendMode::Normal), false),
				DocumentInputType::value("Opacity", TaggedValue::F64(100.), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::color_overlay_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Image Color Palette",
			category: "Raster",
			implementation: DocumentNodeImplementation::proto("graphene_std::image_color_palette::ImageColorPaletteNode<_>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Max Size", TaggedValue::U32(8), true),
			],
			outputs: vec![DocumentOutputType::new("Colors", FrontendGraphDataType::General)],
			properties: node_properties::image_color_palette,
			..Default::default()
		},
	]
}

pub static IMAGINATE_NODE: Lazy<DocumentNodeDefinition> = Lazy::new(|| DocumentNodeDefinition {
	name: "Imaginate",
	category: "Image Synthesis",
	implementation: DocumentNodeImplementation::Network(NodeNetwork {
		exports: vec![NodeInput::node(NodeId(1), 0)],
		nodes: [
			(
				NodeId(0),
				DocumentNode {
					inputs: vec![NodeInput::network(concrete!(ImageFrame<Color>), 0)],
					..monitor_node()
				},
			),
			(
				NodeId(1),
				DocumentNode {
					name: "Imaginate".into(),
					inputs: vec![
						NodeInput::node(NodeId(0), 0),
						NodeInput::scope("editor-api"),
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
					],
					implementation: DocumentNodeImplementation::proto("graphene_std::raster::ImaginateNode<_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _>"),
					..Default::default()
				},
			),
		]
		.into(),
		..Default::default()
	}),
	inputs: vec![
		DocumentInputType::value("Input Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
		DocumentInputType {
			name: "Editor Api",
			data_type: FrontendGraphDataType::General,
			default: NodeInput::scope("editor-api"),
		},
		DocumentInputType::value("Controller", TaggedValue::ImaginateController(Default::default()), false),
		DocumentInputType::value("Seed", TaggedValue::U64(0), false), // Remember to keep index used in `ImaginateRandom` updated with this entry's index
		DocumentInputType::value("Resolution", TaggedValue::OptionalDVec2(None), false),
		DocumentInputType::value("Samples", TaggedValue::U32(30), false),
		DocumentInputType::value("Sampling Method", TaggedValue::ImaginateSamplingMethod(ImaginateSamplingMethod::EulerA), false),
		DocumentInputType::value("Prompt Guidance", TaggedValue::F64(7.5), false),
		DocumentInputType::value("Prompt", TaggedValue::String(String::new()), false),
		DocumentInputType::value("Negative Prompt", TaggedValue::String(String::new()), false),
		DocumentInputType::value("Adapt Input Image", TaggedValue::Bool(false), false),
		DocumentInputType::value("Image Creativity", TaggedValue::F64(66.), false),
		DocumentInputType::value("Inpaint", TaggedValue::Bool(true), false),
		DocumentInputType::value("Mask Blur", TaggedValue::F64(4.), false),
		DocumentInputType::value("Mask Starting Fill", TaggedValue::ImaginateMaskStartingFill(ImaginateMaskStartingFill::Fill), false),
		DocumentInputType::value("Improve Faces", TaggedValue::Bool(false), false),
		DocumentInputType::value("Tiling", TaggedValue::Bool(false), false),
	],
	outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
	properties: node_properties::imaginate_properties,
	..Default::default()
});

pub fn resolve_document_node_type(name: &str) -> Option<&DocumentNodeDefinition> {
	DOCUMENT_NODE_TYPES.iter().find(|node| node.name == name)
}

pub fn collect_node_types() -> Vec<FrontendNodeType> {
	DOCUMENT_NODE_TYPES
		.iter()
		.filter(|node_type| !node_type.category.eq_ignore_ascii_case("ignore"))
		.map(|node_type| FrontendNodeType::new(node_type.name, node_type.category))
		.collect()
}

impl DocumentNodeDefinition {
	/// Converts the [DocumentNodeDefinition] type to a [DocumentNode], based on the inputs from the graph (which must be the correct length) and the metadata
	pub fn to_document_node(&self, inputs: impl IntoIterator<Item = NodeInput>, metadata: DocumentNodeMetadata) -> DocumentNode {
		let inputs: Vec<_> = inputs.into_iter().collect();
		assert_eq!(inputs.len(), self.inputs.len(), "Inputs passed from the graph must be equal to the number required");

		DocumentNode {
			name: self.name.to_string(),
			is_layer: self.is_layer,
			inputs,
			manual_composition: self.manual_composition.clone(),
			has_primary_output: self.has_primary_output,
			implementation: self.implementation.clone(),
			metadata,
			..Default::default()
		}
	}

	/// Converts the [DocumentNodeDefinition] type to a [DocumentNode], using the provided `input_override` and falling back to the default inputs.
	/// `input_override` does not have to be the correct length.
	pub fn to_document_node_default_inputs(&self, input_override: impl IntoIterator<Item = Option<NodeInput>>, metadata: DocumentNodeMetadata) -> DocumentNode {
		let mut input_override = input_override.into_iter();
		let inputs = self.inputs.iter().map(|default| input_override.next().unwrap_or_default().unwrap_or_else(|| default.default.clone()));
		self.to_document_node(inputs, metadata)
	}

	/// Converts the [DocumentNodeDefinition] type to a [DocumentNode], completely default.
	pub fn default_document_node(&self) -> DocumentNode {
		self.to_document_node(self.inputs.iter().map(|input| input.default.clone()), DocumentNodeMetadata::default())
	}
}

pub fn wrap_network_in_scope(mut network: NodeNetwork, editor_api: Arc<WasmEditorApi>) -> NodeNetwork {
	network.generate_node_paths(&[]);

	let inner_network = DocumentNode {
		name: "Scope".to_string(),
		implementation: DocumentNodeImplementation::Network(network),
		inputs: vec![NodeInput::node(NodeId(0), 1)],
		metadata: DocumentNodeMetadata::position((-10, 0)),
		..Default::default()
	};

	let render_node = graph_craft::document::DocumentNode {
		name: "Output".into(),
		inputs: vec![NodeInput::node(NodeId(0), 0), NodeInput::node(NodeId(2), 0)],
		implementation: graph_craft::document::DocumentNodeImplementation::Network(NodeNetwork {
			exports: vec![NodeInput::node(NodeId(2), 0)],
			nodes: [
				DocumentNode {
					name: "Create Canvas".to_string(),
					inputs: vec![NodeInput::scope("editor-api")],
					manual_composition: Some(concrete!(Footprint)),
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("wgpu_executor::CreateGpuSurfaceNode<_>")),
					skip_deduplication: true,
					..Default::default()
				},
				DocumentNode {
					name: "Cache".to_string(),
					manual_composition: Some(concrete!(Footprint)),
					inputs: vec![NodeInput::node(NodeId(0), 0)],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::ImpureMemoNode<_, _, _>")),
					..Default::default()
				},
				// TODO: Add conversion step
				DocumentNode {
					name: "RenderNode".to_string(),
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
		metadata: DocumentNodeMetadata::position((-3, 0)),
		..Default::default()
	};

	// wrap the inner network in a scope
	let nodes = vec![
		inner_network,
		render_node,
		DocumentNode {
			name: "Editor Api".into(),
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::IdentityNode"),
			inputs: vec![NodeInput::value(TaggedValue::EditorApi(editor_api), false)],
			..Default::default()
		},
	];

	NodeNetwork {
		exports: vec![NodeInput::node(NodeId(1), 0)],
		nodes: nodes.into_iter().enumerate().map(|(id, node)| (NodeId(id as u64), node)).collect(),
		scope_injections: [("editor-api".to_string(), (NodeId(2), concrete!(&WasmEditorApi)))].into_iter().collect(),
		..Default::default()
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

// Unused
// pub fn new_text_network(text: String, font: Font, size: f64) -> NodeNetwork {
// 	let text_generator = resolve_document_node_type("Text").expect("Text node does not exist");
// 	let transform = resolve_document_node_type("Transform").expect("Transform node does not exist");
// 	let fill = resolve_document_node_type("Fill").expect("Fill node does not exist");
// 	let stroke = resolve_document_node_type("Stroke").expect("Stroke node does not exist");
// 	let output = resolve_document_node_type("Output").expect("Output node does not exist");

// 	let mut network = NodeNetwork { ..Default::default() };
// 	network.push_node_to_document_network(text_generator.to_document_node(
// 		[
// 			NodeInput::scope("editor-api"),
// 			NodeInput::value(TaggedValue::String(text), false),
// 			NodeInput::value(TaggedValue::Font(font), false),
// 			NodeInput::value(TaggedValue::F64(size), false),
// 		],
// 		DocumentNodeMetadata::position((0, 4)),
// 	));
// 	network.push_node_to_document_network(transform.to_document_node_default_inputs([None], Default::default()));
// 	network.push_node_to_document_network(fill.to_document_node_default_inputs([None], Default::default()));
// 	network.push_node_to_document_network(stroke.to_document_node_default_inputs([None], Default::default()));
// 	network.push_node_to_document_network(output.to_document_node_default_inputs([None], Default::default()));
// 	network
// }
