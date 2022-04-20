use super::layer_panel::LayerDataTypeDiscriminant;
use super::utility_types::TargetDocument;
use crate::document::properties_panel_message::TransformOp;
use crate::layout::layout_message::LayoutTarget;
use crate::layout::widgets::{
	ColorInput, IconLabel, LayoutRow, NumberInput, PopoverButton, RadioEntryData, RadioInput, Separator, SeparatorDirection, SeparatorType, TextInput, TextLabel, Widget, WidgetCallback, WidgetHolder,
	WidgetLayout,
};
use crate::message_prelude::*;

use graphene::color::Color;
use graphene::document::Document as GrapheneDocument;
use graphene::layers::layer_info::{Layer, LayerDataType};
use graphene::layers::style::{Fill, LineCap, LineJoin, Stroke};
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PropertiesPanelMessageHandler {
	active_selection: Option<(Vec<LayerId>, TargetDocument)>,
}

impl PropertiesPanelMessageHandler {
	fn matches_selected(&self, path: &[LayerId]) -> bool {
		let last_active_path = self.active_selection.as_ref().and_then(|(v, _)| v.last().copied());
		let last_modified = path.last().copied();
		matches!((last_active_path, last_modified), (Some(active_last), Some(modified_last)) if active_last == modified_last)
	}

	fn create_document_operation(&self, operation: Operation) -> Message {
		let (_, target_document) = self.active_selection.as_ref().unwrap();
		match *target_document {
			TargetDocument::Artboard => ArtboardMessage::DispatchOperation(Box::new(operation)).into(),
			TargetDocument::Artwork => DocumentMessage::DispatchOperation(Box::new(operation)).into(),
		}
	}
}

pub struct PropertiesPanelMessageHandlerData<'a> {
	pub artwork_document: &'a GrapheneDocument,
	pub artboard_document: &'a GrapheneDocument,
}

impl<'a> MessageHandler<PropertiesPanelMessage, PropertiesPanelMessageHandlerData<'a>> for PropertiesPanelMessageHandler {
	#[remain::check]
	fn process_action(&mut self, message: PropertiesPanelMessage, data: PropertiesPanelMessageHandlerData, responses: &mut VecDeque<Message>) {
		let PropertiesPanelMessageHandlerData { artwork_document, artboard_document } = data;
		let get_document = |document_selector: TargetDocument| match document_selector {
			TargetDocument::Artboard => artboard_document,
			TargetDocument::Artwork => artwork_document,
		};
		use PropertiesPanelMessage::*;
		match message {
			SetActiveLayers { paths, document } => {
				if paths.len() > 1 {
					// TODO: Allow for multiple selected layers
					responses.push_back(PropertiesPanelMessage::ClearSelection.into())
				} else {
					let path = paths.into_iter().next().unwrap();
					self.active_selection = Some((path, document));
					responses.push_back(PropertiesPanelMessage::ResendActiveProperties.into())
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
				let (path, target_document) = self.active_selection.as_ref().expect("Received update for properties panel with no active layer");
				let layer = get_document(*target_document).layer(path).unwrap();

				use TransformOp::*;
				let action = match transform_op {
					X => DAffine2::update_x,
					Y => DAffine2::update_y,
					ScaleX | Width => DAffine2::update_scale_x,
					ScaleY | Height => DAffine2::update_scale_y,
					Rotation => DAffine2::update_rotation,
				};

				let scale = match transform_op {
					Width => layer.bounding_transform().scale_x() / layer.transform.scale_x(),
					Height => layer.bounding_transform().scale_y() / layer.transform.scale_y(),
					_ => 1.,
				};

				responses.push_back(self.create_document_operation(Operation::SetLayerTransform {
					path: path.clone(),
					transform: action(layer.transform, value / scale).to_cols_array(),
				}));
			}
			ModifyName { name } => {
				let (path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				responses.push_back(self.create_document_operation(Operation::SetLayerName { path, name }))
			}
			ModifyFill { fill } => {
				let (path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				responses.push_back(self.create_document_operation(Operation::SetLayerFill { path, fill }));
			}
			ModifyStroke { stroke } => {
				let (path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				responses.push_back(self.create_document_operation(Operation::SetLayerStroke { path, stroke }))
			}
			CheckSelectedWasUpdated { path } => {
				if self.matches_selected(&path) {
					responses.push_back(PropertiesPanelMessage::ResendActiveProperties.into())
				}
			}
			CheckSelectedWasDeleted { path } => {
				if self.matches_selected(&path) {
					self.active_selection = None;
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
				let (path, target_document) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				let layer = get_document(target_document).layer(&path).unwrap();
				match target_document {
					TargetDocument::Artboard => register_artboard_layer_properties(layer, responses),
					TargetDocument::Artwork => register_artwork_layer_properties(layer, responses),
				}
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(PropertiesMessageDiscriminant;)
	}
}

fn register_artboard_layer_properties(layer: &Layer, responses: &mut VecDeque<Message>) {
	let options_bar = vec![LayoutRow::Row {
		widgets: vec![
			// TODO: Replace with custom Artboard Icon
			WidgetHolder::new(Widget::IconLabel(IconLabel {
				icon: "NodeShape".into(),
				gap_after: true,
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Related,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::TextLabel(TextLabel {
				value: "Artboard".into(),
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
			let artboard_properties = match shape.style.fill() {
				Fill::Solid(color) => LayoutRow::Section {
					name: "Artboard".into(),
					layout: vec![LayoutRow::Row {
						widgets: vec![
							WidgetHolder::new(Widget::TextLabel(TextLabel {
								value: "Background".into(),
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
				},
				_ => panic!("Artboard must have a solid fill"),
			};

			vec![node_section_transform(layer), artboard_properties]
		}
		_ => panic!("Artboard's can only be shapes"),
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

fn register_artwork_layer_properties(layer: &Layer, responses: &mut VecDeque<Message>) {
	let options_bar = vec![LayoutRow::Row {
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
		LayerDataType::Image(_) => {
			vec![node_section_transform(layer)]
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
						value: layer.bounding_transform().scale_x(),
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
						value: layer.bounding_transform().scale_y(),
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

fn node_section_fill(fill: &Fill) -> Option<LayoutRow> {
	match fill {
		Fill::Solid(color) => Some(LayoutRow::Section {
			name: "Fill".into(),
			layout: vec![LayoutRow::Row {
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
							internal_stroke1
								.clone()
								.with_color(&text_input.value)
								.map_or(PropertiesPanelMessage::ResendActiveProperties.into(), |stroke| PropertiesPanelMessage::ModifyStroke { stroke }.into())
						}),
					})),
				],
			},
			LayoutRow::Row {
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
