use super::utility_types::TransformOp;
use crate::application::generate_uuid;
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::layout::utility_types::widgets::assist_widgets::PivotAssist;
use crate::messages::layout::utility_types::widgets::button_widgets::{IconButton, PopoverButton, TextButton};
use crate::messages::layout::utility_types::widgets::input_widgets::{
	CheckboxInput, ColorInput, DropdownEntryData, DropdownInput, FontInput, NumberInput, NumberInputMode, RadioEntryData, RadioInput, TextAreaInput, TextInput,
};
use crate::messages::layout::utility_types::widgets::label_widgets::{IconLabel, Separator, SeparatorDirection, SeparatorType, TextLabel};
use crate::messages::portfolio::utility_types::{ImaginateServerStatus, PersistentData};
use crate::messages::prelude::*;

use graphene::color::Color;
use graphene::document::pick_layer_safe_imaginate_resolution;
use graphene::layers::imaginate_layer::{ImaginateLayer, ImaginateSamplingMethod, ImaginateStatus};
use graphene::layers::layer_info::{Layer, LayerDataType, LayerDataTypeDiscriminant};
use graphene::layers::nodegraph_layer::NodeGraphFrameLayer;
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

pub fn register_artboard_layer_properties(layer: &Layer, responses: &mut VecDeque<Message>, persistent_data: &PersistentData) {
	let options_bar = vec![LayoutGroup::Row {
		widgets: vec![
			WidgetHolder::new(Widget::IconLabel(IconLabel {
				icon: "NodeArtboard".into(),
				tooltip: "Artboard".into(),
				..Default::default()
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
		let pivot = layer.transform.transform_vector2(layer.layerspace_pivot(&persistent_data.font_cache));

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
							value: Some(layer.bounding_transform(&persistent_data.font_cache).scale_x()),
							label: "W".into(),
							unit: " px".into(),
							is_integer: true,
							min: Some(1.),
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
							value: Some(layer.bounding_transform(&persistent_data.font_cache).scale_y()),
							label: "H".into(),
							unit: " px".into(),
							is_integer: true,
							min: Some(1.),
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
							value: Some(*color),
							on_update: WidgetCallback::new(|text_input: &ColorInput| {
								let fill = if let Some(value) = text_input.value { Fill::Solid(value) } else { Fill::None };
								PropertiesPanelMessage::ModifyFill { fill }.into()
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

pub fn register_artwork_layer_properties(
	layer_path: Vec<graphene::LayerId>,
	layer: &Layer,
	responses: &mut VecDeque<Message>,
	persistent_data: &PersistentData,
	node_graph_message_handler: &NodeGraphMessageHandler,
) {
	let options_bar = vec![LayoutGroup::Row {
		widgets: vec![
			match &layer.data {
				LayerDataType::Folder(_) => WidgetHolder::new(Widget::IconLabel(IconLabel {
					icon: "NodeFolder".into(),
					tooltip: "Folder".into(),
					..Default::default()
				})),
				LayerDataType::Shape(_) => WidgetHolder::new(Widget::IconLabel(IconLabel {
					icon: "NodeShape".into(),
					tooltip: "Shape".into(),
					..Default::default()
				})),
				LayerDataType::Text(_) => WidgetHolder::new(Widget::IconLabel(IconLabel {
					icon: "NodeText".into(),
					tooltip: "Text".into(),
					..Default::default()
				})),
				LayerDataType::Image(_) => WidgetHolder::new(Widget::IconLabel(IconLabel {
					icon: "NodeImage".into(),
					tooltip: "Image".into(),
					..Default::default()
				})),
				LayerDataType::Imaginate(_) => WidgetHolder::new(Widget::IconLabel(IconLabel {
					icon: "NodeImaginate".into(),
					tooltip: "Imaginate".into(),
					..Default::default()
				})),
				LayerDataType::NodeGraphFrame(_) => WidgetHolder::new(Widget::IconLabel(IconLabel {
					icon: "NodeNodes".into(),
					tooltip: "Node Graph Frame".into(),
					..Default::default()
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
				vec![
					node_section_transform(layer, persistent_data),
					fill_layout,
					node_section_stroke(&shape.style.stroke().unwrap_or_default()),
				]
			} else {
				vec![node_section_transform(layer, persistent_data), node_section_stroke(&shape.style.stroke().unwrap_or_default())]
			}
		}
		LayerDataType::Text(text) => {
			vec![
				node_section_transform(layer, persistent_data),
				node_section_font(text),
				node_section_fill(text.path_style.fill()).expect("Text should have fill"),
				node_section_stroke(&text.path_style.stroke().unwrap_or_default()),
			]
		}
		LayerDataType::Image(_) => {
			vec![node_section_transform(layer, persistent_data)]
		}
		LayerDataType::Imaginate(imaginate) => {
			vec![node_section_transform(layer, persistent_data), node_section_imaginate(imaginate, layer, persistent_data, responses)]
		}
		LayerDataType::NodeGraphFrame(node_graph_frame) => {
			let is_graph_open = node_graph_message_handler.layer_path.as_ref().filter(|node_graph| *node_graph == &layer_path).is_some();
			let selected_nodes = &node_graph_message_handler.selected_nodes;
			if !selected_nodes.is_empty() && is_graph_open {
				node_graph_message_handler.collate_properties(&node_graph_frame)
			} else {
				vec![
					node_section_transform(layer, persistent_data),
					node_section_node_graph_frame(layer_path, node_graph_frame, is_graph_open),
				]
			}
		}
		LayerDataType::Folder(_) => {
			vec![node_section_transform(layer, persistent_data)]
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

fn node_section_transform(layer: &Layer, persistent_data: &PersistentData) -> LayoutGroup {
	let pivot = layer.transform.transform_vector2(layer.layerspace_pivot(&persistent_data.font_cache));
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
						..Default::default()
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
						unit: "°".into(),
						mode: NumberInputMode::Range,
						range_min: Some(-180.),
						range_max: Some(180.),
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
						value: Some(layer.bounding_transform(&persistent_data.font_cache).scale_x()),
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
						value: Some(layer.bounding_transform(&persistent_data.font_cache).scale_y()),
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

fn node_section_imaginate(imaginate_layer: &ImaginateLayer, layer: &Layer, persistent_data: &PersistentData, responses: &mut VecDeque<Message>) -> LayoutGroup {
	LayoutGroup::Section {
		name: "Imaginate".into(),
		layout: vec![
			LayoutGroup::Row {
				widgets: {
					let tooltip = "Connection status to the server that computes generated images".to_string();

					vec![
						WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: "Server".into(),
							tooltip: tooltip.clone(),
							..Default::default()
						})),
						WidgetHolder::new(Widget::Separator(Separator {
							separator_type: SeparatorType::Unrelated,
							direction: SeparatorDirection::Horizontal,
						})),
						WidgetHolder::new(Widget::IconButton(IconButton {
							size: 24,
							icon: "Settings".into(),
							tooltip: "Preferences: Imaginate".into(),
							on_update: WidgetCallback::new(|_| DialogMessage::RequestPreferencesDialog.into()),
							..Default::default()
						})),
						WidgetHolder::new(Widget::Separator(Separator {
							separator_type: SeparatorType::Related,
							direction: SeparatorDirection::Horizontal,
						})),
						WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: {
								match &persistent_data.imaginate_server_status {
									ImaginateServerStatus::Unknown => {
										responses.push_back(PortfolioMessage::ImaginateCheckServerStatus.into());
										"Checking...".into()
									}
									ImaginateServerStatus::Checking => "Checking...".into(),
									ImaginateServerStatus::Unavailable => "Unavailable".into(),
									ImaginateServerStatus::Connected => "Connected".into(),
								}
							},
							bold: true,
							tooltip,
							..Default::default()
						})),
						WidgetHolder::new(Widget::Separator(Separator {
							separator_type: SeparatorType::Related,
							direction: SeparatorDirection::Horizontal,
						})),
						WidgetHolder::new(Widget::IconButton(IconButton {
							size: 24,
							icon: "Reload".into(),
							tooltip: "Refresh connection status".into(),
							on_update: WidgetCallback::new(|_| PortfolioMessage::ImaginateCheckServerStatus.into()),
							..Default::default()
						})),
					]
				},
			},
			LayoutGroup::Row {
				widgets: {
					let tooltip = "When generating, the percentage represents how many sampling steps have so far been processed out of the target number".to_string();

					vec![
						WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: "Progress".into(),
							tooltip: tooltip.clone(),
							..Default::default()
						})),
						WidgetHolder::new(Widget::Separator(Separator {
							separator_type: SeparatorType::Unrelated,
							direction: SeparatorDirection::Horizontal,
						})),
						WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: {
								// Since we don't serialize the status, we need to derive from other state whether the Idle state is actually supposed to be the Terminated state
								let mut interpreted_status = imaginate_layer.status.clone();
								if imaginate_layer.status == ImaginateStatus::Idle
									&& imaginate_layer.blob_url.is_some()
									&& imaginate_layer.percent_complete > 0.
									&& imaginate_layer.percent_complete < 100.
								{
									interpreted_status = ImaginateStatus::Terminated;
								}

								match interpreted_status {
									ImaginateStatus::Idle => match imaginate_layer.blob_url {
										Some(_) => "Done".into(),
										None => "Ready".into(),
									},
									ImaginateStatus::Beginning => "Beginning...".into(),
									ImaginateStatus::Uploading(percent) => format!("Uploading Base Image: {:.0}%", percent),
									ImaginateStatus::Generating => format!("Generating: {:.0}%", imaginate_layer.percent_complete),
									ImaginateStatus::Terminating => "Terminating...".into(),
									ImaginateStatus::Terminated => format!("{:.0}% (Terminated)", imaginate_layer.percent_complete),
								}
							},
							bold: true,
							tooltip,
							..Default::default()
						})),
					]
				},
			},
			LayoutGroup::Row {
				widgets: [
					vec![
						WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: "Image".into(),
							tooltip: "Buttons that control the image generation process".into(),
							..Default::default()
						})),
						WidgetHolder::new(Widget::Separator(Separator {
							separator_type: SeparatorType::Unrelated,
							direction: SeparatorDirection::Horizontal,
						})),
					],
					{
						match imaginate_layer.status {
							ImaginateStatus::Beginning | ImaginateStatus::Uploading(_) => vec![WidgetHolder::new(Widget::TextButton(TextButton {
								label: "Beginning...".into(),
								tooltip: "Sending image generation request to the server".into(),
								disabled: true,
								..Default::default()
							}))],
							ImaginateStatus::Generating => vec![WidgetHolder::new(Widget::TextButton(TextButton {
								label: "Terminate".into(),
								tooltip: "Cancel the in-progress image generation and keep the latest progress".into(),
								on_update: WidgetCallback::new(|_| DocumentMessage::ImaginateTerminate.into()),
								..Default::default()
							}))],
							ImaginateStatus::Terminating => vec![WidgetHolder::new(Widget::TextButton(TextButton {
								label: "Terminating...".into(),
								tooltip: "Waiting on the final image generated after termination".into(),
								disabled: true,
								..Default::default()
							}))],
							ImaginateStatus::Idle | ImaginateStatus::Terminated => vec![
								WidgetHolder::new(Widget::IconButton(IconButton {
									size: 24,
									icon: "Random".into(),
									tooltip: "Generate with a random seed".into(),
									on_update: WidgetCallback::new(|_| PropertiesPanelMessage::SetImaginateSeedRandomizeAndGenerate.into()),
									..Default::default()
								})),
								WidgetHolder::new(Widget::Separator(Separator {
									separator_type: SeparatorType::Related,
									direction: SeparatorDirection::Horizontal,
								})),
								WidgetHolder::new(Widget::TextButton(TextButton {
									label: "Generate".into(),
									tooltip: "Fill layer frame by generating a new image".into(),
									on_update: WidgetCallback::new(|_| DocumentMessage::ImaginateGenerate.into()),
									..Default::default()
								})),
								WidgetHolder::new(Widget::Separator(Separator {
									separator_type: SeparatorType::Related,
									direction: SeparatorDirection::Horizontal,
								})),
								WidgetHolder::new(Widget::TextButton(TextButton {
									label: "Clear".into(),
									tooltip: "Remove generated image from the layer frame".into(),
									disabled: imaginate_layer.blob_url.is_none(),
									on_update: WidgetCallback::new(|_| DocumentMessage::FrameClear.into()),
									..Default::default()
								})),
							],
						}
					},
				]
				.concat(),
			},
			LayoutGroup::Row {
				widgets: {
					let tooltip = "Seed determines the random outcome, enabling limitless unique variations".to_string();

					vec![
						WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: "Seed".into(),
							tooltip: tooltip.clone(),
							..Default::default()
						})),
						WidgetHolder::new(Widget::Separator(Separator {
							separator_type: SeparatorType::Unrelated,
							direction: SeparatorDirection::Horizontal,
						})),
						WidgetHolder::new(Widget::IconButton(IconButton {
							size: 24,
							icon: "Regenerate".into(),
							tooltip: "Set a new random seed".into(),
							on_update: WidgetCallback::new(|_| PropertiesPanelMessage::SetImaginateSeedRandomize.into()),
							..Default::default()
						})),
						WidgetHolder::new(Widget::Separator(Separator {
							separator_type: SeparatorType::Related,
							direction: SeparatorDirection::Horizontal,
						})),
						WidgetHolder::new(Widget::NumberInput(NumberInput {
							value: Some(imaginate_layer.seed as f64),
							min: Some(-1.),
							tooltip,
							on_update: WidgetCallback::new(move |number_input: &NumberInput| {
								PropertiesPanelMessage::SetImaginateSeed {
									seed: number_input.value.unwrap().round() as u64,
								}
								.into()
							}),
							..Default::default()
						})),
					]
				},
			},
			LayoutGroup::Row {
				widgets: {
					let tooltip = "
					Width and height of the image that will be generated. Larger resolutions take longer to compute.\n\
					\n\
					512x512 yields optimal results because the AI is trained to understand that scale best. Larger sizes may tend to integrate the prompt's subject more than once. Small sizes are often incoherent. Put the layer in a folder and resize that to keep resolution unchanged.\n\
					\n\
					Dimensions must be a multiple of 64, so these are set by rounding the layer dimensions. A resolution exceeding 1 megapixel is reduced below that limit because larger sizes may exceed available GPU memory on the server.
					".trim().to_string();

					vec![
						WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: "Resolution".into(),
							tooltip: tooltip.clone(),
							..Default::default()
						})),
						WidgetHolder::new(Widget::Separator(Separator {
							separator_type: SeparatorType::Unrelated,
							direction: SeparatorDirection::Horizontal,
						})),
						WidgetHolder::new(Widget::IconButton(IconButton {
							size: 24,
							icon: "Rescale".into(),
							tooltip: "Set the layer scale to this resolution".into(),
							on_update: WidgetCallback::new(|_| PropertiesPanelMessage::SetImaginateScaleFromResolution.into()),
							..Default::default()
						})),
						WidgetHolder::new(Widget::Separator(Separator {
							separator_type: SeparatorType::Related,
							direction: SeparatorDirection::Horizontal,
						})),
						WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: {
								let (width, height) = pick_layer_safe_imaginate_resolution(layer, &persistent_data.font_cache);
								format!("{} W x {} H", width, height)
							},
							tooltip,
							bold: true,
							..Default::default()
						})),
					]
				},
			},
			LayoutGroup::Row {
				widgets: {
					let tooltip = "Number of iterations to improve the image generation quality, with diminishing returns around 40 when using the Euler A sampling method".to_string();
					vec![
						WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: "Sampling Steps".into(),
							tooltip: tooltip.clone(),
							..Default::default()
						})),
						WidgetHolder::new(Widget::Separator(Separator {
							separator_type: SeparatorType::Unrelated,
							direction: SeparatorDirection::Horizontal,
						})),
						WidgetHolder::new(Widget::NumberInput(NumberInput {
							value: Some(imaginate_layer.samples.into()),
							mode: NumberInputMode::Range,
							range_min: Some(0.),
							range_max: Some(150.),
							is_integer: true,
							min: Some(0.),
							max: Some(150.),
							tooltip,
							on_update: WidgetCallback::new(move |number_input: &NumberInput| {
								PropertiesPanelMessage::SetImaginateSamples {
									samples: number_input.value.unwrap().round() as u32,
								}
								.into()
							}),
							..Default::default()
						})),
					]
				},
			},
			LayoutGroup::Row {
				widgets: {
					let tooltip = "Algorithm used to generate the image during each sampling step".to_string();

					let sampling_methods = ImaginateSamplingMethod::list();
					let mut entries = Vec::with_capacity(sampling_methods.len());
					for method in sampling_methods {
						entries.push(DropdownEntryData {
							label: method.to_string(),
							on_update: WidgetCallback::new(move |_| PropertiesPanelMessage::SetImaginateSamplingMethod { method }.into()),
							..DropdownEntryData::default()
						});
					}
					let entries = vec![entries];

					vec![
						WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: "Sampling Method".into(),
							tooltip: tooltip.clone(),
							..Default::default()
						})),
						WidgetHolder::new(Widget::Separator(Separator {
							separator_type: SeparatorType::Unrelated,
							direction: SeparatorDirection::Horizontal,
						})),
						WidgetHolder::new(Widget::DropdownInput(DropdownInput {
							entries,
							selected_index: Some(imaginate_layer.sampling_method as u32),
							tooltip,
							..Default::default()
						})),
					]
				},
			},
			LayoutGroup::Row {
				widgets: {
					let tooltip = "Generate an image based upon the artwork beneath this frame in the containing folder".to_string();

					vec![
						WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: "Use Base Image".into(),
							tooltip: tooltip.clone(),
							..Default::default()
						})),
						WidgetHolder::new(Widget::Separator(Separator {
							separator_type: SeparatorType::Unrelated,
							direction: SeparatorDirection::Horizontal,
						})),
						WidgetHolder::new(Widget::CheckboxInput(CheckboxInput {
							checked: imaginate_layer.use_img2img,
							tooltip,
							on_update: WidgetCallback::new(move |checkbox_input: &CheckboxInput| PropertiesPanelMessage::SetImaginateUseImg2Img { use_img2img: checkbox_input.checked }.into()),
							..Default::default()
						})),
					]
				},
			},
			LayoutGroup::Row {
				widgets: {
					let tooltip = "
					Strength of the artistic liberties allowing changes from the base image. The image is unchanged at 0 and completely different at 1.\n\
					\n\
					This parameter is otherwise known as denoising strength.
					"
					.trim()
					.to_string();

					vec![
						WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: "Image Creativity".into(),
							tooltip: tooltip.clone(),
							..Default::default()
						})),
						WidgetHolder::new(Widget::Separator(Separator {
							separator_type: SeparatorType::Unrelated,
							direction: SeparatorDirection::Horizontal,
						})),
						WidgetHolder::new(Widget::NumberInput(NumberInput {
							value: Some(imaginate_layer.denoising_strength),
							mode: NumberInputMode::Range,
							range_min: Some(0.),
							range_max: Some(1.),
							min: Some(0.),
							max: Some(1.),
							display_decimal_places: 2,
							disabled: !imaginate_layer.use_img2img,
							tooltip,
							on_update: WidgetCallback::new(move |number_input: &NumberInput| {
								PropertiesPanelMessage::SetImaginateDenoisingStrength {
									denoising_strength: number_input.value.unwrap(),
								}
								.into()
							}),
							..Default::default()
						})),
					]
				},
			},
			LayoutGroup::Row {
				widgets: {
					let tooltip = "
					Amplification of the text prompt's influence over the outcome. At 0, the prompt is entirely ignored.\n\
					\n\
					Lower values are more creative and exploratory. Higher values are more literal and uninspired, but may be lower quality.\n\
					\n\
					This parameter is otherwise known as CFG (classifier-free guidance) scale.
					"
					.trim()
					.to_string();

					vec![
						WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: "Text Literalness".into(),
							tooltip: tooltip.to_string(),
							..Default::default()
						})),
						WidgetHolder::new(Widget::Separator(Separator {
							separator_type: SeparatorType::Unrelated,
							direction: SeparatorDirection::Horizontal,
						})),
						WidgetHolder::new(Widget::NumberInput(NumberInput {
							value: Some(imaginate_layer.cfg_scale),
							mode: NumberInputMode::Range,
							range_min: Some(0.),
							range_max: Some(30.),
							min: Some(0.),
							max: Some(30.),
							tooltip,
							on_update: WidgetCallback::new(move |number_input: &NumberInput| {
								PropertiesPanelMessage::SetImaginateCfgScale {
									cfg_scale: number_input.value.unwrap(),
								}
								.into()
							}),
							..Default::default()
						})),
					]
				},
			},
			LayoutGroup::Row {
				widgets: vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Text Prompt".into(),
						tooltip: "
						Description of the desired image subject and style.\n\
						\n\
						Include an artist name like \"Rembrandt\" or art medium like \"watercolor\" or \"photography\" to influence the look. List multiple to meld styles.\n\
						\n\
						To boost (or lessen) the importance of a word or phrase, wrap it in parentheses ending with a colon and a multiplier, for example:\n\
						\"Colorless green ideas (sleep:1.3) furiously\"
						"
						.trim()
						.into(),
						..Default::default()
					})),
					WidgetHolder::new(Widget::Separator(Separator {
						separator_type: SeparatorType::Unrelated,
						direction: SeparatorDirection::Horizontal,
					})),
					WidgetHolder::new(Widget::TextAreaInput(TextAreaInput {
						value: imaginate_layer.prompt.clone(),
						on_update: WidgetCallback::new(move |text_area_input: &TextAreaInput| {
							PropertiesPanelMessage::SetImaginatePrompt {
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
						value: "Neg. Prompt".into(),
						tooltip: "A negative text prompt can be used to list things like objects or colors to avoid".into(),
						..Default::default()
					})),
					WidgetHolder::new(Widget::Separator(Separator {
						separator_type: SeparatorType::Unrelated,
						direction: SeparatorDirection::Horizontal,
					})),
					WidgetHolder::new(Widget::TextAreaInput(TextAreaInput {
						value: imaginate_layer.negative_prompt.clone(),
						on_update: WidgetCallback::new(move |text_area_input: &TextAreaInput| {
							PropertiesPanelMessage::SetImaginateNegativePrompt {
								negative_prompt: text_area_input.value.clone(),
							}
							.into()
						}),
						..Default::default()
					})),
				],
			},
			LayoutGroup::Row {
				widgets: {
					let tooltip = "Postprocess human (or human-like) faces to look subtly less distorted".to_string();

					vec![
						WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: "Improve Faces".into(),
							tooltip: tooltip.clone(),
							..Default::default()
						})),
						WidgetHolder::new(Widget::Separator(Separator {
							separator_type: SeparatorType::Unrelated,
							direction: SeparatorDirection::Horizontal,
						})),
						WidgetHolder::new(Widget::CheckboxInput(CheckboxInput {
							checked: imaginate_layer.restore_faces,
							tooltip,
							on_update: WidgetCallback::new(move |checkbox_input: &CheckboxInput| {
								PropertiesPanelMessage::SetImaginateRestoreFaces {
									restore_faces: checkbox_input.checked,
								}
								.into()
							}),
							..Default::default()
						})),
					]
				},
			},
			LayoutGroup::Row {
				widgets: {
					let tooltip = "Generate the image so its edges loop seamlessly to make repeatable patterns or textures".to_string();

					vec![
						WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: "Tiling".into(),
							tooltip: tooltip.clone(),
							..Default::default()
						})),
						WidgetHolder::new(Widget::Separator(Separator {
							separator_type: SeparatorType::Unrelated,
							direction: SeparatorDirection::Horizontal,
						})),
						WidgetHolder::new(Widget::CheckboxInput(CheckboxInput {
							checked: imaginate_layer.tiling,
							tooltip,
							on_update: WidgetCallback::new(move |checkbox_input: &CheckboxInput| PropertiesPanelMessage::SetImaginateTiling { tiling: checkbox_input.checked }.into()),
							..Default::default()
						})),
					]
				},
			},
		],
	}
}

fn node_section_node_graph_frame(layer_path: Vec<graphene::LayerId>, node_graph_frame: &NodeGraphFrameLayer, open_graph: bool) -> LayoutGroup {
	LayoutGroup::Section {
		name: "Node Graph Frame".into(),
		layout: vec![
			LayoutGroup::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: "Temporary layer that applies a grayscale to the layers below it.".into(),
					..TextLabel::default()
				}))],
			},
			LayoutGroup::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: "Powered by the node graph! :)".into(),
					..TextLabel::default()
				}))],
			},
			LayoutGroup::Row {
				widgets: vec![WidgetHolder::new(Widget::TextButton(TextButton {
					label: if open_graph { "Close Node Graph".into() } else { "Open Node Graph".into() },
					tooltip: format!("{} the node graph associated with this layer", if open_graph { "Close" } else { "Open" }),
					on_update: WidgetCallback::new(move |_| {
						let layer_path = layer_path.clone();
						if open_graph {
							NodeGraphMessage::CloseNodeGraph.into()
						} else {
							NodeGraphMessage::OpenNodeGraph { layer_path }.into()
						}
					}),
					..Default::default()
				}))],
			},
			LayoutGroup::Row {
				widgets: vec![
					WidgetHolder::new(Widget::TextButton(TextButton {
						label: "Render".into(),
						tooltip: "Fill layer frame by rendering the node graph".into(),
						on_update: WidgetCallback::new(|_| DocumentMessage::NodeGraphFrameGenerate.into()),
						..Default::default()
					})),
					WidgetHolder::new(Widget::Separator(Separator {
						separator_type: SeparatorType::Related,
						direction: SeparatorDirection::Horizontal,
					})),
					WidgetHolder::new(Widget::TextButton(TextButton {
						label: "Clear".into(),
						tooltip: "Remove rendered node graph from the layer frame".into(),
						disabled: node_graph_frame.blob_url.is_none(),
						on_update: WidgetCallback::new(|_| DocumentMessage::FrameClear.into()),
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
						tooltip: "Linear gradient changes colors from one side to the other along a line".into(),
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
						tooltip: "Radial gradient changes colors from the inside to the outside of a circular area".into(),
						on_update: WidgetCallback::new(move |_| {
							PropertiesPanelMessage::ModifyFill {
								fill: Fill::Gradient(cloned_gradient_radial.clone()),
							}
							.into()
						}),
						..RadioEntryData::default()
					},
				],
				..Default::default()
			})),
		],
	}
}

fn node_gradient_color(gradient: &Gradient, position: usize) -> LayoutGroup {
	let gradient_clone = Rc::new(gradient.clone());
	let gradient_2 = gradient_clone.clone();
	let gradient_3 = gradient_clone.clone();
	let send_fill_message = move |new_gradient: Gradient| PropertiesPanelMessage::ModifyFill { fill: Fill::Gradient(new_gradient) }.into();

	let value = format!("Gradient: {:.0}%", gradient_clone.positions[position].0 * 100.);
	let mut widgets = vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
		value,
		tooltip: "Adjustable by dragging the gradient stops in the viewport with the Gradient tool active".into(),
		..TextLabel::default()
	}))];
	widgets.push(WidgetHolder::new(Widget::Separator(Separator {
		separator_type: SeparatorType::Unrelated,
		direction: SeparatorDirection::Horizontal,
	})));
	widgets.push(WidgetHolder::new(Widget::ColorInput(ColorInput {
		value: gradient_clone.positions[position].1,
		on_update: WidgetCallback::new(move |text_input: &ColorInput| {
			let mut new_gradient = (*gradient_clone).clone();
			new_gradient.positions[position].1 = text_input.value;
			send_fill_message(new_gradient)
		}),
		..ColorInput::default()
	})));

	let mut skip_separator = false;
	// Remove button
	if gradient.positions.len() != position + 1 && position != 0 {
		let on_update = WidgetCallback::new(move |_| {
			let mut new_gradient = (*gradient_3).clone();
			new_gradient.positions.remove(position);
			send_fill_message(new_gradient)
		});

		skip_separator = true;
		widgets.push(WidgetHolder::new(Widget::Separator(Separator {
			separator_type: SeparatorType::Related,
			direction: SeparatorDirection::Horizontal,
		})));
		widgets.push(WidgetHolder::new(Widget::IconButton(IconButton {
			icon: "Remove".to_string(),
			tooltip: "Remove this gradient stop".to_string(),
			size: 16,
			on_update,
			..Default::default()
		})));
	}
	// Add button
	if gradient.positions.len() != position + 1 {
		let on_update = WidgetCallback::new(move |_| {
			let mut gradient = (*gradient_2).clone();

			let get_color = |index: usize| match (gradient.positions[index].1, gradient.positions.get(index + 1).and_then(|x| x.1)) {
				(Some(a), Some(b)) => Color::from_rgbaf32((a.r() + b.r()) / 2., (a.g() + b.g()) / 2., (a.b() + b.b()) / 2., ((a.a() + b.a()) / 2.).clamp(0., 1.)),
				(Some(v), _) | (_, Some(v)) => Some(v),
				_ => Some(Color::WHITE),
			};
			let get_pos = |index: usize| (gradient.positions[index].0 + gradient.positions.get(index + 1).map(|v| v.0).unwrap_or(1.)) / 2.;

			gradient.positions.push((get_pos(position), get_color(position)));

			gradient.positions.sort_unstable_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

			send_fill_message(gradient)
		});

		if !skip_separator {
			widgets.push(WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Related,
				direction: SeparatorDirection::Horizontal,
			})));
		}
		widgets.push(WidgetHolder::new(Widget::IconButton(IconButton {
			icon: "Add".to_string(),
			tooltip: "Add a gradient stop after this".to_string(),
			size: 16,
			on_update,
			..Default::default()
		})));
	}
	LayoutGroup::Row { widgets }
}

fn node_section_fill(fill: &Fill) -> Option<LayoutGroup> {
	let initial_color = if let Fill::Solid(color) = fill { *color } else { Color::BLACK };

	match fill {
		Fill::Solid(_) | Fill::None => Some(LayoutGroup::Section {
			name: "Fill".into(),
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
							value: if let Fill::Solid(color) = fill { Some(*color) } else { None },
							on_update: WidgetCallback::new(|text_input: &ColorInput| {
								let fill = if let Some(value) = text_input.value { Fill::Solid(value) } else { Fill::None };
								PropertiesPanelMessage::ModifyFill { fill }.into()
							}),
							..ColorInput::default()
						})),
					],
				},
				LayoutGroup::Row {
					widgets: vec![
						WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: "".into(),
							..TextLabel::default()
						})),
						WidgetHolder::new(Widget::Separator(Separator {
							separator_type: SeparatorType::Unrelated,
							direction: SeparatorDirection::Horizontal,
						})),
						WidgetHolder::new(Widget::TextButton(TextButton {
							label: "Use Gradient".into(),
							tooltip: "Change this fill from a solid color to a gradient".into(),
							on_update: WidgetCallback::new(move |_: &TextButton| {
								let (r, g, b, _) = initial_color.components();
								let opposite_color = Color::from_rgbaf32(1. - r, 1. - g, 1. - b, 1.).unwrap();

								PropertiesPanelMessage::ModifyFill {
									fill: Fill::Gradient(Gradient::new(
										DVec2::new(0., 0.5),
										initial_color,
										DVec2::new(1., 0.5),
										opposite_color,
										DAffine2::IDENTITY,
										generate_uuid(),
										GradientType::Linear,
									)),
								}
								.into()
							}),
							..TextButton::default()
						})),
					],
				},
			],
		}),
		Fill::Gradient(gradient) => Some(LayoutGroup::Section {
			name: "Fill".into(),
			layout: {
				let cloned_gradient = gradient.clone();
				let first_color = gradient.positions.get(0).unwrap_or(&(0., None)).1;

				let mut layout = vec![node_gradient_type(gradient)];
				layout.extend((0..gradient.positions.len()).map(|pos| node_gradient_color(gradient, pos)));
				layout.push(LayoutGroup::Row {
					widgets: vec![
						WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: "".into(),
							..TextLabel::default()
						})),
						WidgetHolder::new(Widget::Separator(Separator {
							separator_type: SeparatorType::Unrelated,
							direction: SeparatorDirection::Horizontal,
						})),
						WidgetHolder::new(Widget::TextButton(TextButton {
							label: "Invert".into(),
							icon: Some("Swap".into()),
							tooltip: "Reverse the order of each color stop".into(),
							on_update: WidgetCallback::new(move |_: &TextButton| {
								let mut new_gradient = cloned_gradient.clone();
								new_gradient.positions = new_gradient.positions.iter().map(|(distance, color)| (1. - distance, *color)).collect();
								new_gradient.positions.reverse();
								PropertiesPanelMessage::ModifyFill { fill: Fill::Gradient(new_gradient) }.into()
							}),
							..TextButton::default()
						})),
					],
				});
				layout.push(LayoutGroup::Row {
					widgets: vec![
						WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: "".into(),
							..TextLabel::default()
						})),
						WidgetHolder::new(Widget::Separator(Separator {
							separator_type: SeparatorType::Unrelated,
							direction: SeparatorDirection::Horizontal,
						})),
						WidgetHolder::new(Widget::TextButton(TextButton {
							label: "Use Solid Color".into(),
							tooltip: "Change this fill from a gradient to a solid color, keeping the 0% stop color".into(),
							on_update: WidgetCallback::new(move |_: &TextButton| {
								PropertiesPanelMessage::ModifyFill {
									fill: Fill::Solid(first_color.unwrap_or_default()),
								}
								.into()
							}),
							..TextButton::default()
						})),
					],
				});
				layout
			},
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
						value: stroke.color(),
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
						value: Some(stroke.weight()),
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
						centered: true,
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
						value: Some(stroke.dash_offset()),
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
						..Default::default()
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
						..Default::default()
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
