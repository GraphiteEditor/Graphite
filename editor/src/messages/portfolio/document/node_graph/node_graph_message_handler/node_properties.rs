use crate::messages::layout::utility_types::layout_widget::{LayoutGroup, Widget, WidgetCallback, WidgetHolder};
use crate::messages::layout::utility_types::widgets::button_widgets::ParameterExposeButton;
use crate::messages::layout::utility_types::widgets::input_widgets::{NumberInput, NumberInputMode};
use crate::messages::prelude::NodeGraphMessage;

use glam::DVec2;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, NodeId, NodeInput};

use super::FrontendGraphDataType;

pub fn string_properties(text: impl Into<String>) -> Vec<LayoutGroup> {
	let widget = WidgetHolder::text_widget(text);
	vec![LayoutGroup::Row { widgets: vec![widget] }]
}

fn update_value<T, F: Fn(&T) -> TaggedValue + 'static>(value: F, node_id: NodeId, input_index: usize) -> WidgetCallback<T> {
	WidgetCallback::new(move |number_input: &T| {
		NodeGraphMessage::SetInputValue {
			node: node_id,
			input_index,
			value: value(number_input),
		}
		.into()
	})
}

fn expose_widget(node_id: NodeId, index: usize, data_type: FrontendGraphDataType, exposed: bool) -> WidgetHolder {
	WidgetHolder::new(Widget::ParameterExposeButton(ParameterExposeButton {
		exposed,
		data_type,
		tooltip: "Expose input parameter in node graph".into(),
		on_update: WidgetCallback::new(move |_parameter| {
			NodeGraphMessage::ExposeInput {
				node_id,
				input_index: index,
				new_exposed: !exposed,
			}
			.into()
		}),
		..Default::default()
	}))
}

pub fn hue_shift_image_properties(document_node: &DocumentNode, node_id: NodeId) -> Vec<LayoutGroup> {
	let index = 1;
	let input: &NodeInput = document_node.inputs.get(index).unwrap();

	let mut widgets = vec![
		expose_widget(node_id, index, FrontendGraphDataType::Number, input.is_exposed()),
		WidgetHolder::unrelated_seperator(),
		WidgetHolder::text_widget("Shift Degrees"),
	];
	if let NodeInput::Value {
		tagged_value: TaggedValue::F32(x),
		exposed: false,
	} = document_node.inputs[index]
	{
		widgets.extend_from_slice(&[
			WidgetHolder::unrelated_seperator(),
			WidgetHolder::new(Widget::NumberInput(NumberInput {
				value: Some(x as f64),
				unit: "°".into(),
				mode: NumberInputMode::Range,
				range_min: Some(-180.),
				range_max: Some(180.),
				on_update: update_value(|number_input: &NumberInput| TaggedValue::F32(number_input.value.unwrap() as f32), node_id, 1),
				..NumberInput::default()
			})),
		])
	}

	vec![LayoutGroup::Row { widgets }]
}

pub fn brighten_image_properties(document_node: &DocumentNode, node_id: NodeId) -> Vec<LayoutGroup> {
	let index = 1;
	let input: &NodeInput = document_node.inputs.get(index).unwrap();

	let mut widgets = vec![
		expose_widget(node_id, index, FrontendGraphDataType::Number, input.is_exposed()),
		WidgetHolder::unrelated_seperator(),
		WidgetHolder::text_widget("Brighten Amount"),
	];

	if let NodeInput::Value {
		tagged_value: TaggedValue::F32(x),
		exposed: false,
	} = document_node.inputs[index]
	{
		widgets.extend_from_slice(&[
			WidgetHolder::unrelated_seperator(),
			WidgetHolder::new(Widget::NumberInput(NumberInput {
				value: Some(x as f64),
				mode: NumberInputMode::Range,
				range_min: Some(-255.),
				range_max: Some(255.),
				on_update: update_value(|x: &NumberInput| TaggedValue::F32(x.value.unwrap() as f32), node_id, 1),
				..NumberInput::default()
			})),
		])
	}

	vec![LayoutGroup::Row { widgets }]
}

pub fn add_properties(document_node: &DocumentNode, node_id: NodeId) -> Vec<LayoutGroup> {
	let operand = |name: &str, index| {
		let input: &NodeInput = document_node.inputs.get(index).unwrap();
		let mut widgets = vec![
			expose_widget(node_id, index, FrontendGraphDataType::Number, input.is_exposed()),
			WidgetHolder::unrelated_seperator(),
			WidgetHolder::text_widget(name),
		];

		if let NodeInput::Value {
			tagged_value: TaggedValue::F32(x),
			exposed: false,
		} = document_node.inputs[index]
		{
			widgets.extend_from_slice(&[
				WidgetHolder::unrelated_seperator(),
				WidgetHolder::new(Widget::NumberInput(NumberInput {
					value: Some(x as f64),
					mode: NumberInputMode::Increment,
					on_update: update_value(|number_input: &NumberInput| TaggedValue::F32(number_input.value.unwrap() as f32), node_id, index),
					..NumberInput::default()
				})),
			]);
		}

		LayoutGroup::Row { widgets }
	};
	vec![operand("Input", 0), operand("Addend", 1)]
}

pub fn transform_properties(document_node: &DocumentNode, node_id: NodeId) -> Vec<LayoutGroup> {
	let translation = {
		let index = 1;
		let input: &NodeInput = document_node.inputs.get(index).unwrap();

		let mut widgets = vec![
			expose_widget(node_id, index, FrontendGraphDataType::Vector, input.is_exposed()),
			WidgetHolder::unrelated_seperator(),
			WidgetHolder::text_widget("Translation"),
		];

		if let NodeInput::Value {
			tagged_value: TaggedValue::DVec2(vec2),
			exposed: false,
		} = document_node.inputs[index]
		{
			widgets.extend_from_slice(&[
				WidgetHolder::unrelated_seperator(),
				WidgetHolder::new(Widget::NumberInput(NumberInput {
					value: Some(vec2.x),
					label: "X".into(),
					unit: " px".into(),
					on_update: update_value(move |number_input: &NumberInput| TaggedValue::DVec2(DVec2::new(number_input.value.unwrap(), vec2.y)), node_id, index),
					..NumberInput::default()
				})),
				WidgetHolder::unrelated_seperator(),
				WidgetHolder::new(Widget::NumberInput(NumberInput {
					value: Some(vec2.y),
					label: "Y".into(),
					unit: " px".into(),
					on_update: update_value(move |number_input: &NumberInput| TaggedValue::DVec2(DVec2::new(vec2.x, number_input.value.unwrap())), node_id, index),
					..NumberInput::default()
				})),
			]);
		}

		LayoutGroup::Row { widgets }
	};

	let rotation = {
		let index = 2;
		let input: &NodeInput = document_node.inputs.get(index).unwrap();

		let mut widgets = vec![
			expose_widget(node_id, index, FrontendGraphDataType::Number, input.is_exposed()),
			WidgetHolder::unrelated_seperator(),
			WidgetHolder::text_widget("Rotation"),
		];

		if let NodeInput::Value {
			tagged_value: TaggedValue::F64(val),
			exposed: false,
		} = document_node.inputs[index]
		{
			widgets.extend_from_slice(&[
				WidgetHolder::unrelated_seperator(),
				WidgetHolder::new(Widget::NumberInput(NumberInput {
					value: Some(val.to_degrees()),
					unit: "°".into(),
					mode: NumberInputMode::Range,
					range_min: Some(-180.),
					range_max: Some(180.),
					on_update: update_value(|number_input: &NumberInput| TaggedValue::F64(number_input.value.unwrap().to_radians()), node_id, index),
					..NumberInput::default()
				})),
			]);
		}

		LayoutGroup::Row { widgets }
	};

	let scale = {
		let index = 3;
		let input: &NodeInput = document_node.inputs.get(index).unwrap();

		let mut widgets = vec![
			expose_widget(node_id, index, FrontendGraphDataType::Vector, input.is_exposed()),
			WidgetHolder::unrelated_seperator(),
			WidgetHolder::text_widget("Scale"),
		];

		if let NodeInput::Value {
			tagged_value: TaggedValue::DVec2(vec2),
			exposed: false,
		} = document_node.inputs[index]
		{
			widgets.extend_from_slice(&[
				WidgetHolder::unrelated_seperator(),
				WidgetHolder::new(Widget::NumberInput(NumberInput {
					value: Some(vec2.x),
					label: "X".into(),
					unit: "".into(),
					on_update: update_value(move |number_input: &NumberInput| TaggedValue::DVec2(DVec2::new(number_input.value.unwrap(), vec2.y)), node_id, index),
					..NumberInput::default()
				})),
				WidgetHolder::unrelated_seperator(),
				WidgetHolder::new(Widget::NumberInput(NumberInput {
					value: Some(vec2.y),
					label: "Y".into(),
					unit: "".into(),
					on_update: update_value(move |number_input: &NumberInput| TaggedValue::DVec2(DVec2::new(vec2.x, number_input.value.unwrap())), node_id, index),
					..NumberInput::default()
				})),
			]);
		}

		LayoutGroup::Row { widgets }
	};
	vec![translation, rotation, scale]
}

fn unknown_node_properties(document_node: &DocumentNode) -> Vec<LayoutGroup> {
	string_properties(format!("Node '{}' cannot be found in library", document_node.name))
}

pub fn no_properties(document_node: &DocumentNode, _node_id: NodeId) -> Vec<LayoutGroup> {
	string_properties(format!("The {} node requires no properties.", document_node.name.to_lowercase()))
}

pub fn generate_node_properties(document_node: &DocumentNode, node_id: NodeId) -> LayoutGroup {
	let name = document_node.name.clone();
	let layout = match super::document_node_types::resolve_document_node_type(&name) {
		Some(document_node_type) => (document_node_type.properties)(document_node, node_id),
		None => unknown_node_properties(document_node),
	};
	LayoutGroup::Section { name, layout }
}
