use super::layer_panel::LayerDataTypeDiscriminant;
use crate::document::properties_panel_message::TransformOp;
use crate::layout::layout_message::LayoutTarget;
use crate::layout::widgets::{
	ColorInput, FontInput, IconLabel, LayoutRow, NumberInput, PopoverButton, RadioEntryData, RadioInput, Separator, SeparatorDirection, SeparatorType, TextAreaInput, TextInput, TextLabel, Widget,
	WidgetCallback, WidgetHolder, WidgetLayout,
};
use crate::message_prelude::*;

use graphene::color::Color;
use graphene::document::{Document as GrapheneDocument, FontCache};
use graphene::layers::layer_info::{Layer, LayerDataType};
use graphene::layers::style::{Fill, LineCap, LineJoin, Stroke};
use graphene::layers::text_layer::TextLayer;
use graphene::{LayerId, Operation};

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;
use std::rc::Rc;

trait DAffine2Utils {
	fn scale_x(&self) -> f64;
	fn update_scale_x(self, new_width: f64) -> Self;
	fn scale_y(&self) -> f64;
	fn update_scale_y(self, new_height: f64) -> Self;
	fn x(&self) -> f64;
	fn update_x(self, new_x: f64) -> Self;
	fn y(&self) -> f64;
	fn update_y(self, new_y: f64) -> Self;
	fn rotation(&self) -> f64;
	fn update_rotation(self, new_rotation: f64) -> Self;
}

impl DAffine2Utils for DAffine2 {
	fn scale_x(&self) -> f64 {
		self.transform_vector2((1., 0.).into()).length()
	}

	fn update_scale_x(self, new_width: f64) -> Self {
		self * DAffine2::from_scale((new_width / self.scale_x(), 1.).into())
	}

	fn scale_y(&self) -> f64 {
		self.transform_vector2((0., 1.).into()).length()
	}

	fn update_scale_y(self, new_height: f64) -> Self {
		self * DAffine2::from_scale((1., new_height / self.scale_y()).into())
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
		let cos = self.matrix2.col(0).x / self.scale_x();
		let sin = self.matrix2.col(0).y / self.scale_x();
		sin.atan2(cos)
	}

	fn update_rotation(self, new_rotation: f64) -> Self {
		let width = self.scale_x();
		let height = self.scale_y();
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
		let last_active_path = self.active_path.as_ref().and_then(|v| v.last().copied());
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
					register_layer_properties(layer, responses, &graphene_document.font_cache);
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
			ModifyFont {
				font_family,
				font_style,
				font_file,
				size,
			} => {
				let path = self.active_path.clone().expect("Received update for properties panel with no active layer");

				let layer = graphene_document.layer(&path).unwrap();
				register_layer_properties(layer, responses, &graphene_document.font_cache);

				responses.push_back(
					Operation::ModifyFont {
						path,
						font_family,
						font_style,
						font_file,
						size,
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
					ScaleX | Width => DAffine2::update_scale_x,
					ScaleY | Height => DAffine2::update_scale_y,
					Rotation => DAffine2::update_rotation,
				};

				let scale = match transform_op {
					Width => layer.bounding_transform(&graphene_document.font_cache).scale_x() / layer.transform.scale_x(),
					Height => layer.bounding_transform(&graphene_document.font_cache).scale_y() / layer.transform.scale_y(),
					_ => 1.,
				};

				responses.push_back(
					Operation::SetLayerTransform {
						path: path.clone(),
						transform: action(layer.transform, value / scale).to_cols_array(),
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
			ModifyStroke { stroke } => {
				let path = self.active_path.clone().expect("Received update for properties panel with no active layer");
				responses.push_back(Operation::SetLayerStroke { path, stroke }.into())
			}
			ModifyText { new_text } => {
				let path = self.active_path.clone().expect("Received update for properties panel with no active layer");
				responses.push_back(Operation::SetTextContent { path, new_text }.into())
			}
			CheckSelectedWasUpdated { path } => {
				if self.matches_selected(&path) {
					let layer = graphene_document.layer(&path).unwrap();
					register_layer_properties(layer, responses, &graphene_document.font_cache);
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
				register_layer_properties(layer, responses, &graphene_document.font_cache)
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(PropertiesMessageDiscriminant;)
	}
}

fn register_layer_properties(layer: &Layer, responses: &mut VecDeque<Message>, font_cache: &FontCache) {
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
				LayerDataType::Image(_) => WidgetHolder::new(Widget::IconLabel(IconLabel {
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
				vec![node_section_transform(layer, font_cache), fill_layout, node_section_stroke(&shape.style.stroke().unwrap_or_default())]
			} else {
				vec![node_section_transform(layer, font_cache), node_section_stroke(&shape.style.stroke().unwrap_or_default())]
			}
		}
		LayerDataType::Text(text) => {
			vec![
				node_section_transform(layer, font_cache),
				node_section_font(text),
				node_section_fill(text.path_style.fill()).expect("Text should have fill"),
				node_section_stroke(&text.path_style.stroke().unwrap_or_default()),
			]
		}
		LayerDataType::Image(_) => {
			vec![node_section_transform(layer, font_cache)]
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

fn node_section_transform(layer: &Layer, font_cache: &FontCache) -> LayoutRow {
	LayoutRow::Section {
		name: "Transform".into(),
		layout: vec![
			LayoutRow::Row {
				name: "".into(),
				widgets: vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Location".into(),
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
						value: "Rotation".into(),
						..TextLabel::default()
					})),
					WidgetHolder::new(Widget::Separator(Separator {
						separator_type: SeparatorType::Unrelated,
						direction: SeparatorDirection::Horizontal,
					})),
					WidgetHolder::new(Widget::NumberInput(NumberInput {
						value: layer.transform.rotation() * 180. / PI,
						label: "".into(),
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
			LayoutRow::Row {
				name: "".into(),
				widgets: vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Scale".into(),
						..TextLabel::default()
					})),
					WidgetHolder::new(Widget::Separator(Separator {
						separator_type: SeparatorType::Unrelated,
						direction: SeparatorDirection::Horizontal,
					})),
					WidgetHolder::new(Widget::NumberInput(NumberInput {
						value: layer.transform.scale_x(),
						label: "X".into(),
						unit: "".into(),
						on_update: WidgetCallback::new(|number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyTransform {
								value: number_input.value,
								transform_op: TransformOp::ScaleX,
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
						value: layer.transform.scale_y(),
						label: "Y".into(),
						unit: "".into(),
						on_update: WidgetCallback::new(|number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyTransform {
								value: number_input.value,
								transform_op: TransformOp::ScaleY,
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
						value: layer.bounding_transform(font_cache).scale_x(),
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
						value: layer.bounding_transform(font_cache).scale_y(),
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
		],
	}
}

fn node_section_font(layer: &TextLayer) -> LayoutRow {
	let font_family = layer.font_family.clone();
	let font_style = layer.font_style.clone();
	let font_file = layer.font_file.clone();
	let size = layer.size;
	LayoutRow::Section {
		name: "Font".into(),
		layout: vec![
			LayoutRow::Row {
				name: "".into(),
				widgets: vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Text".into(),
						..TextLabel::default()
					})),
					WidgetHolder::new(Widget::Separator(Separator {
						separator_type: SeparatorType::Unrelated,
						direction: SeparatorDirection::Horizontal,
					})),
					WidgetHolder::new(Widget::TextAreaInput(TextAreaInput {
						value: layer.text.clone(),
						on_update: WidgetCallback::new(|text_area: &TextAreaInput| PropertiesPanelMessage::ModifyText { new_text: text_area.value.clone() }.into()),
					})),
				],
			},
			LayoutRow::Row {
				name: "".into(),
				widgets: vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Font".into(),
						..TextLabel::default()
					})),
					WidgetHolder::new(Widget::Separator(Separator {
						separator_type: SeparatorType::Unrelated,
						direction: SeparatorDirection::Horizontal,
					})),
					WidgetHolder::new(Widget::FontInput(FontInput {
						is_style_picker: false,
						font_family: layer.font_family.clone(),
						font_style: layer.font_style.clone(),
						font_file: String::new(),
						on_update: WidgetCallback::new(move |font_input: &FontInput| {
							PropertiesPanelMessage::ModifyFont {
								font_family: font_input.font_family.clone(),
								font_style: font_input.font_style.clone(),
								font_file: Some(font_input.font_file.clone()),
								size,
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
						value: "Style".into(),
						..TextLabel::default()
					})),
					WidgetHolder::new(Widget::Separator(Separator {
						separator_type: SeparatorType::Unrelated,
						direction: SeparatorDirection::Horizontal,
					})),
					WidgetHolder::new(Widget::FontInput(FontInput {
						is_style_picker: true,
						font_family: layer.font_family.clone(),
						font_style: layer.font_style.clone(),
						font_file: String::new(),
						on_update: WidgetCallback::new(move |font_input: &FontInput| {
							PropertiesPanelMessage::ModifyFont {
								font_family: font_input.font_family.clone(),
								font_style: font_input.font_style.clone(),
								font_file: Some(font_input.font_file.clone()),
								size,
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
						value: "Size".into(),
						..TextLabel::default()
					})),
					WidgetHolder::new(Widget::Separator(Separator {
						separator_type: SeparatorType::Unrelated,
						direction: SeparatorDirection::Horizontal,
					})),
					WidgetHolder::new(Widget::NumberInput(NumberInput {
						value: layer.size,
						min: Some(1.),
						unit: " px".into(),
						on_update: WidgetCallback::new(move |number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyFont {
								font_family: font_family.clone(),
								font_style: font_style.clone(),
								font_file: font_file.clone(),
								size: number_input.value,
							}
							.into()
						}),
						..Default::default()
					})),
				],
			},
		],
	}
}

fn node_section_fill(fill: &Fill) -> Option<LayoutRow> {
	match fill {
		Fill::Solid(_) | Fill::None => Some(LayoutRow::Section {
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
						value: if let Fill::Solid(color) = fill { Some(color.rgba_hex()) } else { None },
						on_update: WidgetCallback::new(|text_input: &ColorInput| {
							if let Some(value) = &text_input.value {
								if let Some(color) = Color::from_rgba_str(value).or_else(|| Color::from_rgb_str(value)) {
									let new_fill = Fill::Solid(color);
									PropertiesPanelMessage::ModifyFill { fill: new_fill }.into()
								} else {
									PropertiesPanelMessage::ResendActiveProperties.into()
								}
							} else {
								PropertiesPanelMessage::ModifyFill { fill: Fill::None }.into()
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
								value: gradient_1.positions[0].1.map(|color| color.rgba_hex()),
								on_update: WidgetCallback::new(move |text_input: &ColorInput| {
									if let Some(value) = &text_input.value {
										if let Some(color) = Color::from_rgba_str(value).or_else(|| Color::from_rgb_str(value)) {
											let mut new_gradient = (*gradient_1).clone();
											new_gradient.positions[0].1 = Some(color);
											PropertiesPanelMessage::ModifyFill {
												fill: Fill::LinearGradient(new_gradient),
											}
											.into()
										} else {
											PropertiesPanelMessage::ResendActiveProperties.into()
										}
									} else {
										let mut new_gradient = (*gradient_1).clone();
										new_gradient.positions[0].1 = None;
										PropertiesPanelMessage::ModifyFill {
											fill: Fill::LinearGradient(new_gradient),
										}
										.into()
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
								value: gradient_2.positions[1].1.map(|color| color.rgba_hex()),
								on_update: WidgetCallback::new(move |text_input: &ColorInput| {
									if let Some(value) = &text_input.value {
										if let Some(color) = Color::from_rgba_str(value).or_else(|| Color::from_rgb_str(value)) {
											let mut new_gradient = (*gradient_2).clone();
											new_gradient.positions[1].1 = Some(color);
											PropertiesPanelMessage::ModifyFill {
												fill: Fill::LinearGradient(new_gradient),
											}
											.into()
										} else {
											PropertiesPanelMessage::ResendActiveProperties.into()
										}
									} else {
										let mut new_gradient = (*gradient_2).clone();
										new_gradient.positions[1].1 = None;
										PropertiesPanelMessage::ModifyFill {
											fill: Fill::LinearGradient(new_gradient),
										}
										.into()
									}
								}),
							})),
						],
					},
				],
			})
		}
	}
}

fn node_section_stroke(stroke: &Stroke) -> LayoutRow {
	// We have to make multiple variables because they get moved into different closures.
	let internal_stroke1 = stroke.clone();
	let internal_stroke2 = stroke.clone();
	let internal_stroke3 = stroke.clone();
	let internal_stroke4 = stroke.clone();
	let internal_stroke5 = stroke.clone();
	let internal_stroke6 = stroke.clone();
	let internal_stroke7 = stroke.clone();
	let internal_stroke8 = stroke.clone();
	let internal_stroke9 = stroke.clone();
	let internal_stroke10 = stroke.clone();
	let internal_stroke11 = stroke.clone();

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
						value: stroke.color().map(|color| color.rgba_hex()),
						on_update: WidgetCallback::new(move |text_input: &ColorInput| {
							internal_stroke1
								.clone()
								.with_color(&text_input.value)
								.map_or(PropertiesPanelMessage::ResendActiveProperties.into(), |stroke| PropertiesPanelMessage::ModifyStroke { stroke }.into())
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
								stroke: internal_stroke2.clone().with_width(number_input.value as f32),
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
						value: "Dash Lengths".into(),
						..TextLabel::default()
					})),
					WidgetHolder::new(Widget::Separator(Separator {
						separator_type: SeparatorType::Unrelated,
						direction: SeparatorDirection::Horizontal,
					})),
					WidgetHolder::new(Widget::TextInput(TextInput {
						value: stroke.dash_lengths(),
						on_update: WidgetCallback::new(move |text_input: &TextInput| {
							internal_stroke3
								.clone()
								.with_dash_lengths(&text_input.value)
								.map_or(PropertiesPanelMessage::ResendActiveProperties.into(), |stroke| PropertiesPanelMessage::ModifyStroke { stroke }.into())
						}),
					})),
				],
			},
			LayoutRow::Row {
				name: "".into(),
				widgets: vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Dash Offset".into(),
						..TextLabel::default()
					})),
					WidgetHolder::new(Widget::Separator(Separator {
						separator_type: SeparatorType::Unrelated,
						direction: SeparatorDirection::Horizontal,
					})),
					WidgetHolder::new(Widget::NumberInput(NumberInput {
						value: stroke.dash_offset() as f64,
						is_integer: true,
						min: Some(0.),
						unit: " px".into(),
						on_update: WidgetCallback::new(move |number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyStroke {
								stroke: internal_stroke4.clone().with_dash_offset(number_input.value as f32),
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
						value: "Line Cap".into(),
						..TextLabel::default()
					})),
					WidgetHolder::new(Widget::Separator(Separator {
						separator_type: SeparatorType::Unrelated,
						direction: SeparatorDirection::Horizontal,
					})),
					WidgetHolder::new(Widget::RadioInput(RadioInput {
						selected_index: stroke.line_cap_index(),
						entries: vec![
							RadioEntryData {
								label: "Butt".into(),
								on_update: WidgetCallback::new(move |_| {
									PropertiesPanelMessage::ModifyStroke {
										stroke: internal_stroke6.clone().with_line_cap(LineCap::Butt),
									}
									.into()
								}),
								..RadioEntryData::default()
							},
							RadioEntryData {
								label: "Round".into(),
								on_update: WidgetCallback::new(move |_| {
									PropertiesPanelMessage::ModifyStroke {
										stroke: internal_stroke7.clone().with_line_cap(LineCap::Round),
									}
									.into()
								}),
								..RadioEntryData::default()
							},
							RadioEntryData {
								label: "Square".into(),
								on_update: WidgetCallback::new(move |_| {
									PropertiesPanelMessage::ModifyStroke {
										stroke: internal_stroke8.clone().with_line_cap(LineCap::Square),
									}
									.into()
								}),
								..RadioEntryData::default()
							},
						],
					})),
				],
			},
			LayoutRow::Row {
				name: "".into(),
				widgets: vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Line Join".into(),
						..TextLabel::default()
					})),
					WidgetHolder::new(Widget::Separator(Separator {
						separator_type: SeparatorType::Unrelated,
						direction: SeparatorDirection::Horizontal,
					})),
					WidgetHolder::new(Widget::RadioInput(RadioInput {
						selected_index: stroke.line_join_index(),
						entries: vec![
							RadioEntryData {
								label: "Miter".into(),
								on_update: WidgetCallback::new(move |_| {
									PropertiesPanelMessage::ModifyStroke {
										stroke: internal_stroke9.clone().with_line_join(LineJoin::Miter),
									}
									.into()
								}),
								..RadioEntryData::default()
							},
							RadioEntryData {
								label: "Bevel".into(),
								on_update: WidgetCallback::new(move |_| {
									PropertiesPanelMessage::ModifyStroke {
										stroke: internal_stroke10.clone().with_line_join(LineJoin::Bevel),
									}
									.into()
								}),
								..RadioEntryData::default()
							},
							RadioEntryData {
								label: "Round".into(),
								on_update: WidgetCallback::new(move |_| {
									PropertiesPanelMessage::ModifyStroke {
										stroke: internal_stroke11.clone().with_line_join(LineJoin::Round),
									}
									.into()
								}),
								..RadioEntryData::default()
							},
						],
					})),
				],
			},
			// TODO: Gray out this row when Line Join isn't set to Miter
			LayoutRow::Row {
				name: "".into(),
				widgets: vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Miter Limit".into(),
						..TextLabel::default()
					})),
					WidgetHolder::new(Widget::Separator(Separator {
						separator_type: SeparatorType::Unrelated,
						direction: SeparatorDirection::Horizontal,
					})),
					WidgetHolder::new(Widget::NumberInput(NumberInput {
						value: stroke.miter_limit() as f64,
						is_integer: true,
						min: Some(0.),
						unit: "".into(),
						on_update: WidgetCallback::new(move |number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyStroke {
								stroke: internal_stroke5.clone().with_miter_limit(number_input.value as f32),
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
