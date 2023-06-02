use super::{node_properties, FrontendGraphDataType, FrontendNodeType};
use crate::consts::{DEFAULT_FONT_FAMILY, DEFAULT_FONT_STYLE};
use crate::messages::layout::utility_types::layout_widget::LayoutGroup;
use crate::node_graph_executor::NodeGraphExecutor;

use graph_craft::concrete;
use graph_craft::document::value::*;
use graph_craft::document::*;
use graph_craft::imaginate_input::ImaginateSamplingMethod;
use graph_craft::NodeIdentifier;
use graphene_core::raster::{BlendMode, Color, Image, ImageFrame, LuminanceCalculation, RedGreenBlue, RelativeAbsolute, SelectiveColorChoice};
use graphene_core::text::Font;
use graphene_core::vector::VectorData;
use graphene_core::*;

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
	pub persistent_data: &'a crate::messages::portfolio::utility_types::PersistentData,
	pub document: &'a document_legacy::document::Document,
	pub responses: &'a mut VecDeque<crate::messages::prelude::Message>,
	pub layer_path: &'a [document_legacy::LayerId],
	pub nested_path: &'a [NodeId],
	pub executor: &'a mut NodeGraphExecutor,
	pub network: &'a NodeNetwork,
}

#[derive(Clone)]
pub enum NodeImplementation {
	ProtoNode(NodeIdentifier),
	DocumentNode(NodeNetwork),
	Extract,
}

impl NodeImplementation {
	pub fn proto(name: &'static str) -> Self {
		Self::ProtoNode(NodeIdentifier::new(name))
	}
}

#[derive(Clone)]
pub struct DocumentNodeType {
	pub name: &'static str,
	pub category: &'static str,
	pub identifier: NodeImplementation,
	pub inputs: Vec<DocumentInputType>,
	pub outputs: Vec<DocumentOutputType>,
	pub properties: fn(&DocumentNode, NodeId, &mut NodePropertiesContext) -> Vec<LayoutGroup>,
}

// We use the once cell for lazy initialization to avoid the overhead of reconstructing the node list every time.
// TODO: make document nodes not require a `'static` lifetime to avoid having to split the construction into const and non-const parts.
static DOCUMENT_NODE_TYPES: once_cell::sync::Lazy<Vec<DocumentNodeType>> = once_cell::sync::Lazy::new(static_nodes);

// TODO: Dynamic node library
fn static_nodes() -> Vec<DocumentNodeType> {
	vec![
		DocumentNodeType {
			name: "Identity",
			category: "Structural",
			identifier: NodeImplementation::proto("graphene_core::ops::IdNode"),
			inputs: vec![DocumentInputType {
				name: "In",
				data_type: FrontendGraphDataType::General,
				default: NodeInput::value(TaggedValue::None, true),
			}],
			outputs: vec![DocumentOutputType::new("Out", FrontendGraphDataType::General)],
			properties: |_document_node, _node_id, _context| node_properties::string_properties("The identity node simply returns the input"),
		},
		DocumentNodeType {
			name: "Monitor",
			category: "Structural",
			identifier: NodeImplementation::proto("graphene_core::ops::IdNode"),
			inputs: vec![DocumentInputType {
				name: "In",
				data_type: FrontendGraphDataType::General,
				default: NodeInput::value(TaggedValue::None, true),
			}],
			outputs: vec![DocumentOutputType::new("Out", FrontendGraphDataType::General)],
			properties: |_document_node, _node_id, _context| node_properties::string_properties("The Monitor node stores the value of its last evaluation"),
		},
		DocumentNodeType {
			name: "Layer",
			category: "General",
			identifier: NodeImplementation::DocumentNode(NodeNetwork {
				inputs: vec![0; 8],
				outputs: vec![NodeOutput::new(1, 0)],
				nodes: [
					(
						0,
						DocumentNode {
							inputs: vec![
								NodeInput::Network(concrete!(graphene_core::vector::VectorData)),
								NodeInput::Network(concrete!(String)),
								NodeInput::Network(concrete!(BlendMode)),
								NodeInput::Network(concrete!(f32)),
								NodeInput::Network(concrete!(bool)),
								NodeInput::Network(concrete!(bool)),
								NodeInput::Network(concrete!(bool)),
								NodeInput::Network(concrete!(graphene_core::GraphicGroup)),
							],
							implementation: DocumentNodeImplementation::proto("graphene_core::ConstructLayerNode<_, _, _, _, _, _, _>"),
							..Default::default()
						},
					),
					// The monitor node is used to display a thumbnail in the UI.
					(
						1,
						DocumentNode {
							inputs: vec![NodeInput::node(0, 0)],
							implementation: DocumentNodeImplementation::proto("graphene_core::memo::MonitorNode<_>"),
							..Default::default()
						},
					),
				]
				.into(),
				..Default::default()
			}),
			inputs: vec![
				DocumentInputType::value("Vector Data", TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
				DocumentInputType::value("Name", TaggedValue::String(String::new()), false),
				DocumentInputType::value("Blend Mode", TaggedValue::BlendMode(BlendMode::Normal), false),
				DocumentInputType::value("Opacity", TaggedValue::F32(100.), false),
				DocumentInputType::value("Visible", TaggedValue::Bool(true), false),
				DocumentInputType::value("Locked", TaggedValue::Bool(false), false),
				DocumentInputType::value("Collapsed", TaggedValue::Bool(false), false),
				DocumentInputType::value("Stack", TaggedValue::GraphicGroup(GraphicGroup::EMPTY), true),
			],
			outputs: vec![DocumentOutputType::new("Out", FrontendGraphDataType::GraphicGroup)],
			properties: node_properties::layer_properties,
		},
		DocumentNodeType {
			name: "Artboard",
			category: "General",
			identifier: NodeImplementation::proto("graphene_core::ConstructArtboardNode<_, _, _>"),
			inputs: vec![
				DocumentInputType::value("Graphic Group", TaggedValue::GraphicGroup(GraphicGroup::EMPTY), true),
				DocumentInputType::value("Location", TaggedValue::IVec2(glam::IVec2::ZERO), false),
				DocumentInputType::value("Dimensions", TaggedValue::IVec2(glam::IVec2::new(1920, 1080)), false),
				DocumentInputType::value("Background", TaggedValue::Color(Color::WHITE), false),
			],
			outputs: vec![DocumentOutputType::new("Out", FrontendGraphDataType::Artboard)],
			properties: node_properties::artboard_properties,
		},
		DocumentNodeType {
			name: "Downres",
			category: "Raster",
			identifier: NodeImplementation::DocumentNode(NodeNetwork {
				inputs: vec![0],
				outputs: vec![NodeOutput::new(1, 0)],
				nodes: [
					DocumentNode {
						name: "Downres".to_string(),
						inputs: vec![NodeInput::Network(concrete!(ImageFrame<Color>))],
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_std::raster::DownresNode<_>")),
						..Default::default()
					},
					DocumentNode {
						name: "Cache".to_string(),
						inputs: vec![NodeInput::ShortCircut(concrete!(())), NodeInput::node(0, 0)],
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::memo::MemoNode<_, _>")),
						..Default::default()
					},
					// We currently just clone by default
					/*DocumentNode {
						name: "Clone".to_string(),
						inputs: vec![NodeInput::node(1, 0)],
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::ops::CloneNode<_>")),
						..Default::default()
					},*/
				]
				.into_iter()
				.enumerate()
				.map(|(id, node)| (id as NodeId, node))
				.collect(),
				..Default::default()
			}),
			inputs: vec![DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), false)],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: |_document_node, _node_id, _context| node_properties::string_properties("Downres the image to a lower resolution"),
		},
		// DocumentNodeType {
		// 	name: "Input Frame",
		// 	category: "Ignore",
		// 	identifier: NodeImplementation::proto("graphene_core::ops::IdNode"),
		// 	inputs: vec![DocumentInputType {
		// 		name: "In",
		// 		data_type: FrontendGraphDataType::Raster,
		// 		default: NodeInput::Network,
		// 	}],
		// 	outputs: vec![DocumentOutputType::new("Out", FrontendGraphDataType::Raster)],
		// 	properties: node_properties::input_properties,
		// },
		DocumentNodeType {
			name: "Input Frame",
			category: "Ignore",
			identifier: NodeImplementation::proto("graphene_core::ExtractImageFrame"),
			inputs: vec![DocumentInputType {
				name: "In",
				data_type: FrontendGraphDataType::General,
				default: NodeInput::Network(concrete!(EditorApi)),
			}],
			outputs: vec![DocumentOutputType {
				name: "Image Frame",
				data_type: FrontendGraphDataType::Raster,
			}],
			properties: node_properties::input_properties,
		},
		DocumentNodeType {
			name: "Create Canvas",
			category: "Structural",
			identifier: NodeImplementation::DocumentNode(NodeNetwork {
				inputs: vec![0],
				outputs: vec![NodeOutput::new(1, 0)],
				nodes: [
					DocumentNode {
						name: "Create Canvas".to_string(),
						inputs: vec![NodeInput::Network(concrete!(EditorApi))],
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::wasm_application_io::CreateSurfaceNode")),
						..Default::default()
					},
					DocumentNode {
						name: "Cache".to_string(),
						inputs: vec![NodeInput::ShortCircut(concrete!(())), NodeInput::node(0, 0)],
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::memo::MemoNode<_, _>")),
						..Default::default()
					},
				]
				.into_iter()
				.enumerate()
				.map(|(id, node)| (id as NodeId, node))
				.collect(),
				..Default::default()
			}),
			inputs: vec![DocumentInputType {
				name: "In",
				data_type: FrontendGraphDataType::General,
				default: NodeInput::Network(concrete!(EditorApi)),
			}],
			outputs: vec![DocumentOutputType {
				name: "Canvas",
				data_type: FrontendGraphDataType::General,
			}],
			properties: node_properties::input_properties,
		},
		DocumentNodeType {
			name: "Draw Canvas",
			category: "Structural",
			identifier: NodeImplementation::DocumentNode(NodeNetwork {
				inputs: vec![0, 2],
				outputs: vec![NodeOutput::new(3, 0)],
				nodes: [
					DocumentNode {
						name: "Convert Image Frame".to_string(),
						inputs: vec![NodeInput::Network(concrete!(ImageFrame<Color>))],
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::ops::IntoNode<_, ImageFrame<SRGBA8>>")),
						..Default::default()
					},
					DocumentNode {
						name: "Create Canvas".to_string(),
						inputs: vec![NodeInput::Network(concrete!(EditorApi))],
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::wasm_application_io::CreateSurfaceNode")),
						..Default::default()
					},
					DocumentNode {
						name: "Cache".to_string(),
						inputs: vec![NodeInput::ShortCircut(concrete!(())), NodeInput::node(1, 0)],
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::memo::MemoNode<_, _>")),
						..Default::default()
					},
					DocumentNode {
						name: "Draw Canvas".to_string(),
						inputs: vec![NodeInput::node(0, 0), NodeInput::node(2, 0)],
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::wasm_application_io::DrawImageFrameNode<_>")),
						..Default::default()
					},
				]
				.into_iter()
				.enumerate()
				.map(|(id, node)| (id as NodeId, node))
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
					default: NodeInput::Network(concrete!(EditorApi)),
				},
			],
			outputs: vec![DocumentOutputType {
				name: "Canvas",
				data_type: FrontendGraphDataType::General,
			}],
			properties: node_properties::input_properties,
		},
		DocumentNodeType {
			name: "Begin Scope",
			category: "Ignore",
			identifier: NodeImplementation::DocumentNode(NodeNetwork {
				inputs: vec![0],
				outputs: vec![NodeOutput::new(1, 0), NodeOutput::new(2, 0)],
				nodes: [
					DocumentNode {
						name: "SetNode".to_string(),
						inputs: vec![NodeInput::ShortCircut(concrete!(EditorApi))],
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::ops::SomeNode")),
						..Default::default()
					},
					DocumentNode {
						name: "LetNode".to_string(),
						inputs: vec![NodeInput::node(0, 0)],
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::memo::LetNode<_>")),
						..Default::default()
					},
					DocumentNode {
						name: "RefNode".to_string(),
						inputs: vec![NodeInput::ShortCircut(concrete!(())), NodeInput::lambda(1, 0)],
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::memo::RefNode<_, _>")),
						..Default::default()
					},
				]
				.into_iter()
				.enumerate()
				.map(|(id, node)| (id as NodeId, node))
				.collect(),

				..Default::default()
			}),
			inputs: vec![DocumentInputType {
				name: "In",
				data_type: FrontendGraphDataType::Raster,
				default: NodeInput::Network(concrete!(EditorApi)),
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
		},
		DocumentNodeType {
			name: "End Scope",
			category: "Ignore",
			identifier: NodeImplementation::proto("graphene_core::memo::EndLetNode<_>"),
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
			properties: |_document_node, _node_id, _context| node_properties::string_properties("The graph's output is drawn in the layer"),
		},
		DocumentNodeType {
			name: "Output",
			category: "Ignore",
			identifier: NodeImplementation::proto("graphene_core::ops::IdNode"),
			inputs: vec![DocumentInputType {
				name: "Output",
				data_type: FrontendGraphDataType::Raster,
				default: NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
			}],
			outputs: vec![],
			properties: node_properties::output_properties,
		},
		DocumentNodeType {
			name: "Image Frame",
			category: "General",
			identifier: NodeImplementation::proto("graphene_std::raster::ImageFrameNode<_, _>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::Image(Image::empty()), true),
				DocumentInputType::value("Transform", TaggedValue::DAffine2(DAffine2::IDENTITY), true),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: |_document_node, _node_id, _context| node_properties::string_properties("Creates an embedded image with the given transform"),
		},
		DocumentNodeType {
			name: "Mask",
			category: "Image Adjustments",
			identifier: NodeImplementation::proto("graphene_std::raster::MaskImageNode<_, _, _>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Stencil", TaggedValue::ImageFrame(ImageFrame::empty()), true),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::mask_properties,
		},
		DocumentNodeType {
			name: "Insert Channel",
			category: "Image Adjustments",
			identifier: NodeImplementation::proto("graphene_std::raster::InsertChannelNode<_, _, _, _>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Insertion", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Replace", TaggedValue::RedGreenBlue(RedGreenBlue::Red), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::insert_channel_properties,
		},
		DocumentNodeType {
			name: "Combine Channels",
			category: "Image Adjustments",
			identifier: NodeImplementation::proto("graphene_std::raster::CombineChannelsNode"),
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
			properties: node_properties::no_properties,
		},
		DocumentNodeType {
			name: "Blend",
			category: "Image Adjustments",
			identifier: NodeImplementation::proto("graphene_core::raster::BlendNode<_, _, _, _>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Second", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("BlendMode", TaggedValue::BlendMode(BlendMode::Normal), false),
				DocumentInputType::value("Opacity", TaggedValue::F64(100.), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::blend_properties,
		},
		DocumentNodeType {
			name: "Levels",
			category: "Image Adjustments",
			identifier: NodeImplementation::proto("graphene_core::raster::LevelsNode<_, _, _, _, _>"),
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
		},
		DocumentNodeType {
			name: "Grayscale",
			category: "Image Adjustments",
			identifier: NodeImplementation::proto("graphene_core::raster::GrayscaleNode<_, _, _, _, _, _, _>"),
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
			properties: node_properties::grayscale_properties,
		},
		DocumentNodeType {
			name: "Luminance",
			category: "Image Adjustments",
			identifier: NodeImplementation::proto("graphene_core::raster::LuminanceNode<_>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Luminance Calc", TaggedValue::LuminanceCalculation(LuminanceCalculation::SRGB), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::luminance_properties,
		},
		DocumentNodeType {
			name: "Extract Channel",
			category: "Image Adjustments",
			identifier: NodeImplementation::proto("graphene_core::raster::ExtractChannelNode<_>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("From", TaggedValue::RedGreenBlue(RedGreenBlue::Red), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::extract_channel_properties,
		},
		DocumentNodeType {
			name: "Extract Alpha",
			category: "Image Adjustments",
			identifier: NodeImplementation::proto("graphene_core::raster::ExtractAlphaNode<>"),
			inputs: vec![DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true)],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::no_properties,
		},
		DocumentNodeType {
			name: "Split Channels",
			category: "Image Adjustments",
			identifier: NodeImplementation::DocumentNode(NodeNetwork {
				inputs: vec![0],
				outputs: vec![NodeOutput::new(4, 0), NodeOutput::new(1, 0), NodeOutput::new(2, 0), NodeOutput::new(3, 0), NodeOutput::new(4, 0)],
				nodes: [
					DocumentNode {
						name: "Identity".to_string(),
						inputs: vec![NodeInput::Network(concrete!(ImageFrame<Color>))],
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::ops::IdNode")),
						..Default::default()
					},
					DocumentNode {
						name: "RedNode".to_string(),
						inputs: vec![NodeInput::node(0, 0), NodeInput::value(TaggedValue::RedGreenBlue(RedGreenBlue::Red), false)],
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::raster::ExtractChannelNode<_>")),
						..Default::default()
					},
					DocumentNode {
						name: "GreenNode".to_string(),
						inputs: vec![NodeInput::node(0, 0), NodeInput::value(TaggedValue::RedGreenBlue(RedGreenBlue::Green), false)],
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::raster::ExtractChannelNode<_>")),
						..Default::default()
					},
					DocumentNode {
						name: "BlueNode".to_string(),
						inputs: vec![NodeInput::node(0, 0), NodeInput::value(TaggedValue::RedGreenBlue(RedGreenBlue::Blue), false)],
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::raster::ExtractChannelNode<_>")),
						..Default::default()
					},
					DocumentNode {
						name: "AlphaNode".to_string(),
						inputs: vec![NodeInput::node(0, 0)],
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::raster::ExtractAlphaNode<>")),
						..Default::default()
					},
					DocumentNode {
						name: "EmptyOutput".to_string(),
						inputs: vec![NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), false)],
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::ops::IdNode")),
						..Default::default()
					},
				]
				.into_iter()
				.enumerate()
				.map(|(id, node)| (id as NodeId, node))
				.collect(),

				..Default::default()
			}),
			inputs: vec![DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true)],
			outputs: vec![
				DocumentOutputType::new("Empty", FrontendGraphDataType::Raster),
				DocumentOutputType::new("Red", FrontendGraphDataType::Raster),
				DocumentOutputType::new("Green", FrontendGraphDataType::Raster),
				DocumentOutputType::new("Blue", FrontendGraphDataType::Raster),
				DocumentOutputType::new("Alpha", FrontendGraphDataType::Raster),
			],
			properties: node_properties::no_properties,
		},
		DocumentNodeType {
			name: "Brush",
			category: "Brush",
			identifier: NodeImplementation::proto("graphene_std::brush::BrushNode<_, _>"),
			inputs: vec![
				DocumentInputType::value("Background", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Bounds", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Trace", TaggedValue::BrushStrokes(Vec::new()), false),
			],
			outputs: vec![DocumentOutputType {
				name: "Image",
				data_type: FrontendGraphDataType::Raster,
			}],
			properties: node_properties::no_properties,
		},
		DocumentNodeType {
			name: "Extract Vector Points",
			category: "Brush",
			identifier: NodeImplementation::proto("graphene_std::brush::VectorPointsNode"),
			inputs: vec![DocumentInputType::value("VectorData", TaggedValue::VectorData(VectorData::empty()), true)],
			outputs: vec![DocumentOutputType {
				name: "Vector Points",
				data_type: FrontendGraphDataType::General,
			}],
			properties: node_properties::no_properties,
		},
		DocumentNodeType {
			name: "Memoize",
			category: "Structural",
			identifier: NodeImplementation::proto("graphene_core::memo::MemoNode<_, _>"),
			inputs: vec![DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true)],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::no_properties,
		},
		DocumentNodeType {
			name: "Image",
			category: "Ignore",
			identifier: NodeImplementation::proto("graphene_core::ops::IdNode"),
			inputs: vec![DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), false)],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: |_document_node, _node_id, _context| node_properties::string_properties("A bitmap image embedded in this node"),
		},
		DocumentNodeType {
			name: "Ref",
			category: "Structural",
			identifier: NodeImplementation::proto("graphene_core::memo::MemoNode<_, _>"),
			inputs: vec![DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true)],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::no_properties,
		},
		#[cfg(feature = "gpu")]
		DocumentNodeType {
			name: "GpuImage",
			category: "Image Adjustments",
			identifier: NodeImplementation::proto("graphene_std::executor::MapGpuSingleImageNode<_>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType {
					name: "Node",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::value(TaggedValue::DocumentNode(DocumentNode::default()), true),
				},
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::no_properties,
		},
		#[cfg(feature = "gpu")]
		DocumentNodeType {
			name: "Blend (GPU)",
			category: "Image Adjustments",
			identifier: NodeImplementation::proto("graphene_std::executor::BlendGpuImageNode<_, _, _>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Second", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Blend Mode", TaggedValue::BlendMode(BlendMode::Normal), false),
				DocumentInputType::value("Opacity", TaggedValue::F32(100.0), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::blend_properties,
		},
		DocumentNodeType {
			name: "Extract",
			category: "Macros",
			identifier: NodeImplementation::Extract,
			inputs: vec![DocumentInputType {
				name: "Node",
				data_type: FrontendGraphDataType::General,
				default: NodeInput::value(TaggedValue::DocumentNode(DocumentNode::default()), true),
			}],
			outputs: vec![DocumentOutputType::new("DocumentNode", FrontendGraphDataType::General)],
			properties: node_properties::no_properties,
		},
		#[cfg(feature = "quantization")]
		#[cfg(feature = "quantization")]
		DocumentNodeType {
			name: "Generate Quantization",
			category: "Quantization",
			identifier: NodeImplementation::proto("graphene_std::quantization::GenerateQuantizationNode<_, _>"),
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
		},
		#[cfg(feature = "quantization")]
		DocumentNodeType {
			name: "Quantize Image",
			category: "Quantization",
			identifier: NodeImplementation::proto("graphene_core::quantization::QuantizeNode<_>"),
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
		},
		#[cfg(feature = "quantization")]
		DocumentNodeType {
			name: "DeQuantize Image",
			category: "Quantization",
			identifier: NodeImplementation::proto("graphene_core::quantization::DeQuantizeNode<_>"),
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
		},
		DocumentNodeType {
			name: "Invert RGB",
			category: "Image Adjustments",
			identifier: NodeImplementation::proto("graphene_core::raster::InvertRGBNode"),
			inputs: vec![DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true)],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::no_properties,
		},
		DocumentNodeType {
			name: "Hue/Saturation",
			category: "Image Adjustments",
			identifier: NodeImplementation::proto("graphene_core::raster::HueSaturationNode<_, _, _>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Hue Shift", TaggedValue::F64(0.), false),
				DocumentInputType::value("Saturation Shift", TaggedValue::F64(0.), false),
				DocumentInputType::value("Lightness Shift", TaggedValue::F64(0.), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::adjust_hsl_properties,
		},
		DocumentNodeType {
			name: "Brightness/Contrast",
			category: "Image Adjustments",
			identifier: NodeImplementation::proto("graphene_core::raster::BrightnessContrastNode<_, _, _>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Brightness", TaggedValue::F64(0.), false),
				DocumentInputType::value("Contrast", TaggedValue::F64(0.), false),
				DocumentInputType::value("Use Legacy", TaggedValue::Bool(false), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::brightness_contrast_properties,
		},
		DocumentNodeType {
			name: "Threshold",
			category: "Image Adjustments",
			identifier: NodeImplementation::proto("graphene_core::raster::ThresholdNode<_, _, _>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Min Luminance", TaggedValue::F64(50.), false),
				DocumentInputType::value("Max Luminance", TaggedValue::F64(100.), false),
				DocumentInputType::value("Luminance Calc", TaggedValue::LuminanceCalculation(LuminanceCalculation::SRGB), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::adjust_threshold_properties,
		},
		DocumentNodeType {
			name: "Vibrance",
			category: "Image Adjustments",
			identifier: NodeImplementation::proto("graphene_core::raster::VibranceNode<_>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Vibrance", TaggedValue::F64(0.), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::adjust_vibrance_properties,
		},
		DocumentNodeType {
			name: "Channel Mixer",
			category: "Image Adjustments",
			identifier: NodeImplementation::proto("graphene_core::raster::ChannelMixerNode<_, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _, _>"),
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
		},
		DocumentNodeType {
			name: "Selective Color",
			category: "Image Adjustments",
			identifier: NodeImplementation::proto(
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
		},
		DocumentNodeType {
			name: "Opacity",
			category: "Image Adjustments",
			identifier: NodeImplementation::proto("graphene_core::raster::OpacityNode<_>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Factor", TaggedValue::F64(100.), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::multiply_opacity,
		},
		DocumentNodeType {
			name: "Posterize",
			category: "Image Adjustments",
			identifier: NodeImplementation::proto("graphene_core::raster::PosterizeNode<_>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Value", TaggedValue::F64(4.), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::posterize_properties,
		},
		DocumentNodeType {
			name: "Exposure",
			category: "Image Adjustments",
			identifier: NodeImplementation::proto("graphene_core::raster::ExposureNode<_, _, _>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Exposure", TaggedValue::F64(0.), false),
				DocumentInputType::value("Offset", TaggedValue::F64(0.), false),
				DocumentInputType::value("Gamma Correction", TaggedValue::F64(1.), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::exposure_properties,
		},
		DocumentNodeType {
			name: "Add",
			category: "Math",
			identifier: NodeImplementation::proto("graphene_core::ops::AddParameterNode<_>"),
			inputs: vec![
				DocumentInputType::value("Input", TaggedValue::F64(0.), true),
				DocumentInputType::value("Addend", TaggedValue::F64(0.), true),
			],
			outputs: vec![DocumentOutputType::new("Output", FrontendGraphDataType::Number)],
			properties: node_properties::add_properties,
		},
		(*IMAGINATE_NODE).clone(),
		DocumentNodeType {
			name: "Unit Circle Generator",
			category: "Vector",
			identifier: NodeImplementation::proto("graphene_core::vector::generator_nodes::UnitCircleGenerator"),
			inputs: vec![DocumentInputType::none()],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Subpath)],
			properties: node_properties::no_properties,
		},
		DocumentNodeType {
			name: "Path Generator",
			category: "Vector",
			identifier: NodeImplementation::proto("graphene_core::vector::generator_nodes::PathGenerator<_>"),
			inputs: vec![
				DocumentInputType::value("Path Data", TaggedValue::Subpaths(vec![]), false),
				DocumentInputType::value("Mirror", TaggedValue::ManipulatorGroupIds(vec![]), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Subpath)],
			properties: node_properties::no_properties,
		},
		DocumentNodeType {
			name: "Text",
			category: "Vector",
			identifier: NodeImplementation::proto("graphene_core::text::TextGenerator<_, _, _>"),
			inputs: vec![
				DocumentInputType::none(),
				DocumentInputType::value("Text", TaggedValue::String("hello world".to_string()), false),
				DocumentInputType::value("Font", TaggedValue::Font(Font::new(DEFAULT_FONT_FAMILY.into(), DEFAULT_FONT_STYLE.into())), false),
				DocumentInputType::value("Size", TaggedValue::F64(24.), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Subpath)],
			properties: node_properties::node_section_font,
		},
		DocumentNodeType {
			name: "Transform",
			category: "Transform",
			identifier: NodeImplementation::proto("graphene_core::transform::TransformNode<_, _, _, _, _>"),
			inputs: vec![
				DocumentInputType::value("Data", TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
				DocumentInputType::value("Translation", TaggedValue::DVec2(DVec2::ZERO), false),
				DocumentInputType::value("Rotation", TaggedValue::F64(0.), false),
				DocumentInputType::value("Scale", TaggedValue::DVec2(DVec2::ONE), false),
				DocumentInputType::value("Skew", TaggedValue::DVec2(DVec2::ZERO), false),
				DocumentInputType::value("Pivot", TaggedValue::DVec2(DVec2::splat(0.5)), false),
			],
			outputs: vec![DocumentOutputType::new("Data", FrontendGraphDataType::Subpath)],
			properties: node_properties::transform_properties,
		},
		DocumentNodeType {
			name: "SetTransform",
			category: "Transform",
			identifier: NodeImplementation::proto("graphene_core::transform::SetTransformNode<_>"),
			inputs: vec![
				DocumentInputType::value("Data", TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
				DocumentInputType::value("Transform", TaggedValue::DAffine2(DAffine2::IDENTITY), true),
			],
			outputs: vec![DocumentOutputType::new("Data", FrontendGraphDataType::Subpath)],
			properties: node_properties::no_properties,
		},
		DocumentNodeType {
			name: "Fill",
			category: "Vector",
			identifier: NodeImplementation::proto("graphene_core::vector::SetFillNode<_, _, _, _, _, _, _>"),
			inputs: vec![
				DocumentInputType::value("Vector Data", TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
				DocumentInputType::value("Fill Type", TaggedValue::FillType(vector::style::FillType::None), false),
				DocumentInputType::value("Solid Color", TaggedValue::OptionalColor(None), false),
				DocumentInputType::value("Gradient Type", TaggedValue::GradientType(vector::style::GradientType::Linear), false),
				DocumentInputType::value("Start", TaggedValue::DVec2(DVec2::new(0., 0.5)), false),
				DocumentInputType::value("End", TaggedValue::DVec2(DVec2::new(1., 0.5)), false),
				DocumentInputType::value("Transform", TaggedValue::DAffine2(DAffine2::IDENTITY), false),
				DocumentInputType::value("Positions", TaggedValue::GradientPositions(vec![(0., Some(Color::BLACK)), (1., Some(Color::WHITE))]), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Subpath)],
			properties: node_properties::fill_properties,
		},
		DocumentNodeType {
			name: "Stroke",
			category: "Vector",
			identifier: NodeImplementation::proto("graphene_core::vector::SetStrokeNode<_, _, _, _, _, _, _>"),
			inputs: vec![
				DocumentInputType::value("Vector Data", TaggedValue::VectorData(graphene_core::vector::VectorData::empty()), true),
				DocumentInputType::value("Color", TaggedValue::OptionalColor(Some(Color::BLACK)), false),
				DocumentInputType::value("Weight", TaggedValue::F64(0.), false),
				DocumentInputType::value("Dash Lengths", TaggedValue::VecF32(Vec::new()), false),
				DocumentInputType::value("Dash Offset", TaggedValue::F64(0.), false),
				DocumentInputType::value("Line Cap", TaggedValue::LineCap(graphene_core::vector::style::LineCap::Butt), false),
				DocumentInputType::value("Line Join", TaggedValue::LineJoin(graphene_core::vector::style::LineJoin::Miter), false),
				DocumentInputType::value("Miter Limit", TaggedValue::F64(4.), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Subpath)],
			properties: node_properties::stroke_properties,
		},
		DocumentNodeType {
			name: "Image Segmentation",
			category: "Image Adjustments",
			identifier: NodeImplementation::proto("graphene_std::image_segmentation::ImageSegmentationNode<_>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Mask", TaggedValue::ImageFrame(ImageFrame::empty()), true),
			],
			outputs: vec![DocumentOutputType::new("Segments", FrontendGraphDataType::Raster)],
			properties: node_properties::no_properties,
		},
		DocumentNodeType {
			name: "Index",
			category: "Image Adjustments",
			identifier: NodeImplementation::proto("graphene_core::raster::IndexNode<_>"),
			inputs: vec![
				DocumentInputType::value("Segmentation", TaggedValue::Segments(vec![ImageFrame::empty()]), true),
				DocumentInputType::value("Index", TaggedValue::U32(0), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::index_node_properties,
		},
	]
}

pub static IMAGINATE_NODE: Lazy<DocumentNodeType> = Lazy::new(|| DocumentNodeType {
	name: "Imaginate",
	category: "Image Synthesis",
	identifier: NodeImplementation::proto("graphene_std::raster::ImaginateNode<_>"),
	inputs: vec![
		DocumentInputType::value("Input Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
		DocumentInputType::value("Seed", TaggedValue::F64(0.), false), // Remember to keep index used in `ImaginateRandom` updated with this entry's index
		DocumentInputType::value("Resolution", TaggedValue::OptionalDVec2(None), false),
		DocumentInputType::value("Samples", TaggedValue::F64(30.), false),
		DocumentInputType::value("Sampling Method", TaggedValue::ImaginateSamplingMethod(ImaginateSamplingMethod::EulerA), false),
		DocumentInputType::value("Prompt Guidance", TaggedValue::F64(7.5), false),
		DocumentInputType::value("Prompt", TaggedValue::String(String::new()), false),
		DocumentInputType::value("Negative Prompt", TaggedValue::String(String::new()), false),
		DocumentInputType::value("Adapt Input Image", TaggedValue::Bool(false), false),
		DocumentInputType::value("Image Creativity", TaggedValue::F64(66.), false),
		DocumentInputType::value("Masking Layer", TaggedValue::LayerPath(None), false),
		DocumentInputType::value("Inpaint", TaggedValue::Bool(true), false),
		DocumentInputType::value("Mask Blur", TaggedValue::F64(4.), false),
		DocumentInputType::value("Mask Starting Fill", TaggedValue::ImaginateMaskStartingFill(ImaginateMaskStartingFill::Fill), false),
		DocumentInputType::value("Improve Faces", TaggedValue::Bool(false), false),
		DocumentInputType::value("Tiling", TaggedValue::Bool(false), false),
		// Non-user status (is document input the right way to do this?)
		DocumentInputType::value("Cached Data", TaggedValue::RcImage(None), false),
		DocumentInputType::value("Percent Complete", TaggedValue::F64(0.), false),
		DocumentInputType::value("Status", TaggedValue::ImaginateStatus(ImaginateStatus::Idle), false),
	],
	outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
	properties: node_properties::imaginate_properties,
});

pub fn resolve_document_node_type(name: &str) -> Option<&DocumentNodeType> {
	DOCUMENT_NODE_TYPES.iter().find(|node| node.name == name)
}

pub fn collect_node_types() -> Vec<FrontendNodeType> {
	DOCUMENT_NODE_TYPES
		.iter()
		.filter(|node_type| !node_type.category.eq_ignore_ascii_case("ignore"))
		.map(|node_type| FrontendNodeType::new(node_type.name, node_type.category))
		.collect()
}

impl DocumentNodeType {
	/// Generate a [`DocumentNodeImplementation`] from this node type, using a nested network.
	pub fn generate_implementation(&self) -> DocumentNodeImplementation {
		let num_inputs = self.inputs.len();

		let inner_network = match &self.identifier {
			NodeImplementation::DocumentNode(network) => network.clone(),
			NodeImplementation::ProtoNode(ident) => {
				NodeNetwork {
					inputs: (0..num_inputs).map(|_| 0).collect(),
					outputs: vec![NodeOutput::new(0, 0)],
					nodes: [(
						0,
						DocumentNode {
							name: format!("{}_impl", self.name),
							// TODO: Allow inserting nodes that contain other nodes.
							implementation: DocumentNodeImplementation::Unresolved(ident.clone()),
							inputs: self.inputs.iter().map(|i| NodeInput::Network(i.default.ty())).collect(),
							..Default::default()
						},
					)]
					.into_iter()
					.collect(),
					..Default::default()
				}
			}
			NodeImplementation::Extract => NodeNetwork {
				inputs: (0..num_inputs).map(|_| 0).collect(),
				outputs: vec![NodeOutput::new(0, 0)],
				nodes: [(
					0,
					DocumentNode {
						name: "ExtractNode".to_string(),
						implementation: DocumentNodeImplementation::Extract,
						inputs: self.inputs.iter().map(|i| NodeInput::Network(i.default.ty())).collect(),
						..Default::default()
					},
				)]
				.into_iter()
				.collect(),
				..Default::default()
			},
		};

		DocumentNodeImplementation::Network(inner_network)
	}

	/// Converts the [DocumentNodeType] type to a [DocumentNode], based on the inputs from the graph (which must be the correct length) and the metadata
	pub fn to_document_node(&self, inputs: impl IntoIterator<Item = NodeInput>, metadata: graph_craft::document::DocumentNodeMetadata) -> DocumentNode {
		let inputs: Vec<_> = inputs.into_iter().collect();
		assert_eq!(inputs.len(), self.inputs.len(), "Inputs passed from the graph must be equal to the number required");
		DocumentNode {
			name: self.name.to_string(),
			inputs,
			implementation: self.generate_implementation(),
			metadata,
			..Default::default()
		}
	}

	/// Converts the [DocumentNodeType] type to a [DocumentNode], using the provided `input_override` and falling back to the default inputs.
	/// `input_override` does not have to be the correct length.
	pub fn to_document_node_default_inputs(&self, input_override: impl IntoIterator<Item = Option<NodeInput>>, metadata: graph_craft::document::DocumentNodeMetadata) -> DocumentNode {
		let mut input_override = input_override.into_iter();
		let inputs = self.inputs.iter().map(|default| input_override.next().unwrap_or_default().unwrap_or_else(|| default.default.clone()));
		self.to_document_node(inputs, metadata)
	}
}

pub fn wrap_network_in_scope(mut network: NodeNetwork) -> NodeNetwork {
	let node_ids = network.nodes.keys().copied().collect::<Vec<_>>();

	network.generate_node_paths(&[]);
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
				assert_eq!(input, input_type.as_ref().unwrap(), "Networks wrapped in scope must have the same input type");
				network_inputs.push(*id);
			}
		}
	}
	let len = network_inputs.len();
	network.inputs = network_inputs;

	// if the network has no inputs, it doesn't need to be wrapped in a scope
	if len == 0 {
		return network;
	}

	let inner_network = DocumentNode {
		name: "Scope".to_string(),
		implementation: DocumentNodeImplementation::Network(network),
		inputs: core::iter::repeat(NodeInput::node(0, 1)).take(len).collect(),
		..Default::default()
	};

	// wrap the inner network in a scope
	let nodes = vec![
		resolve_document_node_type("Begin Scope")
			.expect("Begin Scope node type not found")
			.to_document_node(vec![input_type.unwrap()], DocumentNodeMetadata::default()),
		inner_network,
		resolve_document_node_type("End Scope")
			.expect("End Scope node type not found")
			.to_document_node(vec![NodeInput::node(0, 0), NodeInput::node(1, 0)], DocumentNodeMetadata::default()),
	];

	NodeNetwork {
		inputs: vec![0],
		outputs: vec![NodeOutput::new(2, 0)],
		nodes: nodes.into_iter().enumerate().map(|(id, node)| (id as NodeId, node)).collect(),
		..Default::default()
	}
}

pub fn new_image_network(output_offset: i32, output_node_id: NodeId) -> NodeNetwork {
	let mut network = NodeNetwork {
		inputs: vec![0],
		..Default::default()
	};
	network.push_node(
		resolve_document_node_type("Input Frame")
			.expect("Input Frame node does not exist")
			.to_document_node_default_inputs([], DocumentNodeMetadata::position((8, 4))),
		false,
	);
	network.push_node(
		resolve_document_node_type("Output")
			.expect("Output node does not exist")
			.to_document_node([NodeInput::node(output_node_id, 0)], DocumentNodeMetadata::position((output_offset + 8, 4))),
		false,
	);
	network
}

pub fn new_vector_network(subpaths: Vec<bezier_rs::Subpath<uuid::ManipulatorGroupId>>) -> NodeNetwork {
	let path_generator = resolve_document_node_type("Path Generator").expect("Path Generator node does not exist");
	let transform = resolve_document_node_type("Transform").expect("Transform node does not exist");
	let fill = resolve_document_node_type("Fill").expect("Fill node does not exist");
	let stroke = resolve_document_node_type("Stroke").expect("Stroke node does not exist");
	let output = resolve_document_node_type("Output").expect("Output node does not exist");

	let mut network = NodeNetwork {
		inputs: vec![0],
		..Default::default()
	};

	network.push_node(
		path_generator.to_document_node_default_inputs([Some(NodeInput::value(TaggedValue::Subpaths(subpaths), false))], DocumentNodeMetadata::position((0, 4))),
		false,
	);
	network.push_node(transform.to_document_node_default_inputs([None], Default::default()), true);
	network.push_node(fill.to_document_node_default_inputs([None], Default::default()), true);
	network.push_node(stroke.to_document_node_default_inputs([None], Default::default()), true);
	network.push_node(output.to_document_node_default_inputs([None], Default::default()), true);
	network
}

pub fn new_text_network(text: String, font: Font, size: f64) -> NodeNetwork {
	let text_generator = resolve_document_node_type("Text").expect("Text node does not exist");
	let transform = resolve_document_node_type("Transform").expect("Transform node does not exist");
	let fill = resolve_document_node_type("Fill").expect("Fill node does not exist");
	let stroke = resolve_document_node_type("Stroke").expect("Stroke node does not exist");
	let output = resolve_document_node_type("Output").expect("Output node does not exist");

	let mut network = NodeNetwork {
		inputs: vec![0],
		..Default::default()
	};
	network.push_node(
		text_generator.to_document_node(
			[
				NodeInput::Network(concrete!(graphene_core::EditorApi)),
				NodeInput::value(TaggedValue::String(text), false),
				NodeInput::value(TaggedValue::Font(font), false),
				NodeInput::value(TaggedValue::F64(size), false),
			],
			DocumentNodeMetadata::position((0, 4)),
		),
		false,
	);
	network.push_node(transform.to_document_node_default_inputs([None], Default::default()), true);
	network.push_node(fill.to_document_node_default_inputs([None], Default::default()), true);
	network.push_node(stroke.to_document_node_default_inputs([None], Default::default()), true);
	network.push_node(output.to_document_node_default_inputs([None], Default::default()), true);
	network
}
