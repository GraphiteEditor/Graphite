use super::{FrontendGraphDataType, FrontendNodeType};
use crate::messages::layout::utility_types::layout_widget::{LayoutGroup, Widget, WidgetHolder};
use crate::messages::layout::utility_types::widgets::label_widgets::TextLabel;

use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, NodeId, NodeInput};
use graph_craft::proto::{NodeIdentifier, Type};
use graphene_std::raster::Image;

use std::borrow::Cow;

pub struct DocumentInputType {
	pub name: &'static str,
	pub data_type: FrontendGraphDataType,
	pub default: NodeInput,
}

pub struct DocumentNodeType {
	pub name: &'static str,
	pub identifier: NodeIdentifier,
	pub inputs: &'static [DocumentInputType],
	pub outputs: &'static [FrontendGraphDataType],
	pub properties: fn(&DocumentNode, NodeId) -> Vec<LayoutGroup>,
}

// TODO: Dynamic node library
static DOCUMENT_NODE_TYPES: [DocumentNodeType; 7] = [
	DocumentNodeType {
		name: "Identity",
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
					value: format!("The identity node simply returns the input"),
					..Default::default()
				}))],
			}]
		},
	},
	DocumentNodeType {
		name: "Input",
		identifier: NodeIdentifier::new("graphene_core::ops::IdNode", &[Type::Concrete(Cow::Borrowed("Any<'_>"))]),
		inputs: &[],
		outputs: &[FrontendGraphDataType::Raster],
		properties: |_document_node, _node_id| {
			vec![LayoutGroup::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: format!("The input to the graph is the bitmap under the frame"),
					..Default::default()
				}))],
			}]
		},
	},
	DocumentNodeType {
		name: "Output",
		identifier: NodeIdentifier::new("graphene_core::ops::IdNode", &[Type::Concrete(Cow::Borrowed("Any<'_>"))]),
		inputs: &[DocumentInputType {
			name: "In",
			data_type: FrontendGraphDataType::Raster,
			default: NodeInput::Value {
				tagged_value: TaggedValue::Image(Image::empty()),
				exposed: true,
			},
		}],
		outputs: &[],
		properties: |_document_node, _node_id| {
			vec![LayoutGroup::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: format!("The output to the graph is rendered in the frame"),
					..Default::default()
				}))],
			}]
		},
	},
	DocumentNodeType {
		name: "Grayscale Image",
		identifier: NodeIdentifier::new("graphene_std::raster::GrayscaleImageNode", &[]),
		inputs: &[DocumentInputType {
			name: "Image",
			data_type: FrontendGraphDataType::Raster,
			default: NodeInput::Value {
				tagged_value: TaggedValue::Image(Image::empty()),
				exposed: true,
			},
		}],
		outputs: &[FrontendGraphDataType::Raster],
		properties: |_document_node, _node_id| {
			vec![LayoutGroup::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: format!("The output to the graph is rendered in the frame"),
					..Default::default()
				}))],
			}]
		},
	},
	DocumentNodeType {
		name: "Brighten Image",
		identifier: NodeIdentifier::new("graphene_std::raster::BrightenImageNode", &[Type::Concrete(Cow::Borrowed("&TypeErasedNode"))]),
		inputs: &[
			DocumentInputType {
				name: "Image",
				data_type: FrontendGraphDataType::Raster,
				default: NodeInput::Value {
					tagged_value: TaggedValue::Image(Image::empty()),
					exposed: true,
				},
			},
			DocumentInputType {
				name: "Amount",
				data_type: FrontendGraphDataType::Number,
				default: NodeInput::Value {
					tagged_value: TaggedValue::F32(10.),
					exposed: false,
				},
			},
		],
		outputs: &[FrontendGraphDataType::Raster],
		properties: super::node_properties::brighten_image_properties,
	},
	DocumentNodeType {
		name: "Hue Shift Image",
		identifier: NodeIdentifier::new("graphene_std::raster::HueShiftImage", &[Type::Concrete(Cow::Borrowed("&TypeErasedNode"))]),
		inputs: &[
			DocumentInputType {
				name: "Image",
				data_type: FrontendGraphDataType::Raster,
				default: NodeInput::Value {
					tagged_value: TaggedValue::Image(Image::empty()),
					exposed: true,
				},
			},
			DocumentInputType {
				name: "Amount",
				data_type: FrontendGraphDataType::Number,
				default: NodeInput::Value {
					tagged_value: TaggedValue::F32(10.),
					exposed: false,
				},
			},
		],
		outputs: &[FrontendGraphDataType::Raster],
		properties: super::node_properties::hue_shift_image_properties,
	},
	DocumentNodeType {
		name: "Add",
		identifier: NodeIdentifier::new("graphene_core::ops::AddNode", &[Type::Concrete(Cow::Borrowed("&TypeErasedNode"))]),
		inputs: &[
			DocumentInputType {
				name: "Left",
				data_type: FrontendGraphDataType::Number,
				default: NodeInput::Value {
					tagged_value: TaggedValue::F32(0.),
					exposed: true,
				},
			},
			DocumentInputType {
				name: "Right",
				data_type: FrontendGraphDataType::Number,
				default: NodeInput::Value {
					tagged_value: TaggedValue::F32(0.),
					exposed: true,
				},
			},
		],
		outputs: &[FrontendGraphDataType::Number],
		properties: super::node_properties::add_properties,
	},
];

pub fn resolve_document_node_type(name: &str) -> Option<&DocumentNodeType> {
	DOCUMENT_NODE_TYPES.iter().find(|node| node.name == name)
}

pub fn collect_node_types() -> Vec<FrontendNodeType> {
	DOCUMENT_NODE_TYPES.iter().map(|node_type| FrontendNodeType { name: node_type.name.to_string() }).collect()
}
