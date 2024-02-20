use super::{node_properties, FrontendGraphDataType, FrontendNodeType};
use crate::consts::{DEFAULT_FONT_FAMILY, DEFAULT_FONT_STYLE};
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::utility_types::document_metadata::DocumentMetadata;
use crate::messages::portfolio::utility_types::PersistentData;
use crate::messages::prelude::Message;
use crate::node_graph_executor::NodeGraphExecutor;

use graph_craft::concrete;
use graph_craft::document::*;
use graph_craft::document::{value::*, DocumentNodeMetadata};
use graph_craft::imaginate_input::ImaginateSamplingMethod;
use graph_craft::ProtoNodeIdentifier;
#[cfg(feature = "gpu")]
use graphene_core::application_io::SurfaceHandle;
use graphene_core::raster::brush_cache::BrushCache;
use graphene_core::raster::{
	BlendMode, CellularDistanceFunction, CellularReturnType, Color, DomainWarpType, FractalType, Image, ImageFrame, LuminanceCalculation, NoiseType, RedGreenBlue, RelativeAbsolute,
	SelectiveColorChoice,
};
use graphene_core::text::Font;
use graphene_core::transform::Footprint;
use graphene_core::vector::VectorData;
use graphene_core::*;

#[cfg(feature = "gpu")]
use gpu_executor::*;
use graphene_std::wasm_application_io::WasmEditorApi;
use once_cell::sync::Lazy;
use std::collections::VecDeque;
#[cfg(feature = "gpu")]
use wgpu_executor::WgpuExecutor;

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
		let data_type = FrontendGraphDataType::with_tagged_value(&tagged_value);
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
	pub network: &'a NodeNetwork,
	pub metadata: &'a mut DocumentMetadata,
}

/// Acts as a description for a [DocumentNode] before it gets instantiated as one.
#[derive(Clone)]
pub struct DocumentNodeDefinition {
	pub name: &'static str,
	pub category: &'static str,
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
			outputs: vec![DocumentOutputType::new("Out", FrontendGraphDataType::Boolean)],
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
			outputs: vec![DocumentOutputType::new("Out", FrontendGraphDataType::Color)],
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
			name: "Layer",
			category: "General",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				inputs: vec![NodeId(0), NodeId(2)],
				outputs: vec![NodeOutput::new(NodeId(2), 0)],
				nodes: [
					(
						NodeId(0),
						DocumentNode {
							name: "To Graphic Element".to_string(),
							inputs: vec![NodeInput::Network(generic!(T))],
							implementation: DocumentNodeImplementation::proto("graphene_core::ToGraphicElementNode"),
							..Default::default()
						},
					),
					// The monitor node is used to display a thumbnail in the UI.
					(
						NodeId(1),
						DocumentNode {
							inputs: vec![NodeInput::node(NodeId(0), 0)],
							..monitor_node()
						},
					),
					(
						NodeId(2),
						DocumentNode {
							name: "ConstructLayer".to_string(),
							manual_composition: Some(concrete!(Footprint)),
							inputs: vec![
								NodeInput::node(NodeId(1), 0),
								NodeInput::Network(graphene_core::Type::Fn(Box::new(concrete!(Footprint)), Box::new(concrete!(graphene_core::GraphicGroup)))),
							],
							implementation: DocumentNodeImplementation::proto("graphene_core::ConstructLayerNode<_, _>"),
							..Default::default()
						},
					),
				]
				.into(),
				..Default::default()
			}),
			inputs: vec![
				DocumentInputType::value("Graphical Data", TaggedValue::GraphicGroup(GraphicGroup::EMPTY), true),
				DocumentInputType::value("Stack", TaggedValue::GraphicGroup(GraphicGroup::EMPTY), true),
			],
			outputs: vec![DocumentOutputType::new("Out", FrontendGraphDataType::GraphicGroup)],
			properties: node_properties::layer_no_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Artboard",
			category: "General",
			implementation: DocumentNodeImplementation::proto("graphene_core::ConstructArtboardNode<_, _, _, _, _>"),
			inputs: vec![
				DocumentInputType::value("Graphic Group", TaggedValue::GraphicGroup(GraphicGroup::EMPTY), true),
				DocumentInputType::value("Location", TaggedValue::IVec2(glam::IVec2::ZERO), false),
				DocumentInputType::value("Dimensions", TaggedValue::IVec2(glam::IVec2::new(1920, 1080)), false),
				DocumentInputType::value("Background", TaggedValue::Color(Color::WHITE), false),
				DocumentInputType::value("Clip", TaggedValue::Bool(false), false),
			],
			outputs: vec![DocumentOutputType::new("Out", FrontendGraphDataType::Artboard)],
			properties: node_properties::artboard_properties,
			manual_composition: Some(concrete!(Footprint)),
			..Default::default()
		},
		// TODO: Does this need an internal Cull node to be added to its implementation?
		DocumentNodeDefinition {
			name: "Input Frame",
			category: "Ignore",
			implementation: DocumentNodeImplementation::proto("graphene_core::ExtractImageFrame"),
			inputs: vec![DocumentInputType {
				name: "In",
				data_type: FrontendGraphDataType::General,
				default: NodeInput::Network(concrete!(WasmEditorApi)),
			}],
			outputs: vec![DocumentOutputType {
				name: "Image Frame",
				data_type: FrontendGraphDataType::Raster,
			}],
			properties: node_properties::node_no_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Load Image",
			category: "Structural",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				inputs: vec![NodeId(0), NodeId(0)],
				outputs: vec![NodeOutput::new(NodeId(2), 0)],
				nodes: [
					DocumentNode {
						name: "Load Resource".to_string(),
						inputs: vec![NodeInput::Network(concrete!(WasmEditorApi)), NodeInput::Network(concrete!(String))],
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
					default: NodeInput::Network(concrete!(WasmEditorApi)),
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
				inputs: vec![NodeId(0)],
				outputs: vec![NodeOutput::new(NodeId(1), 0)],
				nodes: [
					DocumentNode {
						name: "Create Canvas".to_string(),
						inputs: vec![NodeInput::Network(concrete!(WasmEditorApi))],
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
			inputs: vec![DocumentInputType {
				name: "In",
				data_type: FrontendGraphDataType::General,
				default: NodeInput::Network(concrete!(WasmEditorApi)),
			}],
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
				inputs: vec![NodeId(0), NodeId(2)],
				outputs: vec![NodeOutput::new(NodeId(3), 0)],
				nodes: [
					DocumentNode {
						name: "Convert Image Frame".to_string(),
						inputs: vec![NodeInput::Network(concrete!(ImageFrame<Color>))],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<_, ImageFrame<SRGBA8>>")),
						..Default::default()
					},
					DocumentNode {
						name: "Create Canvas".to_string(),
						inputs: vec![NodeInput::Network(concrete!(WasmEditorApi))],
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
			inputs: vec![
				DocumentInputType {
					name: "In",
					data_type: FrontendGraphDataType::Raster,
					default: NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
				},
				DocumentInputType {
					name: "In",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::Network(concrete!(WasmEditorApi)),
				},
			],
			outputs: vec![DocumentOutputType {
				name: "Canvas",
				data_type: FrontendGraphDataType::General,
			}],
			..Default::default()
		},
		DocumentNodeDefinition {
			// This essentially builds the concept of a closure where we store variables (`let` bindings) so they can be accessed within this scope.
			name: "Begin Scope",
			category: "Ignore",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				inputs: vec![NodeId(0)],
				outputs: vec![NodeOutput::new(NodeId(1), 0), NodeOutput::new(NodeId(2), 0)],
				nodes: [
					DocumentNode {
						name: "SetNode".to_string(),
						manual_composition: Some(concrete!(WasmEditorApi)),
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::SomeNode")),
						..Default::default()
					},
					DocumentNode {
						name: "LetNode".to_string(),
						inputs: vec![NodeInput::node(NodeId(0), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::LetNode<_>")),
						..Default::default()
					},
					DocumentNode {
						name: "RefNode".to_string(),
						manual_composition: Some(concrete!(())),
						inputs: vec![NodeInput::lambda(NodeId(1), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::memo::RefNode<_, _>")),
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
				default: NodeInput::Network(concrete!(WasmEditorApi)),
			}],
			outputs: vec![
				DocumentOutputType {
					name: "Scope",
					data_type: FrontendGraphDataType::General,
				},
				DocumentOutputType {
					name: "Binding",
					data_type: FrontendGraphDataType::Raster,
				},
			],
			properties: |_document_node, _node_id, _context| node_properties::string_properties("Binds the input in a local scope as a variable"),
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "End Scope",
			category: "Ignore",
			implementation: DocumentNodeImplementation::proto("graphene_core::memo::EndLetNode<_, _>"),
			inputs: vec![
				DocumentInputType {
					name: "Scope",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::value(TaggedValue::None, true),
				},
				DocumentInputType {
					name: "Data",
					data_type: FrontendGraphDataType::Raster,
					default: NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
				},
			],
			outputs: vec![DocumentOutputType {
				name: "Frame",
				data_type: FrontendGraphDataType::Raster,
			}],
			properties: |_document_node, _node_id, _context| node_properties::string_properties("Consumes the scope opened by the Begin Scope node and evaluates the contained node network"),
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Output",
			category: "Ignore",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				inputs: vec![NodeId(3), NodeId(0)],
				outputs: vec![NodeOutput::new(NodeId(4), 0)],
				nodes: [
					DocumentNode {
						name: "EditorApi".to_string(),
						inputs: vec![NodeInput::Network(concrete!(WasmEditorApi))],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IdentityNode")),
						..Default::default()
					},
					DocumentNode {
						name: "Create Canvas".to_string(),
						inputs: vec![NodeInput::node(NodeId(0), 0)],
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
						name: "Conversion".to_string(),
						inputs: vec![NodeInput::Network(graphene_core::Type::Fn(Box::new(concrete!(Footprint)), Box::new(generic!(T))))],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<_, GraphicGroup>")),
						..Default::default()
					},
					DocumentNode {
						name: "RenderNode".to_string(),
						inputs: vec![NodeInput::node(NodeId(0), 0), NodeInput::node(NodeId(3), 0), NodeInput::node(NodeId(2), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::wasm_application_io::RenderNode<_, _>")),
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
					name: "Output",
					data_type: FrontendGraphDataType::Raster,
					default: NodeInput::value(TaggedValue::GraphicGroup(GraphicGroup::default()), true),
				},
				DocumentInputType {
					name: "In",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::Network(concrete!(WasmEditorApi)),
				},
			],
			outputs: vec![],
			properties: node_properties::output_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Image Frame",
			category: "General",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				inputs: vec![NodeId(0), NodeId(0)],
				outputs: vec![NodeOutput::new(NodeId(1), 0)],
				nodes: vec![
					DocumentNode {
						name: "Image Frame".to_string(),
						inputs: vec![NodeInput::Network(concrete!(graphene_core::raster::Image<Color>)), NodeInput::Network(concrete!(DAffine2))],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::vector::generator_nodes::ImageFrameNode<_, _>")),
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
				inputs: vec![
					NodeId(0),
					NodeId(0),
					NodeId(0),
					NodeId(0),
					NodeId(0),
					NodeId(0),
					NodeId(0),
					NodeId(0),
					NodeId(0),
					NodeId(0),
					NodeId(0),
					NodeId(0),
					NodeId(0),
					NodeId(0),
					NodeId(0),
					NodeId(0),
				],
				outputs: vec![NodeOutput::new(NodeId(1), 0)],
				nodes: vec![
					DocumentNode {
						name: "Noise Pattern".to_string(),
						inputs: vec![
							NodeInput::Network(concrete!(())),
							NodeInput::Network(concrete!(UVec2)),
							NodeInput::Network(concrete!(u32)),
							NodeInput::Network(concrete!(f64)),
							NodeInput::Network(concrete!(graphene_core::raster::NoiseType)),
							NodeInput::Network(concrete!(graphene_core::raster::FractalType)),
							NodeInput::Network(concrete!(f64)),
							NodeInput::Network(concrete!(graphene_core::raster::FractalType)),
							NodeInput::Network(concrete!(u32)),
							NodeInput::Network(concrete!(f64)),
							NodeInput::Network(concrete!(f64)),
							NodeInput::Network(concrete!(f64)),
							NodeInput::Network(concrete!(f64)),
							NodeInput::Network(concrete!(graphene_core::raster::CellularDistanceFunction)),
							NodeInput::Network(concrete!(graphene_core::raster::CellularReturnType)),
							NodeInput::Network(concrete!(f64)),
						],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_std::raster::NoisePatternNode<_, _, _, _, _, _, _, _, _, _, _, _, _, _, _>")),
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
				DocumentInputType::value("None", TaggedValue::None, false),
				// All
				DocumentInputType::value("Dimensions", TaggedValue::UVec2((512, 512).into()), false),
				DocumentInputType::value("Seed", TaggedValue::U32(0), false),
				DocumentInputType::value("Scale", TaggedValue::F64(10.), false),
				DocumentInputType::value("Noise Type", TaggedValue::NoiseType(NoiseType::Perlin), false),
				// Domain Warp
				DocumentInputType::value("Domain Warp Type", TaggedValue::DomainWarpType(DomainWarpType::None), false),
				DocumentInputType::value("Domain Warp Amplitude", TaggedValue::F64(100.), false),
				// Fractal
				DocumentInputType::value("Fractal Type", TaggedValue::FractalType(FractalType::None), false),
				DocumentInputType::value("Fractal Octaves", TaggedValue::U32(3), false),
				DocumentInputType::value("Fractal Lacunarity", TaggedValue::F64(2.), false),
				DocumentInputType::value("Fractal Gain", TaggedValue::F64(0.5), false),
				DocumentInputType::value("Fractal Weighted Strength", TaggedValue::F64(0.), false), // 0-1 range
				DocumentInputType::value("Fractal Ping Pong Strength", TaggedValue::F64(2.), false),
				// Cellular
				DocumentInputType::value("Cellular Distance Function", TaggedValue::CellularDistanceFunction(CellularDistanceFunction::Euclidean), false),
				DocumentInputType::value("Cellular Return Type", TaggedValue::CellularReturnType(CellularReturnType::Nearest), false),
				DocumentInputType::value("Cellular Jitter", TaggedValue::F64(1.), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::noise_pattern_properties,
			..Default::default()
		},
		// TODO: This needs to work with resolution-aware (raster with footprint, post-Cull node) data.
		DocumentNodeDefinition {
			name: "Mask",
			category: "Image Adjustments",
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
			category: "Image Adjustments",
			implementation: DocumentNodeImplementation::proto("graphene_std::raster::InsertChannelNode<_, _, _, _>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Insertion", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Replace", TaggedValue::RedGreenBlue(RedGreenBlue::Red), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::insert_channel_properties,
			..Default::default()
		},
		// TODO: This needs to work with resolution-aware (raster with footprint, post-Cull node) data.
		DocumentNodeDefinition {
			name: "Combine Channels",
			category: "Image Adjustments",
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
			category: "Image Adjustments",
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
			category: "Image Adjustments",
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
			category: "Image Adjustments",
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
			category: "Image Adjustments",
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::IdentityNode"),
			inputs: vec![DocumentInputType::value("Channel", TaggedValue::RedGreenBlue(RedGreenBlue::Red), false)],
			outputs: vec![DocumentOutputType::new("Out", FrontendGraphDataType::General)],
			properties: node_properties::color_channel_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Blend Mode Value",
			category: "Inputs",
			implementation: DocumentNodeImplementation::proto("graphene_core::ops::IdentityNode"),
			inputs: vec![DocumentInputType::value("Blend Mode", TaggedValue::BlendMode(BlendMode::Normal), false)],
			outputs: vec![DocumentOutputType::new("Out", FrontendGraphDataType::General)],
			properties: node_properties::blend_mode_value_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Luminance",
			category: "Image Adjustments",
			implementation: DocumentNodeImplementation::proto("graphene_core::raster::LuminanceNode<_>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Luminance Calc", TaggedValue::LuminanceCalculation(LuminanceCalculation::SRGB), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::luminance_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Extract Channel",
			category: "Image Adjustments",
			implementation: DocumentNodeImplementation::proto("graphene_core::raster::ExtractChannelNode<_>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("From", TaggedValue::RedGreenBlue(RedGreenBlue::Red), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::extract_channel_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Extract Alpha",
			category: "Image Adjustments",
			implementation: DocumentNodeImplementation::proto("graphene_core::raster::ExtractAlphaNode<>"),
			inputs: vec![DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true)],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Extract Opaque",
			category: "Image Adjustments",
			implementation: DocumentNodeImplementation::proto("graphene_core::raster::ExtractOpaqueNode<>"),
			inputs: vec![DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true)],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Split Channels",
			category: "Image Adjustments",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				inputs: vec![NodeId(0)],
				outputs: vec![
					NodeOutput::new(NodeId(1), 0),
					NodeOutput::new(NodeId(2), 0),
					NodeOutput::new(NodeId(3), 0),
					NodeOutput::new(NodeId(4), 0),
				],
				nodes: [
					// The input image feeds into the identity, then we take its passed-through value when the other channels are reading from it instead of the original input.
					// We do this for technical restrictions imposed by Graphene which doesn't allow an input to feed into multiple interior nodes in the subgraph.
					// Diagram: <https://files.keavon.com/-/AchingSecondHypsilophodon/capture.png>
					DocumentNode {
						name: "Identity".to_string(),
						inputs: vec![NodeInput::Network(concrete!(ImageFrame<Color>))],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IdentityNode")),
						..Default::default()
					},
					DocumentNode {
						name: "RedNode".to_string(),
						inputs: vec![NodeInput::node(NodeId(0), 0), NodeInput::value(TaggedValue::RedGreenBlue(RedGreenBlue::Red), false)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::raster::ExtractChannelNode<_>")),
						..Default::default()
					},
					DocumentNode {
						name: "GreenNode".to_string(),
						inputs: vec![NodeInput::node(NodeId(0), 0), NodeInput::value(TaggedValue::RedGreenBlue(RedGreenBlue::Green), false)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::raster::ExtractChannelNode<_>")),
						..Default::default()
					},
					DocumentNode {
						name: "BlueNode".to_string(),
						inputs: vec![NodeInput::node(NodeId(0), 0), NodeInput::value(TaggedValue::RedGreenBlue(RedGreenBlue::Blue), false)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::raster::ExtractChannelNode<_>")),
						..Default::default()
					},
					DocumentNode {
						name: "AlphaNode".to_string(),
						inputs: vec![NodeInput::node(NodeId(0), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::raster::ExtractAlphaNode<>")),
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
				inputs: vec![NodeId(0), NodeId(0), NodeId(0), NodeId(0)],
				outputs: vec![NodeOutput::new(NodeId(1), 0)],
				nodes: vec![
					DocumentNode {
						name: "Brush".to_string(),
						inputs: vec![
							NodeInput::Network(concrete!(graphene_core::raster::ImageFrame<Color>)),
							NodeInput::Network(concrete!(graphene_core::raster::ImageFrame<Color>)),
							NodeInput::Network(concrete!(Vec<graphene_core::vector::brush_stroke::BrushStroke>)),
							NodeInput::Network(concrete!(BrushCache)),
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
				inputs: vec![NodeId(0)],
				outputs: vec![NodeOutput::new(NodeId(1), 0)],
				nodes: vec![
					DocumentNode {
						name: "Identity".to_string(),
						inputs: vec![NodeInput::Network(concrete!(ImageFrame<Color>))],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IdentityNode")),
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
				inputs: vec![NodeId(1), NodeId(0)],
				outputs: vec![NodeOutput::new(NodeId(2), 0)],
				nodes: [
					DocumentNode {
						name: "Extract Executor".to_string(),
						inputs: vec![NodeInput::Network(concrete!(WasmEditorApi))],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<_, &WgpuExecutor>")),
						..Default::default()
					},
					DocumentNode {
						name: "Create Uniform".to_string(),
						inputs: vec![NodeInput::Network(generic!(T)), NodeInput::node(NodeId(0), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("gpu_executor::UniformNode<_>")),
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
					default: NodeInput::value(TaggedValue::F64(0.), true),
				},
				DocumentInputType {
					name: "In",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::Network(concrete!(WasmEditorApi)),
				},
			],
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
				inputs: vec![NodeId(1), NodeId(0)],
				outputs: vec![NodeOutput::new(NodeId(2), 0)],
				nodes: [
					DocumentNode {
						name: "Extract Executor".to_string(),
						inputs: vec![NodeInput::Network(concrete!(WasmEditorApi))],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<_, &WgpuExecutor>")),
						..Default::default()
					},
					DocumentNode {
						name: "Create Storage".to_string(),
						inputs: vec![NodeInput::Network(concrete!(Vec<u8>)), NodeInput::node(NodeId(0), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("gpu_executor::StorageNode<_>")),
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
					default: NodeInput::Network(concrete!(WasmEditorApi)),
				},
			],
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
				inputs: vec![NodeId(1), NodeId(1), NodeId(0)],
				outputs: vec![NodeOutput::new(NodeId(2), 0)],
				nodes: [
					DocumentNode {
						name: "Extract Executor".to_string(),
						inputs: vec![NodeInput::Network(concrete!(WasmEditorApi))],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<_, &WgpuExecutor>")),
						..Default::default()
					},
					DocumentNode {
						name: "Create Output Buffer".to_string(),
						inputs: vec![NodeInput::Network(concrete!(usize)), NodeInput::node(NodeId(0), 0), NodeInput::Network(concrete!(Type))],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("gpu_executor::CreateOutputBufferNode<_, _>")),
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
					default: NodeInput::Network(concrete!(WasmEditorApi)),
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
				inputs: vec![NodeId(1), NodeId(0), NodeId(1), NodeId(1)],
				outputs: vec![NodeOutput::new(NodeId(2), 0)],
				nodes: [
					DocumentNode {
						name: "Extract Executor".to_string(),
						inputs: vec![NodeInput::Network(concrete!(WasmEditorApi))],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<_, &WgpuExecutor>")),
						..Default::default()
					},
					DocumentNode {
						name: "Create Compute Pass".to_string(),
						inputs: vec![
							NodeInput::Network(concrete!(gpu_executor::PipelineLayout<WgpuExecutor>)),
							NodeInput::node(NodeId(0), 0),
							NodeInput::Network(concrete!(ShaderInput<WgpuExecutor>)),
							NodeInput::Network(concrete!(gpu_executor::ComputePassDimensions)),
						],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("gpu_executor::CreateComputePassNode<_, _, _>")),
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
					default: NodeInput::Network(concrete!(gpu_executor::PipelineLayout<WgpuExecutor>)),
				},
				DocumentInputType {
					name: "In",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::Network(concrete!(WasmEditorApi)),
				},
				DocumentInputType {
					name: "In",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::Network(concrete!(ShaderInput<WgpuExecutor>)),
				},
				DocumentInputType {
					name: "In",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::Network(concrete!(gpu_executor::ComputePassDimensions)),
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
			implementation: DocumentNodeImplementation::proto("gpu_executor::CreatePipelineLayoutNode<_, _, _, _>"),
			inputs: vec![
				DocumentInputType {
					name: "ShaderHandle",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::Network(concrete!(<WgpuExecutor as GpuExecutor>::ShaderHandle)),
				},
				DocumentInputType {
					name: "String",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::Network(concrete!(String)),
				},
				DocumentInputType {
					name: "Bindgroup",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::Network(concrete!(gpu_executor::Bindgroup<WgpuExecutor>)),
				},
				DocumentInputType {
					name: "ArcShaderInput",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::Network(concrete!(Arc<ShaderInput<WgpuExecutor>>)),
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
				inputs: vec![NodeId(1), NodeId(0)],
				outputs: vec![NodeOutput::new(NodeId(2), 0)],
				nodes: [
					DocumentNode {
						name: "Extract Executor".to_string(),
						inputs: vec![NodeInput::Network(concrete!(WasmEditorApi))],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<_, &WgpuExecutor>")),
						..Default::default()
					},
					DocumentNode {
						name: "Execute Compute Pipeline".to_string(),
						inputs: vec![NodeInput::Network(concrete!(<WgpuExecutor as GpuExecutor>::CommandBuffer)), NodeInput::node(NodeId(0), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("gpu_executor::ExecuteComputePipelineNode<_>")),
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
					default: NodeInput::Network(concrete!(WasmEditorApi)),
				},
			],
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
				inputs: vec![NodeId(1), NodeId(0)],
				outputs: vec![NodeOutput::new(NodeId(2), 0)],
				nodes: [
					DocumentNode {
						name: "Extract Executor".to_string(),
						inputs: vec![NodeInput::Network(concrete!(WasmEditorApi))],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<_, &WgpuExecutor>")),
						..Default::default()
					},
					DocumentNode {
						name: "Read Output Buffer".to_string(),
						inputs: vec![NodeInput::Network(concrete!(Arc<ShaderInput<WgpuExecutor>>)), NodeInput::node(NodeId(0), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("gpu_executor::ReadOutputBufferNode<_, _>")),
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
					default: NodeInput::Network(concrete!(WasmEditorApi)),
				},
			],
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
				inputs: vec![NodeId(0)],
				outputs: vec![NodeOutput::new(NodeId(1), 0)],
				nodes: [
					DocumentNode {
						name: "Create Gpu Surface".to_string(),
						inputs: vec![NodeInput::Network(concrete!(WasmEditorApi))],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("gpu_executor::CreateGpuSurfaceNode")),
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
			inputs: vec![DocumentInputType {
				name: "In",
				data_type: FrontendGraphDataType::General,
				default: NodeInput::Network(concrete!(WasmEditorApi)),
			}],
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
				inputs: vec![NodeId(1), NodeId(1), NodeId(0)],
				outputs: vec![NodeOutput::new(NodeId(1), 0)],
				nodes: [
					DocumentNode {
						name: "Extract Executor".to_string(),
						inputs: vec![NodeInput::Network(concrete!(WasmEditorApi))],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<_, &WgpuExecutor>")),
						..Default::default()
					},
					DocumentNode {
						name: "Render Texture".to_string(),
						inputs: vec![
							NodeInput::Network(concrete!(ShaderInputFrame<WgpuExecutor>)),
							NodeInput::Network(concrete!(Arc<SurfaceHandle<<WgpuExecutor as GpuExecutor>::Surface<'_>>>)),
							NodeInput::node(NodeId(0), 0),
						],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("gpu_executor::RenderTextureNode<_, _>")),
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
				DocumentInputType {
					name: "EditorApi",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::Network(concrete!(WasmEditorApi)),
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
				inputs: vec![NodeId(1), NodeId(0)],
				outputs: vec![NodeOutput::new(NodeId(2), 0)],
				nodes: [
					DocumentNode {
						name: "Extract Executor".to_string(),
						inputs: vec![NodeInput::Network(concrete!(WasmEditorApi))],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IntoNode<_, &WgpuExecutor>")),
						..Default::default()
					},
					DocumentNode {
						name: "Upload Texture".to_string(),
						inputs: vec![NodeInput::Network(concrete!(ImageFrame<Color>)), NodeInput::node(NodeId(0), 0)],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("gpu_executor::UploadTextureNode<_>")),
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
					default: NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
				},
				DocumentInputType {
					name: "In",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::Network(concrete!(WasmEditorApi)),
				},
			],
			outputs: vec![DocumentOutputType {
				name: "Texture",
				data_type: FrontendGraphDataType::General,
			}],
			..Default::default()
		},
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			name: "GpuImage",
			category: "Image Adjustments",
			implementation: DocumentNodeImplementation::proto("graphene_std::executor::MapGpuSingleImageNode<_>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType {
					name: "Node",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::value(TaggedValue::DocumentNode(DocumentNode::default()), true),
				},
				DocumentInputType {
					name: "In",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::Network(concrete!(WasmEditorApi)),
				},
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			..Default::default()
		},
		#[cfg(feature = "gpu")]
		DocumentNodeDefinition {
			name: "Blend (GPU)",
			category: "Image Adjustments",
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
			category: "Image Adjustments",
			implementation: DocumentNodeImplementation::proto("graphene_core::raster::InvertRGBNode"),
			inputs: vec![DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true)],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Hue/Saturation",
			category: "Image Adjustments",
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
			category: "Image Adjustments",
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
			category: "Image Adjustments",
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
			category: "Image Adjustments",
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
			category: "Image Adjustments",
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
			category: "Image Adjustments",
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
				DocumentInputType::value("Output Channel", TaggedValue::RedGreenBlue(RedGreenBlue::Red), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::adjust_channel_mixer_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Selective Color",
			category: "Image Adjustments",
			implementation: DocumentNodeImplementation::proto(
				"graphene_core::raster::SelectiveColorNode<_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _>",
			),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				// Mode
				DocumentInputType::value("Mode", TaggedValue::RelativeAbsolute(RelativeAbsolute::Relative), false),
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
				DocumentInputType::value("Colors", TaggedValue::SelectiveColorChoice(SelectiveColorChoice::Reds), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::adjust_selective_color_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Opacity",
			category: "Image Adjustments",
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
			category: "Image Adjustments",
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
			category: "Image Adjustments",
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
			category: "Image Adjustments",
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
			outputs: vec![DocumentOutputType::new("Output", FrontendGraphDataType::Boolean)],
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
			outputs: vec![DocumentOutputType::new("Output", FrontendGraphDataType::Boolean)],
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
			outputs: vec![DocumentOutputType::new("Output", FrontendGraphDataType::Boolean)],
			properties: node_properties::logic_operator_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Not",
			category: "Logic",
			implementation: DocumentNodeImplementation::proto("graphene_core::logic::LogicNotNode"),
			inputs: vec![DocumentInputType::value("Input", TaggedValue::Bool(false), true)],
			outputs: vec![DocumentOutputType::new("Output", FrontendGraphDataType::Boolean)],
			properties: node_properties::node_no_properties,
			..Default::default()
		},
		(*IMAGINATE_NODE).clone(),
		DocumentNodeDefinition {
			name: "Circle",
			category: "Vector",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				inputs: vec![NodeId(0), NodeId(0)],
				outputs: vec![NodeOutput::new(NodeId(1), 0)],
				nodes: vec![
					DocumentNode {
						name: "Circle Generator".to_string(),
						inputs: vec![NodeInput::Network(concrete!(())), NodeInput::Network(concrete!(f64))],
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
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Subpath)],
			properties: node_properties::circle_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Ellipse",
			category: "Vector",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				inputs: vec![NodeId(0), NodeId(0), NodeId(0)],
				outputs: vec![NodeOutput::new(NodeId(1), 0)],
				nodes: vec![
					DocumentNode {
						name: "Ellipse Generator".to_string(),
						inputs: vec![NodeInput::Network(concrete!(())), NodeInput::Network(concrete!(f64)), NodeInput::Network(concrete!(f64))],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::vector::generator_nodes::EllipseGenerator<_, _>")),
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
				DocumentInputType::none(),
				DocumentInputType::value("Radius X", TaggedValue::F64(50.), false),
				DocumentInputType::value("Radius Y", TaggedValue::F64(25.), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Subpath)],
			properties: node_properties::ellipse_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Rectangle",
			category: "Vector",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				inputs: vec![NodeId(0), NodeId(0), NodeId(0)],
				outputs: vec![NodeOutput::new(NodeId(1), 0)],
				nodes: vec![
					DocumentNode {
						name: "Rectangle Generator".to_string(),
						inputs: vec![NodeInput::Network(concrete!(())), NodeInput::Network(concrete!(f64)), NodeInput::Network(concrete!(f64))],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::vector::generator_nodes::RectangleGenerator<_, _>")),
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
				DocumentInputType::none(),
				DocumentInputType::value("Size X", TaggedValue::F64(100.), false),
				DocumentInputType::value("Size Y", TaggedValue::F64(100.), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Subpath)],
			properties: node_properties::rectangle_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Regular Polygon",
			category: "Vector",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				inputs: vec![NodeId(0), NodeId(0), NodeId(0)],
				outputs: vec![NodeOutput::new(NodeId(1), 0)],
				nodes: vec![
					DocumentNode {
						name: "Regular Polygon Generator".to_string(),
						inputs: vec![NodeInput::Network(concrete!(())), NodeInput::Network(concrete!(u32)), NodeInput::Network(concrete!(f64))],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::vector::generator_nodes::RegularPolygonGenerator<_, _>")),
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
				DocumentInputType::none(),
				DocumentInputType::value("Sides", TaggedValue::U32(6), false),
				DocumentInputType::value("Radius", TaggedValue::F64(50.), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Subpath)],
			properties: node_properties::regular_polygon_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Star",
			category: "Vector",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				inputs: vec![NodeId(0), NodeId(0), NodeId(0), NodeId(0)],
				outputs: vec![NodeOutput::new(NodeId(1), 0)],
				nodes: vec![
					DocumentNode {
						name: "Star Generator".to_string(),
						inputs: vec![
							NodeInput::Network(concrete!(())),
							NodeInput::Network(concrete!(u32)),
							NodeInput::Network(concrete!(f64)),
							NodeInput::Network(concrete!(f64)),
						],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::vector::generator_nodes::StarGenerator<_, _, _>")),
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
				DocumentInputType::none(),
				DocumentInputType::value("Sides", TaggedValue::U32(5), false),
				DocumentInputType::value("Radius", TaggedValue::F64(50.), false),
				DocumentInputType::value("Inner Radius", TaggedValue::F64(25.), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Subpath)],
			properties: node_properties::star_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Line",
			category: "Vector",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				inputs: vec![NodeId(0), NodeId(0), NodeId(0)],
				outputs: vec![NodeOutput::new(NodeId(1), 0)],
				nodes: vec![
					DocumentNode {
						name: "Line Generator".to_string(),
						inputs: vec![NodeInput::Network(concrete!(())), NodeInput::Network(concrete!(DVec2)), NodeInput::Network(concrete!(DVec2))],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::vector::generator_nodes::LineGenerator<_, _>")),
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
				DocumentInputType::none(),
				DocumentInputType::value("Start", TaggedValue::DVec2(DVec2::new(0., -50.)), false),
				DocumentInputType::value("End", TaggedValue::DVec2(DVec2::new(0., 50.)), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Subpath)],
			properties: node_properties::line_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Spline",
			category: "Vector",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				inputs: vec![NodeId(0), NodeId(0)],
				outputs: vec![NodeOutput::new(NodeId(1), 0)],
				nodes: vec![
					DocumentNode {
						name: "Spline Generator".to_string(),
						inputs: vec![NodeInput::Network(concrete!(())), NodeInput::Network(concrete!(Vec<DVec2>))],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::vector::generator_nodes::SplineGenerator<_>")),
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
				DocumentInputType::none(),
				DocumentInputType::value("Points", TaggedValue::VecDVec2(vec![DVec2::new(0., -50.), DVec2::new(25., 0.), DVec2::new(0., 50.)]), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Subpath)],
			properties: node_properties::spline_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Shape",
			category: "Vector",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				inputs: vec![NodeId(0), NodeId(0)],
				outputs: vec![NodeOutput::new(NodeId(1), 0)],
				nodes: vec![
					DocumentNode {
						name: "Path Generator".to_string(),
						inputs: vec![
							NodeInput::Network(concrete!(Vec<bezier_rs::Subpath<graphene_core::uuid::ManipulatorGroupId>>)),
							NodeInput::Network(concrete!(Vec<graphene_core::uuid::ManipulatorGroupId>)),
						],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::vector::generator_nodes::PathGenerator<_>")),
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
				DocumentInputType::value("Path Data", TaggedValue::Subpaths(vec![]), false),
				DocumentInputType::value("Mirror", TaggedValue::ManipulatorGroupIds(vec![]), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Subpath)],
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Sample",
			category: "Structural",
			implementation: DocumentNodeImplementation::proto("graphene_std::raster::SampleNode<_>"),
			manual_composition: Some(concrete!(Footprint)),
			inputs: vec![DocumentInputType::value("Raseter Data", TaggedValue::ImageFrame(ImageFrame::empty()), true)],
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
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Subpath)],
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Text",
			category: "Vector",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				inputs: vec![NodeId(0), NodeId(0), NodeId(0), NodeId(0)],
				outputs: vec![NodeOutput::new(NodeId(1), 0)],
				nodes: vec![
					DocumentNode {
						name: "Text Generator".to_string(),
						inputs: vec![
							NodeInput::Network(concrete!(application_io::EditorApi<graphene_std::wasm_application_io::WasmApplicationIo>)),
							NodeInput::Network(concrete!(String)),
							NodeInput::Network(concrete!(graphene_core::text::Font)),
							NodeInput::Network(concrete!(f64)),
						],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::text::TextGeneratorNode<_, _, _>")),
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
				DocumentInputType::none(),
				DocumentInputType::value("Text", TaggedValue::String("Lorem ipsum".to_string()), false),
				DocumentInputType::value("Font", TaggedValue::Font(Font::new(DEFAULT_FONT_FAMILY.into(), DEFAULT_FONT_STYLE.into())), false),
				DocumentInputType::value("Size", TaggedValue::F64(24.), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Subpath)],
			properties: node_properties::node_section_font,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Transform",
			category: "Transform",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				inputs: vec![NodeId(0), NodeId(1), NodeId(1), NodeId(1), NodeId(1), NodeId(1)],
				outputs: vec![NodeOutput::new(NodeId(1), 0)],
				nodes: [
					DocumentNode {
						inputs: vec![NodeInput::Network(concrete!(VectorData))],
						..monitor_node()
					},
					DocumentNode {
						name: "Transform".to_string(),
						inputs: vec![
							NodeInput::node(NodeId(0), 0),
							NodeInput::Network(concrete!(DVec2)),
							NodeInput::Network(concrete!(f64)),
							NodeInput::Network(concrete!(DVec2)),
							NodeInput::Network(concrete!(DVec2)),
							NodeInput::Network(concrete!(DVec2)),
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
			manual_composition: Some(concrete!(Footprint)),
			inputs: vec![
				DocumentInputType::value("Vector Data", TaggedValue::VectorData(VectorData::empty()), true),
				DocumentInputType::value("Translation", TaggedValue::DVec2(DVec2::ZERO), false),
				DocumentInputType::value("Rotation", TaggedValue::F64(0.), false),
				DocumentInputType::value("Scale", TaggedValue::DVec2(DVec2::ONE), false),
				DocumentInputType::value("Skew", TaggedValue::DVec2(DVec2::ZERO), false),
				DocumentInputType::value("Pivot", TaggedValue::DVec2(DVec2::splat(0.5)), false),
			],
			outputs: vec![DocumentOutputType::new("Data", FrontendGraphDataType::Subpath)],
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
			outputs: vec![DocumentOutputType::new("Data", FrontendGraphDataType::Subpath)],
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Fill",
			category: "Vector",
			implementation: DocumentNodeImplementation::proto("graphene_core::vector::SetFillNode<_, _, _, _, _, _, _>"),
			inputs: vec![
				DocumentInputType::value("Vector Data", TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
				DocumentInputType::value("Fill Type", TaggedValue::FillType(vector::style::FillType::Solid), false),
				DocumentInputType::value("Solid Color", TaggedValue::OptionalColor(None), false),
				DocumentInputType::value("Gradient Type", TaggedValue::GradientType(vector::style::GradientType::Linear), false),
				DocumentInputType::value("Start", TaggedValue::DVec2(DVec2::new(0., 0.5)), false),
				DocumentInputType::value("End", TaggedValue::DVec2(DVec2::new(1., 0.5)), false),
				DocumentInputType::value("Transform", TaggedValue::DAffine2(DAffine2::IDENTITY), false),
				DocumentInputType::value("Positions", TaggedValue::GradientPositions(vec![(0., Color::BLACK), (1., Color::WHITE)]), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Subpath)],
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
				DocumentInputType::value("Line Cap", TaggedValue::LineCap(graphene_core::vector::style::LineCap::Butt), false),
				DocumentInputType::value("Line Join", TaggedValue::LineJoin(graphene_core::vector::style::LineJoin::Miter), false),
				DocumentInputType::value("Miter Limit", TaggedValue::F64(4.), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Subpath)],
			properties: node_properties::stroke_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Bounding Box",
			category: "Vector",
			implementation: DocumentNodeImplementation::proto("graphene_core::vector::BoundingBoxNode"),
			inputs: vec![DocumentInputType::value("Vector Data", TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true)],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Subpath)],
			properties: node_properties::node_no_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Repeat",
			category: "Vector",
			implementation: DocumentNodeImplementation::proto("graphene_core::vector::RepeatNode<_, _>"),
			inputs: vec![
				DocumentInputType::value("Instance", TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
				DocumentInputType::value("Direction", TaggedValue::DVec2((100., 0.).into()), false),
				DocumentInputType::value("Count", TaggedValue::U32(10), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Subpath)],
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
				DocumentInputType::value("Count", TaggedValue::U32(10), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Subpath)],
			properties: node_properties::circular_repeat_properties,
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
				DocumentInputType::value("Random Scale Bias", TaggedValue::F64(1.), false),
				DocumentInputType::value("Random Rotation", TaggedValue::F64(0.), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Subpath)],
			properties: node_properties::copy_to_points_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Sample Points",
			category: "Vector",
			implementation: DocumentNodeImplementation::Network(NodeNetwork {
				inputs: vec![NodeId(0), NodeId(2), NodeId(2), NodeId(2), NodeId(2)], // First is given to Identity, the rest are given to Sample Points
				outputs: vec![NodeOutput::new(NodeId(2), 0)],                        // Taken from output 0 of Sample Points
				nodes: [
					DocumentNode {
						name: "Identity".to_string(),
						inputs: vec![NodeInput::Network(concrete!(graphene_core::vector::VectorData))], // From the document node's parameters
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::ops::IdentityNode")),
						..Default::default()
					},
					DocumentNode {
						name: "Lengths of Segments of Subpaths".to_string(),
						inputs: vec![NodeInput::node(NodeId(0), 0)], // From output 0 of Identity
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::vector::LengthsOfSegmentsOfSubpaths")),
						..Default::default()
					},
					DocumentNode {
						name: "Sample Points".to_string(),
						inputs: vec![
							NodeInput::node(NodeId(0), 0),       // From output 0 of Identity
							NodeInput::Network(concrete!(f64)),  // From the document node's parameters
							NodeInput::Network(concrete!(f64)),  // From the document node's parameters
							NodeInput::Network(concrete!(f64)),  // From the document node's parameters
							NodeInput::Network(concrete!(bool)), // From the document node's parameters
							NodeInput::node(NodeId(1), 0),       // From output 0 of Lengths of Segments of Subpaths
						],
						implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("graphene_core::vector::SamplePoints<_, _, _, _, _, _>")),
						manual_composition: Some(concrete!(Footprint)),
						..Default::default()
					},
					// TODO: Add a cache node here?
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
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Subpath)],
			properties: node_properties::sample_points_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Poisson-Disk Points",
			category: "Vector",
			implementation: DocumentNodeImplementation::proto("graphene_core::vector::PoissonDiskPoints<_>"),
			inputs: vec![
				DocumentInputType::value("Vector Data", TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
				DocumentInputType::value("Separation Disk Diameter", TaggedValue::F64(10.), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Subpath)],
			properties: node_properties::poisson_disk_points_properties,
			..Default::default()
		},
		DocumentNodeDefinition {
			name: "Splines from Points",
			category: "Vector",
			implementation: DocumentNodeImplementation::proto("graphene_core::vector::SplinesFromPointsNode"),
			inputs: vec![DocumentInputType::value("Vector Data", TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true)],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Subpath)],
			properties: node_properties::node_no_properties,
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
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Subpath)],
			manual_composition: Some(concrete!(Footprint)),
			properties: node_properties::morph_properties,
			..Default::default()
		},
		// TODO: This needs to work with resolution-aware (raster with footprint, post-Cull node) data.
		DocumentNodeDefinition {
			name: "Image Segmentation",
			category: "Image Adjustments",
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
			category: "Image Adjustments",
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
			category: "Image Adjustments",
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
			category: "Image Adjustments",
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
			category: "Image Adjustments",
			implementation: DocumentNodeImplementation::proto("graphene_std::image_color_palette::ImageColorPaletteNode<_>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Max Size", TaggedValue::U32(8), true),
			],
			outputs: vec![DocumentOutputType::new("Colors", FrontendGraphDataType::Color)],
			properties: node_properties::image_color_palette,
			..Default::default()
		},
	]
}

pub static IMAGINATE_NODE: Lazy<DocumentNodeDefinition> = Lazy::new(|| DocumentNodeDefinition {
	name: "Imaginate",
	category: "Image Synthesis",
	implementation: DocumentNodeImplementation::Network(NodeNetwork {
		inputs: vec![
			NodeId(0),
			NodeId(1),
			NodeId(1),
			NodeId(1),
			NodeId(1),
			NodeId(1),
			NodeId(1),
			NodeId(1),
			NodeId(1),
			NodeId(1),
			NodeId(1),
			NodeId(1),
			NodeId(1),
			NodeId(1),
			NodeId(1),
			NodeId(1),
			NodeId(1),
		],
		outputs: vec![NodeOutput::new(NodeId(1), 0)],
		nodes: [
			(
				NodeId(0),
				DocumentNode {
					inputs: vec![NodeInput::Network(concrete!(ImageFrame<Color>))],
					..monitor_node()
				},
			),
			(
				NodeId(1),
				DocumentNode {
					name: "Imaginate".into(),
					inputs: vec![
						NodeInput::node(NodeId(0), 0),
						NodeInput::Network(concrete!(WasmEditorApi)),
						NodeInput::Network(concrete!(ImaginateController)),
						NodeInput::Network(concrete!(f64)),
						NodeInput::Network(concrete!(Option<DVec2>)),
						NodeInput::Network(concrete!(u32)),
						NodeInput::Network(concrete!(ImaginateSamplingMethod)),
						NodeInput::Network(concrete!(f64)),
						NodeInput::Network(concrete!(String)),
						NodeInput::Network(concrete!(String)),
						NodeInput::Network(concrete!(bool)),
						NodeInput::Network(concrete!(f64)),
						NodeInput::Network(concrete!(bool)),
						NodeInput::Network(concrete!(f64)),
						NodeInput::Network(concrete!(ImaginateMaskStartingFill)),
						NodeInput::Network(concrete!(bool)),
						NodeInput::Network(concrete!(bool)),
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
			default: NodeInput::Network(concrete!(WasmEditorApi)),
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

	/// Converts the [DocumentNodeDefinition] type to a [DocumentNode], completely default
	pub fn default_document_node(&self) -> DocumentNode {
		self.to_document_node(self.inputs.iter().map(|input| input.default.clone()), DocumentNodeMetadata::default())
	}
}

pub fn wrap_network_in_scope(mut network: NodeNetwork, hash: u64) -> NodeNetwork {
	network.generate_node_paths(&[]);

	network.resolve_empty_stacks();
	let node_ids = network.nodes.keys().copied().collect::<Vec<_>>();
	for id in node_ids {
		network.flatten(id);
	}

	let mut network_inputs = Vec::new();
	let mut input_type = None;
	for (id, node) in network.nodes.iter() {
		for input in node.inputs.iter() {
			if let NodeInput::Network(_) = input {
				if input_type.is_none() {
					input_type = Some(input.clone());
				}
				assert_eq!(input, input_type.as_ref().unwrap(), "Networks wrapped in scope must have the same input type {network:#?}");
				network_inputs.push(*id);
			}
		}
	}
	let len = network_inputs.len();
	network.inputs = network_inputs;

	// if the network has no inputs, it doesn't need to be wrapped in a scope
	if len == 0 {
		log::warn!("Network has no inputs, not wrapping in scope");
		return network;
	}

	let inner_network = DocumentNode {
		name: "Scope".to_string(),
		implementation: DocumentNodeImplementation::Network(network),
		inputs: core::iter::repeat(NodeInput::node(NodeId(0), 1)).take(len).collect(),
		..Default::default()
	};

	let mut begin_scope = resolve_document_node_type("Begin Scope")
		.expect("Begin Scope node type not found")
		.to_document_node(vec![input_type.unwrap()], DocumentNodeMetadata::default());
	if let DocumentNodeImplementation::Network(g) = &mut begin_scope.implementation {
		if let Some(node) = g.nodes.get_mut(&NodeId(0)) {
			node.world_state_hash = hash;
		}
	}

	// wrap the inner network in a scope
	let nodes = vec![
		begin_scope,
		inner_network,
		resolve_document_node_type("End Scope")
			.expect("End Scope node type not found")
			.to_document_node(vec![NodeInput::node(NodeId(0), 0), NodeInput::node(NodeId(1), 0)], DocumentNodeMetadata::default()),
	];

	NodeNetwork {
		inputs: vec![NodeId(0)],
		outputs: vec![NodeOutput::new(NodeId(2), 0)],
		nodes: nodes.into_iter().enumerate().map(|(id, node)| (NodeId(id as u64), node)).collect(),
		..Default::default()
	}
}

pub fn new_image_network(output_offset: i32, output_node_id: NodeId) -> NodeNetwork {
	let mut network = NodeNetwork {
		inputs: vec![NodeId(0)],
		..Default::default()
	};
	network.push_node(
		resolve_document_node_type("Input Frame")
			.expect("Input Frame node does not exist")
			.to_document_node_default_inputs([], DocumentNodeMetadata::position((8, 4))),
	);
	network.push_node(
		resolve_document_node_type("Output")
			.expect("Output node does not exist")
			.to_document_node([NodeInput::node(output_node_id, 0)], DocumentNodeMetadata::position((output_offset + 8, 4))),
	);
	network
}

pub fn new_text_network(text: String, font: Font, size: f64) -> NodeNetwork {
	let text_generator = resolve_document_node_type("Text").expect("Text node does not exist");
	let transform = resolve_document_node_type("Transform").expect("Transform node does not exist");
	let fill = resolve_document_node_type("Fill").expect("Fill node does not exist");
	let stroke = resolve_document_node_type("Stroke").expect("Stroke node does not exist");
	let output = resolve_document_node_type("Output").expect("Output node does not exist");

	let mut network = NodeNetwork {
		inputs: vec![NodeId(0)],
		..Default::default()
	};
	network.push_node(text_generator.to_document_node(
		[
			NodeInput::Network(concrete!(WasmEditorApi)),
			NodeInput::value(TaggedValue::String(text), false),
			NodeInput::value(TaggedValue::Font(font), false),
			NodeInput::value(TaggedValue::F64(size), false),
		],
		DocumentNodeMetadata::position((0, 4)),
	));
	network.push_node(transform.to_document_node_default_inputs([None], Default::default()));
	network.push_node(fill.to_document_node_default_inputs([None], Default::default()));
	network.push_node(stroke.to_document_node_default_inputs([None], Default::default()));
	network.push_node(output.to_document_node_default_inputs([None], Default::default()));
	network
}
