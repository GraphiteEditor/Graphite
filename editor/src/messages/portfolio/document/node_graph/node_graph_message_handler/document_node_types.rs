use super::{node_properties, FrontendGraphDataType, FrontendNodeType};
use crate::messages::layout::utility_types::layout_widget::{LayoutGroup, Widget, WidgetHolder};
use crate::messages::layout::utility_types::widgets::label_widgets::TextLabel;

use glam::DVec2;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, NodeId, NodeInput};
use graph_craft::proto::{NodeIdentifier, Type};
use graphene_std::raster::Image;
use graphene_std::vector::subpath::Subpath;

use std::borrow::Cow;

pub struct DocumentInputType {
	pub name: &'static str,
	pub data_type: FrontendGraphDataType,
	pub default: NodeInput,
}

impl DocumentInputType {
	pub const fn none() -> Self {
		Self {
			name: "None",
			data_type: FrontendGraphDataType::General,
			default: NodeInput::value(TaggedValue::None, false),
		}
	}
}

pub struct DocumentNodeType {
	pub name: &'static str,
	pub category: &'static str,
	pub identifier: NodeIdentifier,
	pub inputs: &'static [DocumentInputType],
	pub outputs: &'static [FrontendGraphDataType],
	pub properties: fn(&DocumentNode, NodeId) -> Vec<LayoutGroup>,
}

// TODO: Dynamic node library
static DOCUMENT_NODE_TYPES: &[DocumentNodeType] = &[
	DocumentNodeType {
		name: "Identity",
		category: "Meta",
		identifier: NodeIdentifier::new("graphene_core::ops::IdNode", &[Type::Concrete(Cow::Borrowed("Any<'_>"))]),
		inputs: &[DocumentInputType {
			name: "In",
			data_type: FrontendGraphDataType::General,
			default: NodeInput::Node(0),
		}],
		outputs: &[FrontendGraphDataType::General],
		properties: |_document_node, _node_id| {
			vec![LayoutGroup::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: "The identity node simply returns the input".to_string(),
					..Default::default()
				}))],
			}]
		},
	},
	DocumentNodeType {
		name: "Input",
		category: "Meta",
		identifier: NodeIdentifier::new("graphene_core::ops::IdNode", &[Type::Concrete(Cow::Borrowed("Any<'_>"))]),
		inputs: &[DocumentInputType {
			name: "In",
			data_type: FrontendGraphDataType::Raster,
			default: NodeInput::Network,
		}],
		outputs: &[FrontendGraphDataType::Raster],
		properties: |_document_node, _node_id| node_properties::string_properties("The input to the graph is the bitmap under the frame".to_string()),
	},
	DocumentNodeType {
		name: "Output",
		category: "Meta",
		identifier: NodeIdentifier::new("graphene_core::ops::IdNode", &[Type::Concrete(Cow::Borrowed("Any<'_>"))]),
		inputs: &[DocumentInputType {
			name: "In",
			data_type: FrontendGraphDataType::Raster,
			default: NodeInput::value(TaggedValue::Image(Image::empty()), true),
		}],
		outputs: &[],
		properties: |_document_node, _node_id| node_properties::string_properties("The output to the graph is rendered in the frame".to_string()),
	},
	DocumentNodeType {
		name: "Grayscale Image",
		category: "Image Color Correction",
		identifier: NodeIdentifier::new("graphene_std::raster::GrayscaleImageNode", &[]),
		inputs: &[DocumentInputType {
			name: "Image",
			data_type: FrontendGraphDataType::Raster,
			default: NodeInput::value(TaggedValue::Image(Image::empty()), true),
		}],
		outputs: &[FrontendGraphDataType::Raster],
		properties: node_properties::no_properties,
	},
	DocumentNodeType {
		name: "Invert Image Color",
		category: "Image Color Correction",
		identifier: NodeIdentifier::new("graphene_std::raster::InvertImageColorNode", &[]),
		inputs: &[DocumentInputType {
			name: "Image",
			data_type: FrontendGraphDataType::Raster,
			default: NodeInput::value(TaggedValue::Image(Image::empty()), true),
		}],
		outputs: &[FrontendGraphDataType::Raster],
		properties: node_properties::no_properties,
	},
	DocumentNodeType {
		name: "Shift Image HSL",
		category: "Image Color Correction",
		identifier: NodeIdentifier::new("graphene_std::raster::ShiftImageHSLNode", &[Type::Concrete(Cow::Borrowed("&TypeErasedNode"))]),
		inputs: &[
			DocumentInputType {
				name: "Image",
				data_type: FrontendGraphDataType::Raster,
				default: NodeInput::value(TaggedValue::Image(Image::empty()), true),
			},
			DocumentInputType {
				name: "Hue Shift",
				data_type: FrontendGraphDataType::Number,
				default: NodeInput::value(TaggedValue::F64(0.), false),
			},
			DocumentInputType {
				name: "Saturation Shift",
				data_type: FrontendGraphDataType::Number,
				default: NodeInput::value(TaggedValue::F64(0.), false),
			},
			DocumentInputType {
				name: "Luminance Shift",
				data_type: FrontendGraphDataType::Number,
				default: NodeInput::value(TaggedValue::F64(0.), false),
			},
		],
		outputs: &[FrontendGraphDataType::Raster],
		properties: node_properties::adjust_hsl_properties,
	},
	DocumentNodeType {
		name: "Image Contrast and Brightness",
		category: "Image Color Correction",
		identifier: NodeIdentifier::new("graphene_std::raster::ImageBrightnessAndContrast", &[Type::Concrete(Cow::Borrowed("&TypeErasedNode"))]),
		inputs: &[
			DocumentInputType {
				name: "Image",
				data_type: FrontendGraphDataType::Raster,
				default: NodeInput::value(TaggedValue::Image(Image::empty()), true),
			},
			DocumentInputType {
				name: "Brightness",
				data_type: FrontendGraphDataType::Number,
				default: NodeInput::value(TaggedValue::F64(0.), false),
			},
			DocumentInputType {
				name: "Contrast",
				data_type: FrontendGraphDataType::Number,
				default: NodeInput::value(TaggedValue::F64(0.), false),
			},
		],
		outputs: &[FrontendGraphDataType::Raster],
		properties: node_properties::brighten_image_properties,
	},
	DocumentNodeType {
		name: "Adjust Image Gamma",
		category: "Image Color Correction",
		identifier: NodeIdentifier::new("graphene_std::raster::ImageGammaNode", &[Type::Concrete(Cow::Borrowed("&TypeErasedNode"))]),
		inputs: &[
			DocumentInputType {
				name: "Image",
				data_type: FrontendGraphDataType::Raster,
				default: NodeInput::value(TaggedValue::Image(Image::empty()), true),
			},
			DocumentInputType {
				name: "Gamma",
				data_type: FrontendGraphDataType::Number,
				default: NodeInput::value(TaggedValue::F64(1.), false),
			},
		],
		outputs: &[FrontendGraphDataType::Raster],
		properties: node_properties::adjust_gamma_properties,
	},
	DocumentNodeType {
		name: "Multiply Image Opactiy",
		category: "Image Color Correction",
		identifier: NodeIdentifier::new("graphene_std::raster::ImageOpacityNode", &[Type::Concrete(Cow::Borrowed("&TypeErasedNode"))]),
		inputs: &[
			DocumentInputType {
				name: "Image",
				data_type: FrontendGraphDataType::Raster,
				default: NodeInput::value(TaggedValue::Image(Image::empty()), true),
			},
			DocumentInputType {
				name: "Factor",
				data_type: FrontendGraphDataType::Number,
				default: NodeInput::value(TaggedValue::F64(1.), false),
			},
		],
		outputs: &[FrontendGraphDataType::Raster],
		properties: node_properties::multiply_opacity,
	},
	DocumentNodeType {
		name: "Posterize",
		category: "Image Filters",
		identifier: NodeIdentifier::new("graphene_std::raster::Posterize", &[Type::Concrete(Cow::Borrowed("&TypeErasedNode"))]),
		inputs: &[
			DocumentInputType {
				name: "Image",
				data_type: FrontendGraphDataType::Raster,
				default: NodeInput::value(TaggedValue::Image(Image::empty()), true),
			},
			DocumentInputType {
				name: "Value",
				data_type: FrontendGraphDataType::Number,
				default: NodeInput::value(TaggedValue::F64(5.), false),
			},
		],
		outputs: &[FrontendGraphDataType::Raster],
		properties: node_properties::posterize_properties,
	},
	DocumentNodeType {
		name: "Exposure",
		category: "Image Color Correction",
		identifier: NodeIdentifier::new("graphene_std::raster::ExposureNode", &[Type::Concrete(Cow::Borrowed("&TypeErasedNode"))]),
		inputs: &[
			DocumentInputType {
				name: "Image",
				data_type: FrontendGraphDataType::Raster,
				default: NodeInput::value(TaggedValue::Image(Image::empty()), true),
			},
			DocumentInputType {
				name: "Value",
				data_type: FrontendGraphDataType::Number,
				default: NodeInput::value(TaggedValue::F64(1.), false),
			},
		],
		outputs: &[FrontendGraphDataType::Raster],
		properties: node_properties::exposure_properties,
	},
	DocumentNodeType {
		name: "Add",
		category: "Mathmatics",
		identifier: NodeIdentifier::new("graphene_core::ops::AddNode", &[Type::Concrete(Cow::Borrowed("&TypeErasedNode"))]),
		inputs: &[
			DocumentInputType {
				name: "Input",
				data_type: FrontendGraphDataType::Number,
				default: NodeInput::value(TaggedValue::F64(0.), true),
			},
			DocumentInputType {
				name: "Addend",
				data_type: FrontendGraphDataType::Number,
				default: NodeInput::value(TaggedValue::F64(0.), true),
			},
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
		identifier: NodeIdentifier::new("graphene_core::ops::IdNode", &[Type::Concrete(Cow::Borrowed("Any<'_>"))]),
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
