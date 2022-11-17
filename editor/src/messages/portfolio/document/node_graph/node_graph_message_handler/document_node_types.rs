use std::borrow::Cow;

use graph_craft::document::value::TaggedValue;
use graph_craft::document::NodeInput;
use graph_craft::proto::{NodeIdentifier, Type};
use graphene_std::raster::Image;

use super::FrontendNodeType;

pub struct DocumentNodeType {
	pub name: &'static str,
	pub identifier: NodeIdentifier,
	pub default_inputs: &'static [NodeInput],
}

// TODO: Dynamic node library
static DOCUMENT_NODE_TYPES: [DocumentNodeType; 5] = [
	DocumentNodeType {
		name: "Identity",
		identifier: NodeIdentifier::new("graphene_core::ops::IdNode", &[Type::Concrete(Cow::Borrowed("Any<'_>"))]),
		default_inputs: &[NodeInput::Node(0)],
	},
	DocumentNodeType {
		name: "Grayscale Image",
		identifier: NodeIdentifier::new("graphene_std::raster::GrayscaleImageNode", &[]),
		default_inputs: &[NodeInput::Value {
			tagged_value: TaggedValue::Image(Image {
				width: 0,
				height: 0,
				data: Vec::new(),
			}),
			exposed: true,
		}],
	},
	DocumentNodeType {
		name: "Brighten Image",
		identifier: NodeIdentifier::new("graphene_std::raster::BrightenImageNode", &[Type::Concrete(Cow::Borrowed("&TypeErasedNode"))]),
		default_inputs: &[
			NodeInput::Value {
				tagged_value: TaggedValue::Image(Image {
					width: 0,
					height: 0,
					data: Vec::new(),
				}),
				exposed: true,
			},
			NodeInput::Value {
				tagged_value: TaggedValue::F32(10.),
				exposed: false,
			},
		],
	},
	DocumentNodeType {
		name: "Hue Shift Image",
		identifier: NodeIdentifier::new("graphene_std::raster::HueShiftImage", &[Type::Concrete(Cow::Borrowed("&TypeErasedNode"))]),
		default_inputs: &[
			NodeInput::Value {
				tagged_value: TaggedValue::Image(Image {
					width: 0,
					height: 0,
					data: Vec::new(),
				}),
				exposed: true,
			},
			NodeInput::Value {
				tagged_value: TaggedValue::F32(50.),
				exposed: false,
			},
		],
	},
	DocumentNodeType {
		name: "Add",
		identifier: NodeIdentifier::new("graphene_core::ops::AddNode", &[Type::Concrete(Cow::Borrowed("u32")), Type::Concrete(Cow::Borrowed("u32"))]),
		default_inputs: &[
			NodeInput::Value {
				tagged_value: TaggedValue::U32(0),
				exposed: false,
			},
			NodeInput::Value {
				tagged_value: TaggedValue::U32(0),
				exposed: false,
			},
		],
	},
];

pub fn resolve_document_node_type(name: &str) -> Option<&DocumentNodeType> {
	DOCUMENT_NODE_TYPES.iter().find(|node| node.name == name)
}

pub fn collect_node_types() -> Vec<FrontendNodeType> {
	DOCUMENT_NODE_TYPES.iter().map(|node_type| FrontendNodeType { name: node_type.name.to_string() }).collect()
}
