use crate::messages::layout::utility_types::layout_widget::{LayoutGroup, Widget, WidgetCallback, WidgetHolder};
use crate::messages::layout::utility_types::widgets::input_widgets::{NumberInput, NumberInputMode};
use crate::messages::layout::utility_types::widgets::label_widgets::{Separator, SeparatorDirection, SeparatorType, TextLabel};
use crate::messages::prelude::NodeGraphMessage;

use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeId, NodeInput};

pub fn generate_node_properties(document_node: &DocumentNode, node_id: NodeId) -> LayoutGroup {
	let name = document_node.name.clone();
	let layout = match &document_node.implementation {
		DocumentNodeImplementation::Network(_) => match document_node.name.as_str() {
			"Hue Shift Image" => vec![LayoutGroup::Row {
				widgets: vec![
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
			}],
			"Brighten Image" => vec![LayoutGroup::Row {
				widgets: vec![
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
			}],
			_ => vec![LayoutGroup::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: format!("Cannot currently display parameters for network {}", document_node.name),
					..Default::default()
				}))],
			}],
		},
		DocumentNodeImplementation::Unresolved(identifier) => match identifier.name.as_ref() {
			"graphene_std::raster::MapImageNode" | "graphene_core::ops::IdNode" => vec![LayoutGroup::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: format!("{} exposes no parameters", document_node.name),
					..Default::default()
				}))],
			}],
			unknown => {
				vec![
					LayoutGroup::Row {
						widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: format!("TODO: {} parameters", unknown),
							..Default::default()
						}))],
					},
					LayoutGroup::Row {
						widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: "Add in editor/src/messages/portfolio/document/node_graph/node_graph_message_handler.rs".to_string(),
							..Default::default()
						}))],
					},
				]
			}
		},
	};
	LayoutGroup::Section { name, layout }
}
