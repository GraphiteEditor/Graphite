use super::layer_panel::LayerDataTypeDiscriminant;
use crate::document::properties_panel_message::TransformOp;
use crate::layout::layout_message::LayoutTarget;
use crate::layout::widgets::{
	ColorInput, IconLabel, LayoutRow, NumberInput, PopoverButton, Separator, SeparatorDirection, SeparatorType, TextInput, TextLabel, Widget, WidgetCallback, WidgetHolder, WidgetLayout,
};
use crate::message_prelude::*;

use graphene::color::Color;
use graphene::document::Document as GrapheneDocument;
use graphene::layers::layer_info::{Layer, LayerDataType};
use graphene::layers::style::{Fill, Stroke};
use graphene::{LayerId, Operation};

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;
use std::rc::Rc;

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
		let angle_translation_adjustment = angle_translation_offset(new_rotation) - angle_translation_offset(self.rotation());

		DAffine2::from_scale_angle_translation((width, height).into(), new_rotation, self.translation + angle_translation_adjustment)
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
		let graphene_document = data;
		use PropertiesPanelMessage::*;
		match message {
			SetActiveLayers { paths } => {
				if paths.len() > 1 {
					// TODO: Allow for multiple selected layers
					responses.push_back(PropertiesPanelMessage::ClearSelection.into())
				} else {
					let path = paths.into_iter().next().unwrap();
					let layer = graphene_document.layer(&path).unwrap();
					register_layer_properties(layer, responses);
					self.active_path = Some(path)
				}
			}
			ClearSelection => {
				responses.push_back(
					LayoutMessage::SendLayout {
						layout: WidgetLayout::new(vec![]),
						layout_target: LayoutTarget::PropertiesOptionsPanel,
					}
					.into(),
				);
				responses.push_back(
					LayoutMessage::SendLayout {
						layout: WidgetLayout::new(vec![]),
						layout_target: LayoutTarget::PropertiesSectionsPanel,
					}
					.into(),
				);
			}
			ModifyTransform { value, transform_op } => {
				let path = self.active_path.as_ref().expect("Received update for properties panel with no active layer");
				let layer = graphene_document.layer(path).unwrap();

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
			ModifyFill { fill } => {
				let path = self.active_path.clone().expect("Received update for properties panel with no active layer");
				responses.push_back(Operation::SetLayerFill { path, fill }.into());
			}
			ModifyStroke { color, weight } => {
				let path = self.active_path.clone().expect("Received update for properties panel with no active layer");
				let layer = graphene_document.layer(&path).unwrap();
				if let Some(color) = Color::from_rgba_str(&color).or_else(|| Color::from_rgb_str(&color)) {
					let stroke = Stroke::new(color, weight as f32);
					responses.push_back(Operation::SetLayerStroke { path, stroke }.into())
				} else {
					// Failed to update, Show user unchanged state
					register_layer_properties(layer, responses)
				}
			}
			CheckSelectedWasUpdated { path } => {
				if self.matches_selected(&path) {
					let layer = graphene_document.layer(&path).unwrap();
					register_layer_properties(layer, responses);
				}
			}
			CheckSelectedWasDeleted { path } => {
				if self.matches_selected(&path) {
					self.active_path = None;
					responses.push_back(
						LayoutMessage::SendLayout {
							layout_target: LayoutTarget::PropertiesOptionsPanel,
							layout: WidgetLayout::default(),
						}
						.into(),
					);
					responses.push_back(
						LayoutMessage::SendLayout {
							layout_target: LayoutTarget::PropertiesSectionsPanel,
							layout: WidgetLayout::default(),
						}
						.into(),
					);
				}
			}
			ResendActiveProperties => {
				let path = self.active_path.clone().expect("Received update for properties panel with no active layer");
				let layer = graphene_document.layer(&path).unwrap();
				register_layer_properties(layer, responses)
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(PropertiesMessageDiscriminant;)
	}
}

fn register_layer_properties(layer: &Layer, responses: &mut VecDeque<Message>) {
	let options_bar = vec![LayoutRow::Row {
		name: "".into(),
		widgets: vec![
			match &layer.data {
				LayerDataType::Folder(_) => WidgetHolder::new(Widget::IconLabel(IconLabel {
					icon: "NodeFolder".into(),
					gap_after: true,
				})),
				LayerDataType::Shape(_) => WidgetHolder::new(Widget::IconLabel(IconLabel {
					icon: "NodeShape".into(),
					gap_after: true,
				})),
				LayerDataType::Text(_) => WidgetHolder::new(Widget::IconLabel(IconLabel {
					icon: "NodeText".into(),
					gap_after: true,
				})),
				LayerDataType::Bitmap(_) => WidgetHolder::new(Widget::IconLabel(IconLabel {
					icon: "NodeImage".into(),
					gap_after: true,
				})),
			},
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Related,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::TextLabel(TextLabel {
				value: LayerDataTypeDiscriminant::from(&layer.data).to_string(),
				..TextLabel::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Unrelated,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::TextInput(TextInput {
				value: layer.name.clone().unwrap_or_else(|| "Untitled".to_string()),
				on_update: WidgetCallback::new(|text_input: &TextInput| PropertiesPanelMessage::ModifyName { name: text_input.value.clone() }.into()),
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Related,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::PopoverButton(PopoverButton {
				title: "Options Bar".into(),
				text: "The contents of this popover menu are coming soon".into(),
			})),
		],
	}];

	let properties_body = match &layer.data {
		LayerDataType::Shape(shape) => {
			if let Some(fill_layout) = node_section_fill(shape.style.fill()) {
				vec![node_section_transform(layer), fill_layout, node_section_stroke(&shape.style.stroke().unwrap_or_default())]
			} else {
				vec![node_section_transform(layer), node_section_stroke(&shape.style.stroke().unwrap_or_default())]
			}
		}
		LayerDataType::Text(text) => {
			vec![
				node_section_transform(layer),
				node_section_fill(text.style.fill()).expect("Text should have fill"),
				node_section_stroke(&text.style.stroke().unwrap_or_default()),
			]
		}
		_ => {
			vec![]
		}
	};

	responses.push_back(
		LayoutMessage::SendLayout {
			layout: WidgetLayout::new(options_bar),
			layout_target: LayoutTarget::PropertiesOptionsPanel,
		}
		.into(),
	);
	responses.push_back(
		LayoutMessage::SendLayout {
			layout: WidgetLayout::new(properties_body),
			layout_target: LayoutTarget::PropertiesSectionsPanel,
		}
		.into(),
	);
}

fn node_section_transform(layer: &Layer) -> LayoutRow {
	LayoutRow::Section {
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
						separator_type: SeparatorType::Unrelated,
						direction: SeparatorDirection::Horizontal,
					})),
					WidgetHolder::new(Widget::NumberInput(NumberInput {
						value: layer.transform.x(),
						label: "X".into(),
						unit: " px".into(),
						on_update: WidgetCallback::new(|number_input: &NumberInput| {
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
						value: layer.transform.y(),
						label: "Y".into(),
						unit: " px".into(),
						on_update: WidgetCallback::new(|number_input: &NumberInput| {
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
						separator_type: SeparatorType::Unrelated,
						direction: SeparatorDirection::Horizontal,
					})),
					WidgetHolder::new(Widget::NumberInput(NumberInput {
						value: layer.transform.width(),
						label: "W".into(),
						unit: " px".into(),
						on_update: WidgetCallback::new(|number_input: &NumberInput| {
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
						value: layer.transform.height(),
						label: "H".into(),
						unit: " px".into(),
						on_update: WidgetCallback::new(|number_input: &NumberInput| {
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
						value: "Rotation".into(),
						..TextLabel::default()
					})),
					WidgetHolder::new(Widget::Separator(Separator {
						separator_type: SeparatorType::Unrelated,
						direction: SeparatorDirection::Horizontal,
					})),
					WidgetHolder::new(Widget::NumberInput(NumberInput {
						value: layer.transform.rotation() * 180. / PI,
						label: "R".into(),
						unit: "°".into(),
						on_update: WidgetCallback::new(|number_input: &NumberInput| {
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
	}
}

fn node_section_fill(fill: &Fill) -> Option<LayoutRow> {
	match fill {
		Fill::Solid(color) => Some(LayoutRow::Section {
			name: "Fill".into(),
			layout: vec![LayoutRow::Row {
				name: "".into(),
				widgets: vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Color".into(),
						..TextLabel::default()
					})),
					WidgetHolder::new(Widget::Separator(Separator {
						separator_type: SeparatorType::Unrelated,
						direction: SeparatorDirection::Horizontal,
					})),
					WidgetHolder::new(Widget::ColorInput(ColorInput {
						value: color.rgba_hex(),
						on_update: WidgetCallback::new(|text_input: &ColorInput| {
							if let Some(color) = Color::from_rgba_str(&text_input.value).or_else(|| Color::from_rgb_str(&text_input.value)) {
								let new_fill = Fill::Solid(color);
								PropertiesPanelMessage::ModifyFill { fill: new_fill }.into()
							} else {
								PropertiesPanelMessage::ResendActiveProperties.into()
							}
						}),
					})),
				],
			}],
		}),
		Fill::LinearGradient(gradient) => {
			let gradient_1 = Rc::new(gradient.clone());
			let gradient_2 = gradient_1.clone();
			Some(LayoutRow::Section {
				name: "Fill".into(),
				layout: vec![
					LayoutRow::Row {
						name: "".into(),
						widgets: vec![
							WidgetHolder::new(Widget::TextLabel(TextLabel {
								value: "Gradient: 0%".into(),
								..TextLabel::default()
							})),
							WidgetHolder::new(Widget::Separator(Separator {
								separator_type: SeparatorType::Unrelated,
								direction: SeparatorDirection::Horizontal,
							})),
							WidgetHolder::new(Widget::ColorInput(ColorInput {
								value: gradient_1.positions[0].1.rgba_hex(),
								on_update: WidgetCallback::new(move |text_input: &ColorInput| {
									if let Some(color) = Color::from_rgba_str(&text_input.value).or_else(|| Color::from_rgb_str(&text_input.value)) {
										let mut new_gradient = (*gradient_1).clone();
										new_gradient.positions[0].1 = color;
										PropertiesPanelMessage::ModifyFill {
											fill: Fill::LinearGradient(new_gradient),
										}
										.into()
									} else {
										PropertiesPanelMessage::ResendActiveProperties.into()
									}
								}),
							})),
						],
					},
					LayoutRow::Row {
						name: "".into(),
						widgets: vec![
							WidgetHolder::new(Widget::TextLabel(TextLabel {
								value: "Gradient: 100%".into(),
								..TextLabel::default()
							})),
							WidgetHolder::new(Widget::Separator(Separator {
								separator_type: SeparatorType::Unrelated,
								direction: SeparatorDirection::Horizontal,
							})),
							WidgetHolder::new(Widget::ColorInput(ColorInput {
								value: gradient_2.positions[1].1.rgba_hex(),
								on_update: WidgetCallback::new(move |text_input: &ColorInput| {
									if let Some(color) = Color::from_rgba_str(&text_input.value).or_else(|| Color::from_rgb_str(&text_input.value)) {
										let mut new_gradient = (*gradient_2).clone();
										new_gradient.positions[1].1 = color;
										PropertiesPanelMessage::ModifyFill {
											fill: Fill::LinearGradient(new_gradient),
										}
										.into()
									} else {
										PropertiesPanelMessage::ResendActiveProperties.into()
									}
								}),
							})),
						],
					},
				],
			})
		}
		Fill::None => None,
	}
}

fn node_section_stroke(stroke: &Stroke) -> LayoutRow {
	let color = stroke.color();
	let weight = stroke.width();
	LayoutRow::Section {
		name: "Stroke".into(),
		layout: vec![
			LayoutRow::Row {
				name: "".into(),
				widgets: vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Color".into(),
						..TextLabel::default()
					})),
					WidgetHolder::new(Widget::Separator(Separator {
						separator_type: SeparatorType::Unrelated,
						direction: SeparatorDirection::Horizontal,
					})),
					WidgetHolder::new(Widget::ColorInput(ColorInput {
						value: stroke.color().rgba_hex(),
						on_update: WidgetCallback::new(move |text_input: &ColorInput| {
							PropertiesPanelMessage::ModifyStroke {
								color: text_input.value.clone(),
								weight: weight as f64,
							}
							.into()
						}),
					})),
				],
			},
			LayoutRow::Row {
				name: "".into(),
				widgets: vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Weight".into(),
						..TextLabel::default()
					})),
					WidgetHolder::new(Widget::Separator(Separator {
						separator_type: SeparatorType::Unrelated,
						direction: SeparatorDirection::Horizontal,
					})),
					WidgetHolder::new(Widget::NumberInput(NumberInput {
						value: stroke.width() as f64,
						is_integer: true,
						min: Some(0.),
						unit: " px".into(),
						on_update: WidgetCallback::new(move |number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyStroke {
								color: color.rgba_hex(),
								weight: number_input.value,
							}
							.into()
						}),
						..NumberInput::default()
					})),
				],
			},
		],
	}
}
