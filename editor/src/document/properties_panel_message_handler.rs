use std::f64::consts::PI;

use crate::document::properties_panel_message::TransformOp;
use crate::layout::layout_message::LayoutTarget;
use crate::layout::widgets::{
	IconLabel, LayoutRow, NumberInput, PopoverButton, PropertyHolder, Separator, SeparatorDirection, SeparatorType, TextInput, TextLabel, Widget, WidgetCallback, WidgetHolder, WidgetLayout,
};
use crate::message_prelude::*;

use graphene::document::Document as GrapheneDocument;
use graphene::layers::layer_info::{Layer, LayerDataType};
use graphene::{LayerId, Operation};

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};

trait DAffine2Utils {
	fn width(&self) -> f64;
	fn update_width(self, new_width: f64) -> Self;
	fn height(&self) -> f64;
	fn update_height(self, new_height: f64) -> Self;
	fn x(&self) -> f64;
	fn update_x(self, new_x: f64) -> Self;
	fn y(&self) -> f64;
	fn update_y(self, new_y: f64) -> Self;
	fn rotation(&self) -> f64;
	fn update_rotation(self, new_rotation: f64) -> Self;
}

impl DAffine2Utils for DAffine2 {
	fn width(&self) -> f64 {
		self.transform_vector2((1., 0.).into()).length()
	}
	fn update_width(self, new_width: f64) -> Self {
		self * DAffine2::from_scale((new_width / self.width(), 1.).into())
	}
	fn height(&self) -> f64 {
		self.transform_vector2((0., 1.).into()).length()
	}
	fn update_height(self, new_height: f64) -> Self {
		self * DAffine2::from_scale((1., new_height / self.height()).into())
	}
	fn x(&self) -> f64 {
		self.translation.x
	}
	fn update_x(mut self, new_x: f64) -> Self {
		self.translation.x = new_x;
		self
	}
	fn y(&self) -> f64 {
		self.translation.y
	}
	fn update_y(mut self, new_y: f64) -> Self {
		self.translation.y = new_y;
		self
	}
	fn rotation(&self) -> f64 {
		let cos = self.matrix2.col(0).x / self.width();
		let sin = self.matrix2.col(0).y / self.width();
		sin.atan2(cos)
	}
	fn update_rotation(self, new_rotation: f64) -> Self {
		let width = self.width();
		let height = self.height();
		let half_width = width / 2.;
		let half_height = height / 2.;

		let angle_translation_offset = |angle: f64| DVec2::new(-half_width * angle.cos() + half_height * angle.sin(), -half_width * angle.sin() - half_height * angle.cos());
		let angle_adj = angle_translation_offset(new_rotation) - angle_translation_offset(self.rotation());

		DAffine2::from_scale_angle_translation((width, height).into(), new_rotation, self.translation + angle_adj)
	}
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PropertiesPanelMessageHandler {
	active_path: Option<Vec<LayerId>>,
}

impl PropertiesPanelMessageHandler {
	fn matches_selected(&self, path: &[LayerId]) -> bool {
		let last_active_path = self.active_path.as_ref().map(|v| v.last().copied()).flatten();
		let last_modified = path.last().copied();
		matches!((last_active_path, last_modified), (Some(active_last), Some(modified_last)) if active_last == modified_last)
	}
}

impl MessageHandler<PropertiesPanelMessage, &GrapheneDocument> for PropertiesPanelMessageHandler {
	#[remain::check]
	fn process_action(&mut self, message: PropertiesPanelMessage, data: &GrapheneDocument, responses: &mut VecDeque<Message>) {
		let graphine_document = data;
		use PropertiesPanelMessage::*;
		match message {
			SetActiveLayers { paths } => {
				if paths.len() > 1 {
					// TODO: Allow for multiple selected layers
					responses.push_back(PropertiesPanelMessage::ClearSelection.into())
				} else {
					let path = paths.into_iter().next().unwrap();
					let layer = graphine_document.layer(&path).unwrap();
					layer.register_properties(responses, LayoutTarget::PropertiesPanel);
					self.active_path = Some(path)
				}
			}
			ClearSelection => responses.push_back(
				LayoutMessage::SendLayout {
					layout: WidgetLayout::new(vec![]),
					layout_target: LayoutTarget::PropertiesPanel,
				}
				.into(),
			),
			ModifyTransform { value, transform_op } => {
				let path = self.active_path.as_ref().expect("Received update for properties panel with no active layer");
				let layer = graphine_document.layer(path).unwrap();

				use TransformOp::*;
				let action = match transform_op {
					X => DAffine2::update_x,
					Y => DAffine2::update_y,
					Width => DAffine2::update_width,
					Height => DAffine2::update_height,
					Rotation => DAffine2::update_rotation,
				};

				responses.push_back(
					Operation::SetLayerTransform {
						path: path.clone(),
						transform: action(layer.transform, value).to_cols_array(),
					}
					.into(),
				);
			}
			ModifyName { name } => {
				let path = self.active_path.clone().expect("Received update for properties panel with no active layer");
				responses.push_back(DocumentMessage::SetLayerName { layer_path: path, name }.into())
			}
			CheckSelectedWasUpdated { path } => {
				if self.matches_selected(&path) {
					let layer = graphine_document.layer(&path).unwrap();
					layer.register_properties(responses, LayoutTarget::PropertiesPanel);
				}
			}
			CheckSelectedWasDeleted { path } => {
				if self.matches_selected(&path) {
					self.active_path = None;
					responses.push_back(
						LayoutMessage::SendLayout {
							layout_target: LayoutTarget::PropertiesPanel,
							layout: WidgetLayout::default(),
						}
						.into(),
					)
				}
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(PropertiesMessageDiscriminant;)
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
			value: self.name.clone().unwrap_or_else(|| "Untitled".to_string()),
			on_update: WidgetCallback::new(|text_input| PropertiesPanelMessage::ModifyName { name: text_input.value.clone() }.into()),
		})));
		options_bar.push(WidgetHolder::new(Widget::PopoverButton(PopoverButton {
			title: "Options Bar".into(),
			text: "The contents of this popover menu are coming soon".into(),
		})));

		let mut properties_body = match &self.data {
			LayerDataType::Folder(_) => {
				vec![]
			}
			LayerDataType::Shape(_) => {
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
								WidgetHolder::new(Widget::Separator(Separator {
									separator_type: SeparatorType::Related,
									direction: SeparatorDirection::Horizontal,
								})),
								WidgetHolder::new(Widget::NumberInput(NumberInput {
									value: self.transform.x(),
									label: "X".into(),
									unit: " px".into(),
									on_update: WidgetCallback::new(|number_input| {
										PropertiesPanelMessage::ModifyTransform {
											value: number_input.value,
											transform_op: TransformOp::X,
										}
										.into()
									}),
									..NumberInput::default()
								})),
								WidgetHolder::new(Widget::Separator(Separator {
									separator_type: SeparatorType::Related,
									direction: SeparatorDirection::Horizontal,
								})),
								WidgetHolder::new(Widget::NumberInput(NumberInput {
									value: self.transform.y(),
									label: "Y".into(),
									unit: " px".into(),
									on_update: WidgetCallback::new(|number_input| {
										PropertiesPanelMessage::ModifyTransform {
											value: number_input.value,
											transform_op: TransformOp::Y,
										}
										.into()
									}),
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
								WidgetHolder::new(Widget::Separator(Separator {
									separator_type: SeparatorType::Related,
									direction: SeparatorDirection::Horizontal,
								})),
								WidgetHolder::new(Widget::NumberInput(NumberInput {
									value: self.transform.width(),
									label: "W".into(),
									unit: " px".into(),
									on_update: WidgetCallback::new(|number_input| {
										PropertiesPanelMessage::ModifyTransform {
											value: number_input.value,
											transform_op: TransformOp::Width,
										}
										.into()
									}),
									..NumberInput::default()
								})),
								WidgetHolder::new(Widget::Separator(Separator {
									separator_type: SeparatorType::Related,
									direction: SeparatorDirection::Horizontal,
								})),
								WidgetHolder::new(Widget::NumberInput(NumberInput {
									value: self.transform.height(),
									label: "H".into(),
									unit: " px".into(),
									on_update: WidgetCallback::new(|number_input| {
										PropertiesPanelMessage::ModifyTransform {
											value: number_input.value,
											transform_op: TransformOp::Height,
										}
										.into()
									}),
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
								WidgetHolder::new(Widget::Separator(Separator {
									separator_type: SeparatorType::Related,
									direction: SeparatorDirection::Horizontal,
								})),
								WidgetHolder::new(Widget::NumberInput(NumberInput {
									value: self.transform.rotation() * 180. / PI,
									label: "R".into(),
									unit: "°".into(),
									on_update: WidgetCallback::new(|number_input| {
										PropertiesPanelMessage::ModifyTransform {
											value: number_input.value / 180. * PI,
											transform_op: TransformOp::Rotation,
										}
										.into()
									}),
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
