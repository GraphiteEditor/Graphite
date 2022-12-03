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

fn number_range_widget(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, range_min: Option<f64>, range_max: Option<f64>, unit: String, is_integer: bool) -> Vec<WidgetHolder> {
	let input: &NodeInput = document_node.inputs.get(index).unwrap();

	let mut widgets = vec![
		expose_widget(node_id, index, FrontendGraphDataType::Number, input.is_exposed()),
		WidgetHolder::unrelated_seperator(),
		WidgetHolder::text_widget(name),
	];

	if let NodeInput::Value {
		tagged_value: TaggedValue::F64(x),
		exposed: false,
	} = document_node.inputs[index]
	{
		widgets.extend_from_slice(&[
			WidgetHolder::unrelated_seperator(),
			WidgetHolder::new(Widget::NumberInput(NumberInput {
				value: Some(x),
				mode: if range_max.is_some() { NumberInputMode::Range } else { NumberInputMode::Increment },
				range_min,
				range_max,
				unit,
				is_integer,
				on_update: update_value(|x: &NumberInput| TaggedValue::F64(x.value.unwrap()), node_id, index),
				..NumberInput::default()
			})),
		])
	}
	widgets
}

pub fn adjust_hsl_properties(document_node: &DocumentNode, node_id: NodeId) -> Vec<LayoutGroup> {
	let hue_shift = number_range_widget(document_node, node_id, 1, "Hue Shift", Some(-180.), Some(180.), "°".into(), false);
	let saturation_shift = number_range_widget(document_node, node_id, 2, "Saturation Shift", Some(-100.), Some(100.), "%".into(), false);
	let luminance_shift = number_range_widget(document_node, node_id, 3, "Luminance Shift", Some(-100.), Some(100.), "%".into(), false);

	vec![
		LayoutGroup::Row { widgets: hue_shift },
		LayoutGroup::Row { widgets: saturation_shift },
		LayoutGroup::Row { widgets: luminance_shift },
	]
}

pub fn brighten_image_properties(document_node: &DocumentNode, node_id: NodeId) -> Vec<LayoutGroup> {
	let brightness = number_range_widget(document_node, node_id, 1, "Brightness", Some(-255.), Some(255.), "".into(), false);
	let contrast = number_range_widget(document_node, node_id, 2, "Contrast", Some(-255.), Some(255.), "".into(), false);

	vec![LayoutGroup::Row { widgets: brightness }, LayoutGroup::Row { widgets: contrast }]
}

pub fn adjust_gamma_properties(document_node: &DocumentNode, node_id: NodeId) -> Vec<LayoutGroup> {
	let gamma = number_range_widget(document_node, node_id, 1, "Gamma", Some(0.01), None, "".into(), false);

	vec![LayoutGroup::Row { widgets: gamma }]
}

pub fn multiply_opacity(document_node: &DocumentNode, node_id: NodeId) -> Vec<LayoutGroup> {
	let gamma = number_range_widget(document_node, node_id, 1, "Factor", Some(0.), Some(1.), "".into(), false);

	vec![LayoutGroup::Row { widgets: gamma }]
}

pub fn posterize_properties(document_node: &DocumentNode, node_id: NodeId) -> Vec<LayoutGroup> {
	let value = number_range_widget(document_node, node_id, 1, "Levels", Some(2.), Some(255.), "".into(), true);

	vec![LayoutGroup::Row { widgets: value }]
}

pub fn exposure_properties(document_node: &DocumentNode, node_id: NodeId) -> Vec<LayoutGroup> {
	let value = number_range_widget(document_node, node_id, 1, "Value", Some(-3.), Some(3.), "".into(), false);

	vec![LayoutGroup::Row { widgets: value }]
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
