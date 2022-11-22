use crate::messages::layout::utility_types::layout_widget::{LayoutGroup, Widget, WidgetCallback, WidgetHolder};
use crate::messages::layout::utility_types::widgets::button_widgets::ParameterExposeButton;
use crate::messages::layout::utility_types::widgets::input_widgets::{NumberInput, NumberInputMode};
use crate::messages::layout::utility_types::widgets::label_widgets::{Separator, SeparatorDirection, SeparatorType, TextLabel};
use crate::messages::prelude::NodeGraphMessage;

use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, NodeId, NodeInput};

use super::FrontendGraphDataType;

pub fn hue_shift_image_properties(document_node: &DocumentNode, node_id: NodeId) -> Vec<LayoutGroup> {
	vec![LayoutGroup::Row {
		widgets: vec![
			WidgetHolder::new(Widget::ParameterExposeButton(ParameterExposeButton {
				exposed: true,
				data_type: FrontendGraphDataType::Number,
				tooltip: "Expose input parameter in node graph".into(),
				..Default::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Unrelated,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::TextLabel(TextLabel {
				value: "Shift Degrees".into(),
				..Default::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Unrelated,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::NumberInput(NumberInput {
				value: Some({
					let NodeInput::Value {tagged_value: TaggedValue::F32(x), ..} = document_node.inputs[1] else {
						panic!("Hue rotate should be f32")
					};
					x as f64
				}),
				unit: "°".into(),
				mode: NumberInputMode::Range,
				range_min: Some(-180.),
				range_max: Some(180.),
				on_update: WidgetCallback::new(move |number_input: &NumberInput| {
					NodeGraphMessage::SetInputValue {
						node: node_id,
						input_index: 1,
						value: TaggedValue::F32(number_input.value.unwrap() as f32),
					}
					.into()
				}),
				..NumberInput::default()
			})),
		],
	}]
}

pub fn brighten_image_properties(document_node: &DocumentNode, node_id: NodeId) -> Vec<LayoutGroup> {
	vec![LayoutGroup::Row {
		widgets: vec![
			WidgetHolder::new(Widget::ParameterExposeButton(ParameterExposeButton {
				exposed: true,
				data_type: FrontendGraphDataType::Number,
				tooltip: "Expose input parameter in node graph".into(),
				..Default::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Unrelated,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::TextLabel(TextLabel {
				value: "Brighten Amount".into(),
				..Default::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Unrelated,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::NumberInput(NumberInput {
				value: Some({
					let NodeInput::Value {tagged_value: TaggedValue::F32(x), ..} = document_node.inputs[1] else {
						panic!("Brighten amount should be f32")
					};
					x as f64
				}),
				mode: NumberInputMode::Range,
				range_min: Some(-255.),
				range_max: Some(255.),
				on_update: WidgetCallback::new(move |number_input: &NumberInput| {
					NodeGraphMessage::SetInputValue {
						node: node_id,
						input_index: 1,
						value: TaggedValue::F32(number_input.value.unwrap() as f32),
					}
					.into()
				}),
				..NumberInput::default()
			})),
		],
	}]
}

pub fn add_properties(document_node: &DocumentNode, node_id: NodeId) -> Vec<LayoutGroup> {
	let operand = |name: &str, index| LayoutGroup::Row {
		widgets: vec![
			WidgetHolder::new(Widget::ParameterExposeButton(ParameterExposeButton {
				exposed: true,
				data_type: FrontendGraphDataType::Number,
				tooltip: "Expose input parameter in node graph".into(),
				..Default::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Unrelated,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::TextLabel(TextLabel {
				value: name.into(),
				..Default::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Unrelated,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::NumberInput(NumberInput {
				value: Some({
					let NodeInput::Value {tagged_value: TaggedValue::F32(x), ..} = document_node.inputs[index] else {
						panic!("Add input should be f32")
					};

					x as f64
				}),
				mode: NumberInputMode::Increment,
				on_update: WidgetCallback::new(move |number_input: &NumberInput| {
					NodeGraphMessage::SetInputValue {
						node: node_id,
						input_index: index,
						value: TaggedValue::F32(number_input.value.unwrap() as f32),
					}
					.into()
				}),
				..NumberInput::default()
			})),
		],
	};
	vec![operand("Left", 0), operand("Right", 1)]
}

fn unknown_node_properties(document_node: &DocumentNode) -> Vec<LayoutGroup> {
	vec![LayoutGroup::Row {
		widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
			value: format!("Node '{}' cannot be found in library", document_node.name),
			..Default::default()
		}))],
	}]
}

pub fn generate_node_properties(document_node: &DocumentNode, node_id: NodeId) -> LayoutGroup {
	let name = document_node.name.clone();
	let layout = match super::document_node_types::resolve_document_node_type(&name) {
		Some(document_node_type) => (document_node_type.properties)(document_node, node_id),
		None => unknown_node_properties(document_node),
	};
	LayoutGroup::Section { name, layout }
}
