use super::{node_properties, FrontendGraphDataType, FrontendNodeType};
use crate::messages::layout::utility_types::layout_widget::LayoutGroup;
use crate::node_graph_executor::NodeGraphExecutor;
use once_cell::sync::Lazy;

use graph_craft::document::value::*;
use graph_craft::document::*;
use graph_craft::imaginate_input::ImaginateSamplingMethod;

use graph_craft::concrete;
use graph_craft::NodeIdentifier;
use graphene_core::raster::{BlendMode, Color, Image, ImageFrame, LuminanceCalculation};
use graphene_core::*;

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

	pub const fn _none() -> Self {
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
			category: "General",
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
			name: "Image",
			category: "Ignore",
			identifier: NodeImplementation::proto("graphene_core::ops::IdNode"),
			inputs: vec![DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), false)],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: |_document_node, _node_id, _context| node_properties::string_properties("A bitmap image embedded in this node"),
		},
		// DocumentNodeType {
		// 	name: "Input",
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
			name: "Input",
			category: "Ignore",
			identifier: NodeImplementation::DocumentNode(NodeNetwork {
				inputs: vec![0, 1],
				outputs: vec![NodeOutput::new(0, 0), NodeOutput::new(1, 0)],
				nodes: [DocumentNode {
					name: "Identity".to_string(),
					inputs: vec![NodeInput::Network(concrete!(ImageFrame))],
					implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::ops::IdNode")),
					metadata: Default::default(),
				}]
				.into_iter()
				.enumerate()
				.map(|(id, node)| (id as NodeId, node))
				.collect(),
				..Default::default()
			}),
			inputs: vec![
				DocumentInputType {
					name: "In",
					data_type: FrontendGraphDataType::General,
					default: NodeInput::Network(concrete!(ImageFrame)),
				},
				DocumentInputType::value("Transform", TaggedValue::DAffine2(DAffine2::IDENTITY), false),
			],
			outputs: vec![DocumentOutputType {
				name: "Image Frame",
				data_type: FrontendGraphDataType::Raster,
			}],
			properties: node_properties::input_properties,
		},
		DocumentNodeType {
			name: "Begin Scope",
			category: "Structural",
			identifier: NodeImplementation::DocumentNode(NodeNetwork {
				inputs: vec![0, 2],
				outputs: vec![NodeOutput::new(1, 0), NodeOutput::new(3, 0)],
				nodes: [
					DocumentNode {
						name: "SetNode".to_string(),
						inputs: vec![NodeInput::Network(concrete!(ImageFrame))],
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::ops::SomeNode")),
						metadata: Default::default(),
					},
					DocumentNode {
						name: "LetNode".to_string(),
						inputs: vec![NodeInput::node(0, 0)],
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_std::memo::LetNode<_>")),
						metadata: Default::default(),
					},
					DocumentNode {
						name: "RefNode".to_string(),
						inputs: vec![NodeInput::Network(concrete!(())), NodeInput::lambda(1, 0)],
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_std::memo::RefNode<_, _>")),
						metadata: Default::default(),
					},
					DocumentNode {
						name: "CloneNode".to_string(),
						inputs: vec![NodeInput::node(2, 0)],
						implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::ops::CloneNode<_>")),
						metadata: Default::default(),
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
				default: NodeInput::value(TaggedValue::ImageFrame(ImageFrame::empty()), true),
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
			category: "Structural",
			identifier: NodeImplementation::proto("graphene_std::memo::EndLetNode<_>"),
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

			properties: |_document_node, _node_id, _context| node_properties::string_properties("The graph's output is rendered into the frame"),
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
			properties: |_document_node, _node_id, _context| node_properties::string_properties("The graph's output is rendered into the frame"),
		},
		DocumentNodeType {
			name: "Image Frame",
			category: "General",
			identifier: NodeImplementation::proto("graphene_std::raster::ImageFrameNode<_>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::Image(Image::empty()), true),
				DocumentInputType::value("Transform", TaggedValue::DAffine2(DAffine2::IDENTITY), true),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: |_document_node, _node_id, _context| node_properties::string_properties("Creates an embedded image with the given transform"),
		},
		DocumentNodeType {
			name: "Blend Node",
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
					default: NodeInput::value(TaggedValue::F64(50.), false),
				},
				DocumentInputType {
					name: "Yellows",
					data_type: FrontendGraphDataType::Number,
					default: NodeInput::value(TaggedValue::F64(50.), false),
				},
				DocumentInputType {
					name: "Greens",
					data_type: FrontendGraphDataType::Number,
					default: NodeInput::value(TaggedValue::F64(50.), false),
				},
				DocumentInputType {
					name: "Cyans",
					data_type: FrontendGraphDataType::Number,
					default: NodeInput::value(TaggedValue::F64(50.), false),
				},
				DocumentInputType {
					name: "Blues",
					data_type: FrontendGraphDataType::Number,
					default: NodeInput::value(TaggedValue::F64(50.), false),
				},
				DocumentInputType {
					name: "Magentas",
					data_type: FrontendGraphDataType::Number,
					default: NodeInput::value(TaggedValue::F64(50.), false),
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
				DocumentInputType::value("Luma Calculation", TaggedValue::LuminanceCalculation(LuminanceCalculation::SRGB), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::luminance_properties,
		},
		DocumentNodeType {
			name: "Gaussian Blur",
			category: "Image Filters",
			identifier: NodeImplementation::DocumentNode(NodeNetwork {
				inputs: vec![0, 1, 1],
				outputs: vec![NodeOutput::new(1, 0)],
				nodes: vec![
					(
						0,
						DocumentNode {
							name: "CacheNode".to_string(),
							inputs: vec![NodeInput::Network(concrete!(Image))],
							implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_std::memo::CacheNode")),
							metadata: Default::default(),
						},
					),
					(
						1,
						DocumentNode {
							name: "BlurNode".to_string(),
							inputs: vec![NodeInput::node(0, 0), NodeInput::Network(concrete!(u32)), NodeInput::Network(concrete!(f64)), NodeInput::node(0, 0)],
							implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::raster::BlurNode")),
							metadata: Default::default(),
						},
					),
				]
				.into_iter()
				.collect(),
				..Default::default()
			}),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Radius", TaggedValue::U32(3), false),
				DocumentInputType::value("Sigma", TaggedValue::F64(1.), false),
			],
			outputs: vec![DocumentOutputType {
				name: "Image",
				data_type: FrontendGraphDataType::Raster,
			}],
			properties: node_properties::blur_image_properties,
		},
		DocumentNodeType {
			name: "Cache",
			category: "Structural",
			identifier: NodeImplementation::DocumentNode(NodeNetwork {
				inputs: vec![0],
				outputs: vec![NodeOutput::new(1, 0)],
				nodes: vec![
					(
						0,
						DocumentNode {
							name: "CacheNode".to_string(),
							inputs: vec![NodeInput::Network(concrete!(Image))],
							implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_std::memo::CacheNode")),
							metadata: Default::default(),
						},
					),
					(
						1,
						DocumentNode {
							name: "CloneNode".to_string(),
							inputs: vec![NodeInput::node(0, 0)],
							implementation: DocumentNodeImplementation::Unresolved(NodeIdentifier::new("graphene_core::ops::CloneNode<_>")),
							metadata: Default::default(),
						},
					),
				]
				.into_iter()
				.collect(),
				..Default::default()
			}),
			inputs: vec![DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true)],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::no_properties,
		},
		#[cfg(feature = "gpu")]
		DocumentNodeType {
			name: "GpuImage",
			category: "Image Adjustments",
			identifier: NodeImplementation::proto("graphene_std::executor::MapGpuSingleImageNode"),
			inputs: vec![
				DocumentInputType::new("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType {
					name: "Path",
					data_type: FrontendGraphDataType::Text,
					default: NodeInput::value(TaggedValue::String(String::new()), false),
				},
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::gpu_map_properties,
		},
		#[cfg(feature = "quantization")]
		DocumentNodeType {
			name: "QuantizeImage",
			category: "Image Adjustments",
			identifier: NodeImplementation::proto("graphene_std::quantization::GenerateQuantizationNode"),
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
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
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
			identifier: NodeImplementation::proto("graphene_core::raster::BrightnessContrastNode<_, _>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Brightness", TaggedValue::F64(0.), false),
				DocumentInputType::value("Contrast", TaggedValue::F64(0.), false),
			],
			outputs: vec![DocumentOutputType::new("Image", FrontendGraphDataType::Raster)],
			properties: node_properties::brighten_image_properties,
		},
		DocumentNodeType {
			name: "Threshold",
			category: "Image Adjustments",
			identifier: NodeImplementation::proto("graphene_core::raster::ThresholdNode<_, _>"),
			inputs: vec![
				DocumentInputType::value("Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
				DocumentInputType::value("Luma Calculation", TaggedValue::LuminanceCalculation(LuminanceCalculation::SRGB), false),
				DocumentInputType::value("Threshold", TaggedValue::F64(50.), false),
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
		/*DocumentNodeType {
			name: "Unit Circle Generator",
			category: "Vector",
			identifier: NodeImplementation::proto("graphene_std::vector::generator_nodes::UnitCircleGenerator", &[]),
			inputs: vec![DocumentInputType::none()],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Subpath)],
			properties: node_properties::no_properties,
		},
		DocumentNodeType {
			name: "Unit Square Generator",
			category: "Vector",
			identifier: NodeImplementation::proto("graphene_std::vector::generator_nodes::UnitSquareGenerator", &[]),
			inputs: vec![DocumentInputType::none()],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Subpath)],
			properties: node_properties::no_properties,
		},
		DocumentNodeType {
			name: "Path Generator",
			category: "Vector",
			identifier: NodeImplementation::proto("graphene_core::ops::IdNode"),
			inputs: vec![DocumentInputType {
				name: "Path Data",
				data_type: FrontendGraphDataType::Subpath,
				default: NodeInput::value(TaggedValue::Subpath(Subpath::new()), false),
			}],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Subpath)],
			properties: node_properties::no_properties,
		},
		DocumentNodeType {
			name: "Transform Subpath",
			category: "Vector",
			identifier: NodeImplementation::proto("graphene_std::vector::generator_nodes::TransformSubpathNode", &[]),
			inputs: vec![
				DocumentInputType::new("Subpath", TaggedValue::Subpath(Subpath::empty()), true),
				DocumentInputType::new("Translation", TaggedValue::DVec2(DVec2::ZERO), false),
				DocumentInputType::new("Rotation", TaggedValue::F64(0.), false),
				DocumentInputType::new("Scale", TaggedValue::DVec2(DVec2::ONE), false),
				DocumentInputType::new("Skew", TaggedValue::DVec2(DVec2::ZERO), false),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Subpath)],
			properties: node_properties::transform_properties,
		},
		DocumentNodeType {
			name: "Blit Subpath",
			category: "Vector",
			identifier: NodeImplementation::proto("graphene_std::vector::generator_nodes::BlitSubpath", &[]),
			inputs: vec![
				DocumentInputType::new("Image", TaggedValue::Image(Image::empty()), true),
				DocumentInputType::new("Subpath", TaggedValue::Subpath(Subpath::empty()), true),
			],
			outputs: vec![DocumentOutputType::new("Vector", FrontendGraphDataType::Raster)],
			properties: node_properties::no_properties,
		},*/
	]
}

pub static IMAGINATE_NODE: Lazy<DocumentNodeType> = Lazy::new(|| DocumentNodeType {
	name: "Imaginate",
	category: "Image Synthesis",
	identifier: NodeImplementation::proto("graphene_std::raster::ImaginateNode<_>"),
	inputs: vec![
		DocumentInputType::value("Input Image", TaggedValue::ImageFrame(ImageFrame::empty()), true),
		DocumentInputType::value("Transform", TaggedValue::DAffine2(DAffine2::IDENTITY), false),
		DocumentInputType::value("Seed", TaggedValue::F64(0.), false), // Remember to keep index used in `NodeGraphFrameImaginateRandom` updated with this entry's index
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
							metadata: DocumentNodeMetadata::default(),
						},
					)]
					.into_iter()
					.collect(),
					..Default::default()
				}
			}
			NodeImplementation::DocumentNode(network) => network.clone(),
		};

		DocumentNodeImplementation::Network(inner_network)
	}

	pub fn to_document_node(&self, inputs: impl IntoIterator<Item = NodeInput>, metadata: graph_craft::document::DocumentNodeMetadata) -> DocumentNode {
		DocumentNode {
			name: self.name.to_string(),
			inputs: inputs.into_iter().collect(),
			implementation: self.generate_implementation(),
			metadata,
		}
	}
}

pub fn wrap_network_in_scope(network: NodeNetwork) -> NodeNetwork {
	assert_eq!(network.inputs.len(), 1, "Networks wrapped in scope must have exactly one input");
	let input_type = network.nodes[&network.inputs[0]].inputs.iter().find(|&i| matches!(i, NodeInput::Network(_))).unwrap().clone();

	let inner_network = DocumentNode {
		name: "Scope".to_string(),
		implementation: DocumentNodeImplementation::Network(network),
		inputs: vec![NodeInput::node(0, 1)],
		metadata: DocumentNodeMetadata::default(),
	};
	// wrap the inner network in a scope
	let nodes = vec![
		resolve_document_node_type("Begin Scope")
			.expect("Begin Scope node type not found")
			.to_document_node(vec![input_type.clone()], DocumentNodeMetadata::default()),
		inner_network,
		resolve_document_node_type("End Scope")
			.expect("End Scope node type not found")
			.to_document_node(vec![NodeInput::node(0, 0), NodeInput::node(1, 0)], DocumentNodeMetadata::default()),
	];
	let network = NodeNetwork {
		inputs: vec![0],
		outputs: vec![NodeOutput::new(2, 0)],
		nodes: nodes.into_iter().enumerate().map(|(id, node)| (id as NodeId, node)).collect(),
		..Default::default()
	};
	network
}

pub fn new_image_network(output_offset: i32, output_node_id: NodeId) -> NodeNetwork {
	NodeNetwork {
		inputs: vec![0],
		outputs: vec![NodeOutput::new(1, 0)],
		nodes: [
			resolve_document_node_type("Input")
				.expect("Input node does not exist")
				.to_document_node([NodeInput::Network(concrete!(ImageFrame))], DocumentNodeMetadata::position((8, 4))),
			resolve_document_node_type("Output")
				.expect("Output node does not exist")
				.to_document_node([NodeInput::node(output_node_id, 0)], DocumentNodeMetadata::position((output_offset + 8, 4))),
		]
		.into_iter()
		.enumerate()
		.map(|(id, node)| (id as NodeId, node))
		.collect(),
		..Default::default()
	}
}
