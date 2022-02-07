use crate::layout::layout_message::LayoutTarget;
use crate::layout::widgets::{IconLabel, LayoutRow, NumberInput, PopoverButton, PropertyHolder, TextInput, TextLabel, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::message_prelude::*;

use graphene::document::Document as GrapheneDocument;
use graphene::layers::layer_info::{Layer, LayerDataType};
use graphene::LayerId;

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PropertiesPanelMessageHandler {
	active_path: Option<Vec<LayerId>>,
}

impl MessageHandler<PropertiesPanelMessage, &mut GrapheneDocument> for PropertiesPanelMessageHandler {
	#[remain::check]
	fn process_action(&mut self, message: PropertiesPanelMessage, data: &mut GrapheneDocument, responses: &mut VecDeque<Message>) {
		let graphine_document = data;
		use PropertiesPanelMessage::*;
		match message {
			SetActiveLayer(path) => {
				let layer = graphine_document.layer(&path).unwrap();
				layer.register_properties(responses, LayoutTarget::PropertiesPanel);
				self.active_path = Some(path)
			}
			ClearSelection => responses.push_back(
				LayoutMessage::SendLayout {
					layout: WidgetLayout::new(vec![]),
					layout_target: LayoutTarget::PropertiesPanel,
				}
				.into(),
			),
			SetActiveX(new_x) => {
				let path = &self.active_path.as_ref().expect("Received update for properties panel with no active layer");
				let mut layer = graphine_document.layer(path).unwrap().clone();
				layer.transform.translation.x = new_x;
				graphine_document.set_layer(path, layer, -1).unwrap();
				responses.push_back(DocumentMessage::RenderDocument.into())
			}
			SetActiveY(new_y) => {
				let path = &self.active_path.as_ref().expect("Received update for properties panel with no active layer");
				let mut layer = graphine_document.layer(path).unwrap().clone();
				layer.transform.translation.y = new_y;
				graphine_document.set_layer(path, layer, -1).unwrap();
				responses.push_back(DocumentMessage::RenderDocument.into())
			}
			_ => todo!(),
		}
	}

	fn actions(&self) -> ActionList {
		actions!(ArtboardMessageDiscriminant;)
	}
}

impl PropertyHolder for Layer {
	fn properties(&self) -> WidgetLayout {
		let mut options_bar = match &self.data {
			LayerDataType::Folder(_) => {
				vec![
					WidgetHolder::new(Widget::IconLabel(IconLabel {
						icon: "NodeTypeFolder".into(),
						gap_after: true,
					})),
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Folder".into(),
						..TextLabel::default()
					})),
				]
			}
			LayerDataType::Shape(_) => {
				vec![
					WidgetHolder::new(Widget::IconLabel(IconLabel {
						icon: "NodeTypePath".into(),
						gap_after: true,
					})),
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Path".into(),
						..TextLabel::default()
					})),
				]
			}
			LayerDataType::Text(_) => {
				vec![
					WidgetHolder::new(Widget::IconLabel(IconLabel {
						icon: "NodeTypePath".into(),
						gap_after: true,
					})),
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Text".into(),
						..TextLabel::default()
					})),
				]
			}
		};

		options_bar.push(WidgetHolder::new(Widget::TextInput(TextInput {
			value: self.name.clone().unwrap_or_default(),
			// TODO: Add update to change name
			on_update: WidgetCallback::default(),
		})));
		options_bar.push(WidgetHolder::new(Widget::PopoverButton(PopoverButton {
			title: "Options Bar".into(),
			text: "The contents of this popover menu are coming soon".into(),
		})));

		let mut properties_body = match &self.data {
			LayerDataType::Folder(_) => {
				vec![]
			}
			LayerDataType::Shape(shape) => {
				vec![LayoutRow::Section {
					name: "Transform".into(),
					layout: vec![
						LayoutRow::Row {
							name: "".into(),
							widgets: vec![
								WidgetHolder::new(Widget::TextLabel(TextLabel {
									value: "Position".into(),
									..TextLabel::default()
								})),
								WidgetHolder::new(Widget::NumberInput(NumberInput {
									value: self.transform.translation.x,
									label: "X".into(),
									unit: " px".into(),
									on_update: WidgetCallback::new(|number_input| PropertiesPanelMessage::SetActiveX(number_input.value).into()),
									..NumberInput::default()
								})),
								WidgetHolder::new(Widget::NumberInput(NumberInput {
									value: self.transform.translation.y,
									label: "Y".into(),
									unit: " px".into(),
									on_update: WidgetCallback::new(|number_input| PropertiesPanelMessage::SetActiveY(number_input.value).into()),
									..NumberInput::default()
								})),
							],
						},
						LayoutRow::Row {
							name: "".into(),
							widgets: vec![
								WidgetHolder::new(Widget::TextLabel(TextLabel {
									value: "Dimensions".into(),
									..TextLabel::default()
								})),
								WidgetHolder::new(Widget::NumberInput(NumberInput {
									value: self.transform.to_cols_array()[0],
									label: "W".into(),
									unit: " px".into(),
									..NumberInput::default()
								})),
								WidgetHolder::new(Widget::NumberInput(NumberInput {
									value: self.transform.to_cols_array()[1],
									label: "H".into(),
									unit: " px".into(),
									..NumberInput::default()
								})),
							],
						},
						LayoutRow::Row {
							name: "".into(),
							widgets: vec![
								WidgetHolder::new(Widget::TextLabel(TextLabel {
									value: "Rotation/Sheer".into(),
									..TextLabel::default()
								})),
								WidgetHolder::new(Widget::NumberInput(NumberInput {
									value: self.transform.to_cols_array()[2],
									label: "R".into(),
									unit: "°".into(),
									..NumberInput::default()
								})),
								WidgetHolder::new(Widget::NumberInput(NumberInput {
									value: self.transform.to_cols_array()[3],
									label: "S".into(),
									unit: "°".into(),
									..NumberInput::default()
								})),
							],
						},
					],
				}]
			}
			LayerDataType::Text(_) => {
				vec![]
			}
		};

		properties_body.insert(
			0,
			LayoutRow::Row {
				name: "".into(),
				widgets: options_bar,
			},
		);

		WidgetLayout::new(properties_body)
	}
}
