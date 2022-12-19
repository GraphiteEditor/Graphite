use std::collections::VecDeque;

use super::{node_properties, FrontendGraphDataType, FrontendNodeType};
use crate::messages::layout::utility_types::layout_widget::LayoutGroup;
use graph_craft::concrete;
use graph_craft::document::value::*;
use graph_craft::document::{DocumentNode, NodeId, NodeInput};
use graph_craft::imaginate_input::ImaginateSamplingMethod;
use graph_craft::proto::{NodeIdentifier, Type};
use graphene_core::raster::Image;

pub struct DocumentInputType {
	pub name: &'static str,
	pub data_type: FrontendGraphDataType,
	pub default: NodeInput,
}

impl DocumentInputType {
	pub const fn new(name: &'static str, tagged_value: TaggedValue, exposed: bool) -> Self {
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

pub struct NodePropertiesContext<'a> {
	pub persistent_data: &'a crate::messages::portfolio::utility_types::PersistentData,
	pub document: &'a graphene::document::Document,
	pub responses: &'a mut VecDeque<crate::messages::prelude::Message>,
	pub layer_path: &'a [graphene::LayerId],
	pub nested_path: &'a [NodeId],
}

pub struct DocumentNodeType {
	pub name: &'static str,
	pub category: &'static str,
	pub identifier: NodeIdentifier,
	pub inputs: &'static [DocumentInputType],
	pub outputs: &'static [FrontendGraphDataType],
	pub properties: fn(&DocumentNode, NodeId, &mut NodePropertiesContext) -> Vec<LayoutGroup>,
}

// TODO: Dynamic node library
static DOCUMENT_NODE_TYPES: &[DocumentNodeType] = &[
	DocumentNodeType {
		name: "Identity",
		category: "General",
		identifier: NodeIdentifier::new("graphene_core::ops::IdNode", &[concrete!("Any<'_>")]),
		inputs: &[DocumentInputType {
			name: "In",
			data_type: FrontendGraphDataType::General,
			default: NodeInput::Node(0),
		}],
		outputs: &[FrontendGraphDataType::General],
		properties: |_document_node, _node_id, _context| node_properties::string_properties("The identity node simply returns the input".to_string()),
	},
	DocumentNodeType {
		name: "Input",
		category: "Meta",
		identifier: NodeIdentifier::new("graphene_core::ops::IdNode", &[concrete!("Any<'_>")]),
		inputs: &[DocumentInputType {
			name: "In",
			data_type: FrontendGraphDataType::Raster,
			default: NodeInput::Network,
		}],
		outputs: &[FrontendGraphDataType::Raster],
		properties: |_document_node, _node_id, _context| node_properties::string_properties("The input to the graph is the bitmap under the frame".to_string()),
	},
	DocumentNodeType {
		name: "Output",
		category: "Meta",
		identifier: NodeIdentifier::new("graphene_core::ops::IdNode", &[concrete!("Any<'_>")]),
		inputs: &[DocumentInputType {
			name: "In",
			data_type: FrontendGraphDataType::Raster,
			default: NodeInput::value(TaggedValue::Image(Image::empty()), true),
		}],
		outputs: &[],
		properties: |_document_node, _node_id, _context| node_properties::string_properties("The output to the graph is rendered in the frame".to_string()),
	},
	DocumentNodeType {
		name: "Grayscale",
		category: "Image Adjustments",
		identifier: NodeIdentifier::new("graphene_std::raster::GrayscaleNode", &[]),
		inputs: &[DocumentInputType::new("Image", TaggedValue::Image(Image::empty()), true)],
		outputs: &[FrontendGraphDataType::Raster],
		properties: node_properties::no_properties,
	},
	DocumentNodeType {
		name: "GpuImage",
		category: "Image Adjustments",
		identifier: NodeIdentifier::new("graphene_std::executor::MapGpuSingleImageNode", &[concrete!("&TypeErasedNode")]),
		inputs: &[
			DocumentInputType::new("Image", TaggedValue::Image(Image::empty()), true),
			DocumentInputType {
				name: "Path",
				data_type: FrontendGraphDataType::Text,
				default: NodeInput::value(TaggedValue::String(String::new()), true),
			},
		],
		outputs: &[FrontendGraphDataType::Raster],
		properties: node_properties::gpu_map_properties,
	},
	DocumentNodeType {
		name: "Invert RGB",
		category: "Image Adjustments",
		identifier: NodeIdentifier::new("graphene_std::raster::InvertRGBNode", &[]),
		inputs: &[DocumentInputType::new("Image", TaggedValue::Image(Image::empty()), true)],
		outputs: &[FrontendGraphDataType::Raster],
		properties: node_properties::no_properties,
	},
	DocumentNodeType {
		name: "Hue/Saturation",
		category: "Image Adjustments",
		identifier: NodeIdentifier::new("graphene_std::raster::HueSaturationNode", &[concrete!("&TypeErasedNode")]),
		inputs: &[
			DocumentInputType::new("Image", TaggedValue::Image(Image::empty()), true),
			DocumentInputType::new("Hue Shift", TaggedValue::F64(0.), false),
			DocumentInputType::new("Saturation Shift", TaggedValue::F64(0.), false),
			DocumentInputType::new("Lightness Shift", TaggedValue::F64(0.), false),
		],
		outputs: &[FrontendGraphDataType::Raster],
		properties: node_properties::adjust_hsl_properties,
	},
	DocumentNodeType {
		name: "Brightness/Contrast",
		category: "Image Adjustments",
		identifier: NodeIdentifier::new("graphene_std::raster::BrightnessContrastNode", &[concrete!("&TypeErasedNode")]),
		inputs: &[
			DocumentInputType::new("Image", TaggedValue::Image(Image::empty()), true),
			DocumentInputType::new("Brightness", TaggedValue::F64(0.), false),
			DocumentInputType::new("Contrast", TaggedValue::F64(0.), false),
		],
		outputs: &[FrontendGraphDataType::Raster],
		properties: node_properties::brighten_image_properties,
	},
	DocumentNodeType {
		name: "Gamma",
		category: "Image Adjustments",
		identifier: NodeIdentifier::new("graphene_std::raster::GammaNode", &[concrete!("&TypeErasedNode")]),
		inputs: &[
			DocumentInputType::new("Image", TaggedValue::Image(Image::empty()), true),
			DocumentInputType::new("Gamma", TaggedValue::F64(1.), false),
		],
		outputs: &[FrontendGraphDataType::Raster],
		properties: node_properties::adjust_gamma_properties,
	},
	DocumentNodeType {
		name: "Opacity",
		category: "Image Adjustments",
		identifier: NodeIdentifier::new("graphene_std::raster::OpacityNode", &[concrete!("&TypeErasedNode")]),
		inputs: &[
			DocumentInputType::new("Image", TaggedValue::Image(Image::empty()), true),
			DocumentInputType::new("Factor", TaggedValue::F64(1.), false),
		],
		outputs: &[FrontendGraphDataType::Raster],
		properties: node_properties::multiply_opacity,
	},
	DocumentNodeType {
		name: "Posterize",
		category: "Image Adjustments",
		identifier: NodeIdentifier::new("graphene_std::raster::PosterizeNode", &[concrete!("&TypeErasedNode")]),
		inputs: &[
			DocumentInputType::new("Image", TaggedValue::Image(Image::empty()), true),
			DocumentInputType::new("Value", TaggedValue::F64(5.), false),
		],
		outputs: &[FrontendGraphDataType::Raster],
		properties: node_properties::posterize_properties,
	},
	DocumentNodeType {
		name: "Exposure",
		category: "Image Adjustments",
		identifier: NodeIdentifier::new("graphene_std::raster::ExposureNode", &[concrete!("&TypeErasedNode")]),
		inputs: &[
			DocumentInputType::new("Image", TaggedValue::Image(Image::empty()), true),
			DocumentInputType::new("Value", TaggedValue::F64(0.), false),
		],
		outputs: &[FrontendGraphDataType::Raster],
		properties: node_properties::exposure_properties,
	},
	IMAGINATE_NODE,
	DocumentNodeType {
		name: "Add",
		category: "Mathematics",
		identifier: NodeIdentifier::new("graphene_core::ops::AddNode", &[concrete!("&TypeErasedNode")]),
		inputs: &[
			DocumentInputType::new("Input", TaggedValue::F64(0.), true),
			DocumentInputType::new("Addend", TaggedValue::F64(0.), true),
		],
		outputs: &[FrontendGraphDataType::Number],
		properties: node_properties::add_properties,
	},
	/*DocumentNodeType {
		name: "Unit Circle Generator",
		category: "Vector",
		identifier: NodeIdentifier::new("graphene_std::vector::generator_nodes::UnitCircleGenerator", &[]),
		inputs: &[DocumentInputType::none()],
		outputs: &[FrontendGraphDataType::Subpath],
		properties: node_properties::no_properties,
	},
	DocumentNodeType {
		name: "Unit Square Generator",
		category: "Vector",
		identifier: NodeIdentifier::new("graphene_std::vector::generator_nodes::UnitSquareGenerator", &[]),
		inputs: &[DocumentInputType::none()],
		outputs: &[FrontendGraphDataType::Subpath],
		properties: node_properties::no_properties,
	},
	DocumentNodeType {
		name: "Path Generator",
		category: "Vector",
		identifier: NodeIdentifier::new("graphene_core::ops::IdNode", &[concrete!("Any<'_>")]),
		inputs: &[DocumentInputType {
			name: "Path Data",
			data_type: FrontendGraphDataType::Subpath,
			default: NodeInput::value(TaggedValue::Subpath(Subpath::new()), false),
		}],
		outputs: &[FrontendGraphDataType::Subpath],
		properties: node_properties::no_properties,
	},
	DocumentNodeType {
		name: "Transform Subpath",
		category: "Vector",
		identifier: NodeIdentifier::new("graphene_std::vector::generator_nodes::TransformSubpathNode", &[]),
		inputs: &[
			DocumentInputType {
				name: "Subpath",
				data_type: FrontendGraphDataType::Subpath,
				default: NodeInput::value(TaggedValue::Subpath(Subpath::new()), true),
			},
			DocumentInputType {
				name: "Translation",
				data_type: FrontendGraphDataType::Vector,
				default: NodeInput::value(TaggedValue::DVec2(DVec2::ZERO), false),
			},
			DocumentInputType {
				name: "Rotation",
				data_type: FrontendGraphDataType::Number,
				default: NodeInput::value(TaggedValue::F64(0.), false),
			},
			DocumentInputType {
				name: "Scale",
				data_type: FrontendGraphDataType::Vector,
				default: NodeInput::value(TaggedValue::DVec2(DVec2::ONE), false),
			},
			DocumentInputType {
				name: "Skew",
				data_type: FrontendGraphDataType::Vector,
				default: NodeInput::value(TaggedValue::DVec2(DVec2::ZERO), false),
			},
		],
		outputs: &[FrontendGraphDataType::Subpath],
		properties: node_properties::transform_properties,
	},
	DocumentNodeType {
		name: "Blit Subpath",
		category: "Vector",
		identifier: NodeIdentifier::new("graphene_std::vector::generator_nodes::BlitSubpath", &[]),
		inputs: &[
			DocumentInputType {
				name: "Image",
				data_type: FrontendGraphDataType::Raster,
				default: NodeInput::value(TaggedValue::Image(Image::empty()), true),
			},
			DocumentInputType {
				name: "Subpath",
				data_type: FrontendGraphDataType::Subpath,
				default: NodeInput::value(TaggedValue::Subpath(Subpath::new()), true),
			},
		],
		outputs: &[FrontendGraphDataType::Raster],
		properties: node_properties::no_properties,
	},*/
];

pub const IMAGINATE_NODE: DocumentNodeType = DocumentNodeType {
	name: "Imaginate",
	category: "Image Synthesis",
	identifier: NodeIdentifier::new("graphene_std::raster::ImaginateNode", &[concrete!("&TypeErasedNode")]),
	inputs: &[
		DocumentInputType::new("Base Image", TaggedValue::Image(Image::empty()), true),
		DocumentInputType::new("Seed", TaggedValue::F64(0.), false),
		DocumentInputType::new("Resolution", TaggedValue::OptionalDVec2(None), false),
		DocumentInputType::new("Samples", TaggedValue::F64(30.), false),
		DocumentInputType::new("Sampling Method", TaggedValue::ImaginateSamplingMethod(ImaginateSamplingMethod::EulerA), false),
		DocumentInputType::new("Text Guidance", TaggedValue::F64(10.), false),
		DocumentInputType::new("Text Prompt", TaggedValue::String(String::new()), false),
		DocumentInputType::new("Neg. Prompt", TaggedValue::String(String::new()), false),
		DocumentInputType::new("Use Base Image", TaggedValue::Bool(true), false),
		DocumentInputType::new("Image Creativity", TaggedValue::F64(66.), false),
		DocumentInputType::new("Masking Layer", TaggedValue::LayerPath(None), false),
		DocumentInputType::new("Inpaint", TaggedValue::Bool(true), false),
		DocumentInputType::new("Mask Blur", TaggedValue::F64(4.), false),
		DocumentInputType::new("Mask Starting Fill", TaggedValue::ImaginateMaskStartingFill(ImaginateMaskStartingFill::Fill), false),
		DocumentInputType::new("Improve Faces", TaggedValue::Bool(false), false),
		DocumentInputType::new("Tiling", TaggedValue::Bool(false), false),
		// Non user status (is document input the right way to do this?)
		DocumentInputType::new("Cached Data", TaggedValue::RcImage(None), false),
		DocumentInputType::new("Percent Complete", TaggedValue::F64(0.), false),
		DocumentInputType::new("Status", TaggedValue::ImaginateStatus(ImaginateStatus::Idle), false),
	],
	outputs: &[FrontendGraphDataType::Raster],
	properties: node_properties::imaginate_properties,
};

pub fn resolve_document_node_type(name: &str) -> Option<&DocumentNodeType> {
	DOCUMENT_NODE_TYPES.iter().find(|node| node.name == name)
}

pub fn collect_node_types() -> Vec<FrontendNodeType> {
	DOCUMENT_NODE_TYPES
		.iter()
		.filter(|node_type| !matches!(node_type.name, "Input" | "Output"))
		.map(|node_type| FrontendNodeType::new(node_type.name, node_type.category))
		.collect()
}
