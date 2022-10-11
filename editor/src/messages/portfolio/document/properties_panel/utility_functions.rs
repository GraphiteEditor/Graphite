use super::utility_types::TransformOp;
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::layout::utility_types::widgets::assist_widgets::PivotAssist;
use crate::messages::layout::utility_types::widgets::button_widgets::{PopoverButton, TextButton};
use crate::messages::layout::utility_types::widgets::input_widgets::{CheckboxInput, ColorInput, FontInput, NumberInput, RadioEntryData, RadioInput, TextAreaInput, TextInput};
use crate::messages::layout::utility_types::widgets::label_widgets::{IconLabel, IconStyle, Separator, SeparatorDirection, SeparatorType, TextLabel};
use crate::messages::prelude::*;

use graphene::color::Color;
use graphene::layers::ai_artist_layer::AiArtistLayer;
use graphene::layers::layer_info::{Layer, LayerDataType, LayerDataTypeDiscriminant};
use graphene::layers::style::{Fill, Gradient, GradientType, LineCap, LineJoin, Stroke};
use graphene::layers::text_layer::{FontCache, TextLayer};

use glam::{DAffine2, DVec2};
use std::f64::consts::PI;
use std::rc::Rc;

pub fn apply_transform_operation(layer: &Layer, transform_op: TransformOp, value: f64, font_cache: &FontCache) -> [f64; 6] {
	let transformation = match transform_op {
		TransformOp::X => DAffine2::update_x,
		TransformOp::Y => DAffine2::update_y,
		TransformOp::ScaleX | TransformOp::Width => DAffine2::update_scale_x,
		TransformOp::ScaleY | TransformOp::Height => DAffine2::update_scale_y,
		TransformOp::Rotation => DAffine2::update_rotation,
	};

	let scale = match transform_op {
		TransformOp::Width => layer.bounding_transform(font_cache).scale_x() / layer.transform.scale_x(),
		TransformOp::Height => layer.bounding_transform(font_cache).scale_y() / layer.transform.scale_y(),
		_ => 1.,
	};

	transformation(layer.transform, value / scale).to_cols_array()
}

pub fn register_artboard_layer_properties(layer: &Layer, responses: &mut VecDeque<Message>, font_cache: &FontCache) {
	let options_bar = vec![LayoutGroup::Row {
		widgets: vec![
			WidgetHolder::new(Widget::IconLabel(IconLabel {
				icon: "NodeArtboard".into(),
				icon_style: IconStyle::Node,
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
				..Default::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Related,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::PopoverButton(PopoverButton {
				header: "Options Bar".into(),
				text: "The contents of this popover menu are coming soon".into(),
				..Default::default()
			})),
		],
	}];

	let properties_body = {
		let shape = if let LayerDataType::Shape(shape) = &layer.data {
			shape
		} else {
			panic!("Artboards can only be shapes")
		};
		let color = if let Fill::Solid(color) = shape.style.fill() {
			color
		} else {
			panic!("Artboard must have a solid fill")
		};
		let pivot = layer.transform.transform_vector2(layer.layerspace_pivot(font_cache));

		vec![LayoutGroup::Section {
			name: "Artboard".into(),
			layout: vec![
				LayoutGroup::Row {
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
							value: Some(layer.transform.x() + pivot.x),
							label: "X".into(),
							unit: " px".into(),
							on_update: WidgetCallback::new(move |number_input: &NumberInput| {
								PropertiesPanelMessage::ModifyTransform {
									value: number_input.value.unwrap() - pivot.x,
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
							value: Some(layer.transform.y() + pivot.y),
							label: "Y".into(),
							unit: " px".into(),
							on_update: WidgetCallback::new(move |number_input: &NumberInput| {
								PropertiesPanelMessage::ModifyTransform {
									value: number_input.value.unwrap() - pivot.y,
									transform_op: TransformOp::Y,
								}
								.into()
							}),
							..NumberInput::default()
						})),
					],
				},
				LayoutGroup::Row {
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
							value: Some(layer.bounding_transform(font_cache).scale_x()),
							label: "W".into(),
							unit: " px".into(),
							on_update: WidgetCallback::new(|number_input: &NumberInput| {
								PropertiesPanelMessage::ModifyTransform {
									value: number_input.value.unwrap(),
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
							value: Some(layer.bounding_transform(font_cache).scale_y()),
							label: "H".into(),
							unit: " px".into(),
							on_update: WidgetCallback::new(|number_input: &NumberInput| {
								PropertiesPanelMessage::ModifyTransform {
									value: number_input.value.unwrap(),
									transform_op: TransformOp::Height,
								}
								.into()
							}),
							..NumberInput::default()
						})),
					],
				},
				LayoutGroup::Row {
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
							value: Some(color.rgba_hex()),
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
							no_transparency: true,
							..Default::default()
						})),
					],
				},
			],
		}]
	};

	responses.push_back(
		LayoutMessage::SendLayout {
			layout: Layout::WidgetLayout(WidgetLayout::new(options_bar)),
			layout_target: LayoutTarget::PropertiesOptions,
		}
		.into(),
	);
	responses.push_back(
		LayoutMessage::SendLayout {
			layout: Layout::WidgetLayout(WidgetLayout::new(properties_body)),
			layout_target: LayoutTarget::PropertiesSections,
		}
		.into(),
	);
}

pub fn register_artwork_layer_properties(layer: &Layer, responses: &mut VecDeque<Message>, font_cache: &FontCache) {
	let options_bar = vec![LayoutGroup::Row {
		widgets: vec![
			match &layer.data {
				LayerDataType::Folder(_) => WidgetHolder::new(Widget::IconLabel(IconLabel {
					icon: "NodeFolder".into(),
					icon_style: IconStyle::Node,
				})),
				LayerDataType::Shape(_) => WidgetHolder::new(Widget::IconLabel(IconLabel {
					icon: "NodeShape".into(),
					icon_style: IconStyle::Node,
				})),
				LayerDataType::Text(_) => WidgetHolder::new(Widget::IconLabel(IconLabel {
					icon: "NodeText".into(),
					icon_style: IconStyle::Node,
				})),
				LayerDataType::Image(_) => WidgetHolder::new(Widget::IconLabel(IconLabel {
					icon: "NodeImage".into(),
					icon_style: IconStyle::Node,
				})),
				LayerDataType::AiArtist(_) => WidgetHolder::new(Widget::IconLabel(IconLabel {
					icon: "NodeAiArtist".into(),
					icon_style: IconStyle::Node,
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
				..Default::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Related,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::PopoverButton(PopoverButton {
				header: "Options Bar".into(),
				text: "The contents of this popover menu are coming soon".into(),
				..Default::default()
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
		LayerDataType::AiArtist(ai_artist) => {
			vec![node_section_transform(layer, font_cache), node_section_ai_artist(ai_artist, layer, font_cache)]
		}
		LayerDataType::Folder(_) => {
			vec![node_section_transform(layer, font_cache)]
		}
	};

	responses.push_back(
		LayoutMessage::SendLayout {
			layout: Layout::WidgetLayout(WidgetLayout::new(options_bar)),
			layout_target: LayoutTarget::PropertiesOptions,
		}
		.into(),
	);
	responses.push_back(
		LayoutMessage::SendLayout {
			layout: Layout::WidgetLayout(WidgetLayout::new(properties_body)),
			layout_target: LayoutTarget::PropertiesSections,
		}
		.into(),
	);
}

fn node_section_transform(layer: &Layer, font_cache: &FontCache) -> LayoutGroup {
	let pivot = layer.transform.transform_vector2(layer.layerspace_pivot(font_cache));
	LayoutGroup::Section {
		name: "Transform".into(),
		layout: vec![
			LayoutGroup::Row {
				widgets: vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Location".into(),
						..TextLabel::default()
					})),
					WidgetHolder::new(Widget::Separator(Separator {
						separator_type: SeparatorType::Related,
						direction: SeparatorDirection::Horizontal,
					})),
					WidgetHolder::new(Widget::PivotAssist(PivotAssist {
						position: layer.pivot.into(),
						on_update: WidgetCallback::new(|pivot_assist: &PivotAssist| PropertiesPanelMessage::SetPivot { new_position: pivot_assist.position }.into()),
					})),
					WidgetHolder::new(Widget::Separator(Separator {
						separator_type: SeparatorType::Unrelated,
						direction: SeparatorDirection::Horizontal,
					})),
					WidgetHolder::new(Widget::NumberInput(NumberInput {
						value: Some(layer.transform.x() + pivot.x),
						label: "X".into(),
						unit: " px".into(),
						on_update: WidgetCallback::new(move |number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyTransform {
								value: number_input.value.unwrap() - pivot.x,
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
						value: Some(layer.transform.y() + pivot.y),
						label: "Y".into(),
						unit: " px".into(),
						on_update: WidgetCallback::new(move |number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyTransform {
								value: number_input.value.unwrap() - pivot.y,
								transform_op: TransformOp::Y,
							}
							.into()
						}),
						..NumberInput::default()
					})),
				],
			},
			LayoutGroup::Row {
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
						value: Some(layer.transform.rotation() * 180. / PI),
						label: "".into(),
						unit: "°".into(),
						on_update: WidgetCallback::new(|number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyTransform {
								value: number_input.value.unwrap() / 180. * PI,
								transform_op: TransformOp::Rotation,
							}
							.into()
						}),
						..NumberInput::default()
					})),
				],
			},
			LayoutGroup::Row {
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
						value: Some(layer.transform.scale_x()),
						label: "X".into(),
						unit: "".into(),
						on_update: WidgetCallback::new(|number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyTransform {
								value: number_input.value.unwrap(),
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
						value: Some(layer.transform.scale_y()),
						label: "Y".into(),
						unit: "".into(),
						on_update: WidgetCallback::new(|number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyTransform {
								value: number_input.value.unwrap(),
								transform_op: TransformOp::ScaleY,
							}
							.into()
						}),
						..NumberInput::default()
					})),
				],
			},
			LayoutGroup::Row {
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
						value: Some(layer.bounding_transform(font_cache).scale_x()),
						label: "W".into(),
						unit: " px".into(),
						on_update: WidgetCallback::new(|number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyTransform {
								value: number_input.value.unwrap(),
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
						value: Some(layer.bounding_transform(font_cache).scale_y()),
						label: "H".into(),
						unit: " px".into(),
						on_update: WidgetCallback::new(|number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyTransform {
								value: number_input.value.unwrap(),
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

fn node_section_ai_artist(ai_artist_layer: &AiArtistLayer, layer: &Layer, font_cache: &FontCache) -> LayoutGroup {
	LayoutGroup::Section {
		name: "AI Artist".into(),
		layout: vec![
			LayoutGroup::Row {
				widgets: [
					vec![
						WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: "Image".into(),
							..Default::default()
						})),
						WidgetHolder::new(Widget::Separator(Separator {
							separator_type: SeparatorType::Unrelated,
							direction: SeparatorDirection::Horizontal,
						})),
					],
					{
						if ai_artist_layer.blob_url != None && ai_artist_layer.percent_complete < 100. && !ai_artist_layer.terminated {
							vec![WidgetHolder::new(Widget::TextButton(TextButton {
								label: "Terminate".into(),
								on_update: WidgetCallback::new(|_| DocumentMessage::AiArtistTerminate.into()),
								..Default::default()
							}))]
						} else {
							vec![
								WidgetHolder::new(Widget::TextButton(TextButton {
									label: "Generate".into(),
									on_update: WidgetCallback::new(|_| DocumentMessage::AiArtistGenerate.into()),
									..Default::default()
								})),
								WidgetHolder::new(Widget::Separator(Separator {
									separator_type: SeparatorType::Related,
									direction: SeparatorDirection::Horizontal,
								})),
								WidgetHolder::new(Widget::TextButton(TextButton {
									label: "Clear".into(),
									disabled: ai_artist_layer.blob_url == None,
									on_update: WidgetCallback::new(|_| DocumentMessage::AiArtistClear.into()),
									..Default::default()
								})),
							]
						}
					},
				]
				.concat(),
			},
			LayoutGroup::Row {
				widgets: vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Progress".into(),
						..Default::default()
					})),
					WidgetHolder::new(Widget::Separator(Separator {
						separator_type: SeparatorType::Unrelated,
						direction: SeparatorDirection::Horizontal,
					})),
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: if ai_artist_layer.blob_url == None {
							"Ready".into()
						} else if ai_artist_layer.percent_complete == 100. {
							"Done".into()
						} else if ai_artist_layer.terminated {
							format!("{:.0}% (Terminated)", ai_artist_layer.percent_complete)
						} else {
							format!("{:.0}%", ai_artist_layer.percent_complete)
						},
						bold: true,
						..Default::default()
					})),
				],
			},
			LayoutGroup::Row {
				widgets: vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Resolution".into(),
						..Default::default()
					})),
					WidgetHolder::new(Widget::Separator(Separator {
						separator_type: SeparatorType::Unrelated,
						direction: SeparatorDirection::Horizontal,
					})),
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: {
							let (width, height) = pick_layer_safe_resolution(layer, font_cache);
							format!("{} W x {} H", width, height)
						},
						bold: true,
						..Default::default()
					})),
				],
			},
			LayoutGroup::Row {
				widgets: vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Sample Steps".into(),
						..Default::default()
					})),
					WidgetHolder::new(Widget::Separator(Separator {
						separator_type: SeparatorType::Unrelated,
						direction: SeparatorDirection::Horizontal,
					})),
					WidgetHolder::new(Widget::NumberInput(NumberInput {
						value: Some(ai_artist_layer.samples.into()),
						on_update: WidgetCallback::new(move |number_input: &NumberInput| {
							PropertiesPanelMessage::SetAiArtistSamples {
								samples: number_input.value.unwrap().round() as u32,
							}
							.into()
						}),
						..Default::default()
					})),
				],
			},
			LayoutGroup::Row {
				widgets: vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Text Prompt".into(),
						..Default::default()
					})),
					WidgetHolder::new(Widget::Separator(Separator {
						separator_type: SeparatorType::Unrelated,
						direction: SeparatorDirection::Horizontal,
					})),
					WidgetHolder::new(Widget::TextAreaInput(TextAreaInput {
						value: ai_artist_layer.prompt.clone(),
						on_update: WidgetCallback::new(move |text_area_input: &TextAreaInput| {
							PropertiesPanelMessage::SetAiArtistPrompt {
								prompt: text_area_input.value.clone(),
							}
							.into()
						}),
						..Default::default()
					})),
				],
			},
			LayoutGroup::Row {
				widgets: vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Text Creativity".into(),
						..Default::default()
					})),
					WidgetHolder::new(Widget::Separator(Separator {
						separator_type: SeparatorType::Unrelated,
						direction: SeparatorDirection::Horizontal,
					})),
					WidgetHolder::new(Widget::NumberInput(NumberInput {
						value: Some(ai_artist_layer.cfg_scale),
						on_update: WidgetCallback::new(move |number_input: &NumberInput| {
							PropertiesPanelMessage::SetAiArtistCfgScale {
								cfg_scale: number_input.value.unwrap(),
							}
							.into()
						}),
						..Default::default()
					})),
				],
			},
			LayoutGroup::Row {
				widgets: vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Image Prompt".into(),
						..Default::default()
					})),
					WidgetHolder::new(Widget::Separator(Separator {
						separator_type: SeparatorType::Unrelated,
						direction: SeparatorDirection::Horizontal,
					})),
					WidgetHolder::new(Widget::CheckboxInput(CheckboxInput {
						checked: ai_artist_layer.use_img2img,
						on_update: WidgetCallback::new(move |checkbox_input: &CheckboxInput| PropertiesPanelMessage::SetAiArtistUseImg2Img { use_img2img: checkbox_input.checked }.into()),
						..Default::default()
					})),
				],
			},
			LayoutGroup::Row {
				widgets: vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Image Creativity".into(),
						..Default::default()
					})),
					WidgetHolder::new(Widget::Separator(Separator {
						separator_type: SeparatorType::Unrelated,
						direction: SeparatorDirection::Horizontal,
					})),
					WidgetHolder::new(Widget::NumberInput(NumberInput {
						value: Some(ai_artist_layer.denoising_strength),
						disabled: !ai_artist_layer.use_img2img,
						on_update: WidgetCallback::new(move |number_input: &NumberInput| {
							PropertiesPanelMessage::SetAiArtistDenoisingStrength {
								denoising_strength: number_input.value.unwrap(),
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

fn node_section_font(layer: &TextLayer) -> LayoutGroup {
	let font = layer.font.clone();
	let size = layer.size;
	LayoutGroup::Section {
		name: "Font".into(),
		layout: vec![
			LayoutGroup::Row {
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
						..Default::default()
					})),
				],
			},
			LayoutGroup::Row {
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
						font_family: layer.font.font_family.clone(),
						font_style: layer.font.font_style.clone(),
						on_update: WidgetCallback::new(move |font_input: &FontInput| {
							PropertiesPanelMessage::ModifyFont {
								font_family: font_input.font_family.clone(),
								font_style: font_input.font_style.clone(),
								size,
							}
							.into()
						}),
						..Default::default()
					})),
				],
			},
			LayoutGroup::Row {
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
						font_family: layer.font.font_family.clone(),
						font_style: layer.font.font_style.clone(),
						on_update: WidgetCallback::new(move |font_input: &FontInput| {
							PropertiesPanelMessage::ModifyFont {
								font_family: font_input.font_family.clone(),
								font_style: font_input.font_style.clone(),
								size,
							}
							.into()
						}),
						..Default::default()
					})),
				],
			},
			LayoutGroup::Row {
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
						value: Some(layer.size),
						min: Some(1.),
						unit: " px".into(),
						on_update: WidgetCallback::new(move |number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyFont {
								font_family: font.font_family.clone(),
								font_style: font.font_style.clone(),
								size: number_input.value.unwrap(),
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

fn node_gradient_type(gradient: &Gradient) -> LayoutGroup {
	let selected_index = match gradient.gradient_type {
		GradientType::Linear => 0,
		GradientType::Radial => 1,
	};
	let mut cloned_gradient_linear = gradient.clone();
	cloned_gradient_linear.gradient_type = GradientType::Linear;
	let mut cloned_gradient_radial = gradient.clone();
	cloned_gradient_radial.gradient_type = GradientType::Radial;
	LayoutGroup::Row {
		widgets: vec![
			WidgetHolder::new(Widget::TextLabel(TextLabel {
				value: "Gradient Type".into(),
				..TextLabel::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Unrelated,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::RadioInput(RadioInput {
				selected_index,
				entries: vec![
					RadioEntryData {
						value: "linear".into(),
						label: "Linear".into(),
						tooltip: "Linear Gradient".into(),
						on_update: WidgetCallback::new(move |_| {
							PropertiesPanelMessage::ModifyFill {
								fill: Fill::Gradient(cloned_gradient_linear.clone()),
							}
							.into()
						}),
						..RadioEntryData::default()
					},
					RadioEntryData {
						value: "radial".into(),
						label: "Radial".into(),
						tooltip: "Radial Gradient".into(),
						on_update: WidgetCallback::new(move |_| {
							PropertiesPanelMessage::ModifyFill {
								fill: Fill::Gradient(cloned_gradient_radial.clone()),
							}
							.into()
						}),
						..RadioEntryData::default()
					},
				],
			})),
		],
	}
}

fn node_gradient_color(gradient: &Gradient, percent_label: &'static str, position: usize) -> LayoutGroup {
	let gradient_clone = Rc::new(gradient.clone());
	let send_fill_message = move |new_gradient: Gradient| PropertiesPanelMessage::ModifyFill { fill: Fill::Gradient(new_gradient) }.into();
	LayoutGroup::Row {
		widgets: vec![
			WidgetHolder::new(Widget::TextLabel(TextLabel {
				value: format!("Gradient: {}", percent_label),
				..TextLabel::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Unrelated,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::ColorInput(ColorInput {
				value: gradient_clone.positions[position].1.map(|color| color.rgba_hex()),
				on_update: WidgetCallback::new(move |text_input: &ColorInput| {
					if let Some(value) = &text_input.value {
						if let Some(color) = Color::from_rgba_str(value).or_else(|| Color::from_rgb_str(value)) {
							let mut new_gradient = (*gradient_clone).clone();
							new_gradient.positions[position].1 = Some(color);
							send_fill_message(new_gradient)
						} else {
							PropertiesPanelMessage::ResendActiveProperties.into()
						}
					} else {
						let mut new_gradient = (*gradient_clone).clone();
						new_gradient.positions[position].1 = None;
						send_fill_message(new_gradient)
					}
				}),
				..ColorInput::default()
			})),
		],
	}
}

fn node_section_fill(fill: &Fill) -> Option<LayoutGroup> {
	match fill {
		Fill::Solid(_) | Fill::None => Some(LayoutGroup::Section {
			name: "Fill".into(),
			layout: vec![LayoutGroup::Row {
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
						..ColorInput::default()
					})),
				],
			}],
		}),
		Fill::Gradient(gradient) => Some(LayoutGroup::Section {
			name: "Fill".into(),
			layout: vec![node_gradient_type(gradient), node_gradient_color(gradient, "0%", 0), node_gradient_color(gradient, "100%", 1)],
		}),
	}
}

fn node_section_stroke(stroke: &Stroke) -> LayoutGroup {
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

	LayoutGroup::Section {
		name: "Stroke".into(),
		layout: vec![
			LayoutGroup::Row {
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
						..ColorInput::default()
					})),
				],
			},
			LayoutGroup::Row {
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
						value: Some(stroke.weight() as f64),
						is_integer: false,
						min: Some(0.),
						unit: " px".into(),
						on_update: WidgetCallback::new(move |number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyStroke {
								stroke: internal_stroke2.clone().with_weight(number_input.value.unwrap()),
							}
							.into()
						}),
						..NumberInput::default()
					})),
				],
			},
			LayoutGroup::Row {
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
						..Default::default()
					})),
				],
			},
			LayoutGroup::Row {
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
						value: Some(stroke.dash_offset() as f64),
						is_integer: true,
						min: Some(0.),
						unit: " px".into(),
						on_update: WidgetCallback::new(move |number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyStroke {
								stroke: internal_stroke4.clone().with_dash_offset(number_input.value.unwrap()),
							}
							.into()
						}),
						..NumberInput::default()
					})),
				],
			},
			LayoutGroup::Row {
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
			LayoutGroup::Row {
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
			LayoutGroup::Row {
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
						value: Some(stroke.line_join_miter_limit() as f64),
						is_integer: true,
						min: Some(0.),
						unit: "".into(),
						on_update: WidgetCallback::new(move |number_input: &NumberInput| {
							PropertiesPanelMessage::ModifyStroke {
								stroke: internal_stroke5.clone().with_line_join_miter_limit(number_input.value.unwrap()),
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
		let scale_x = self.scale_x();
		if scale_x != 0. {
			self * DAffine2::from_scale((new_width / scale_x, 1.).into())
		} else {
			self
		}
	}

	fn scale_y(&self) -> f64 {
		self.transform_vector2((0., 1.).into()).length()
	}

	fn update_scale_y(self, new_height: f64) -> Self {
		let scale_y = self.scale_y();
		if scale_y != 0. {
			self * DAffine2::from_scale((1., new_height / scale_y).into())
		} else {
			self
		}
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
		if self.scale_x() != 0. {
			let cos = self.matrix2.col(0).x / self.scale_x();
			let sin = self.matrix2.col(0).y / self.scale_x();
			sin.atan2(cos)
		} else if self.scale_y() != 0. {
			let sin = -self.matrix2.col(1).x / self.scale_y();
			let cos = self.matrix2.col(1).y / self.scale_y();
			sin.atan2(cos)
		} else {
			// Rotation information does not exists anymore in the matrix
			// return 0 for user experience.
			0.
		}
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

pub fn pick_layer_safe_resolution(layer: &Layer, font_cache: &FontCache) -> (u64, u64) {
	let layer_bounds = layer.bounding_transform(font_cache);
	let layer_bounds_size = (layer_bounds.transform_vector2((1., 0.).into()).length(), layer_bounds.transform_vector2((0., 1.).into()).length());

	pick_safe_resolution(layer_bounds_size)
}

pub fn pick_safe_resolution((width, height): (f64, f64)) -> (u64, u64) {
	// const MAX_RESOLUTION: u64 = 1024 * 1024;
	const MAX_RESOLUTION: u64 = 960 * 960;

	let mut scale_factor = 1.;

	let round_to_increment = |size: f64| (size / 64.).round() as u64 * 64;

	loop {
		let possible_solution = (round_to_increment(width * scale_factor), round_to_increment(height * scale_factor));

		if possible_solution.0 * possible_solution.1 <= MAX_RESOLUTION {
			return possible_solution;
		}

		scale_factor -= 0.1;
	}
}
