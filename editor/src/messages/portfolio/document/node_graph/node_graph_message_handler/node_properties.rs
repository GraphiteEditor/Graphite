use crate::messages::layout::utility_types::layout_widget::*;
use crate::messages::layout::utility_types::widgets::{button_widgets::*, input_widgets::*, label_widgets::*};
use crate::messages::portfolio::utility_types::ImaginateServerStatus;
use crate::messages::prelude::*;

use glam::DVec2;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, NodeId, NodeInput};
use graph_craft::imaginate_input::*;

use super::document_node_types::NodePropertiesContext;
use super::FrontendGraphDataType;

pub fn string_properties(text: impl Into<String>) -> Vec<LayoutGroup> {
	let widget = WidgetHolder::text_widget(text);
	vec![LayoutGroup::Row { widgets: vec![widget] }]
}

fn update_value<T, F: Fn(&T) -> TaggedValue + 'static + Send + Sync>(value: F, node_id: NodeId, input_index: usize) -> WidgetCallback<T> {
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

fn text_widget(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str) -> Vec<WidgetHolder> {
	let input: &NodeInput = document_node.inputs.get(index).unwrap();

	let mut widgets = vec![
		expose_widget(node_id, index, FrontendGraphDataType::Number, input.is_exposed()),
		WidgetHolder::unrelated_seperator(),
		WidgetHolder::text_widget(name),
	];

	if let NodeInput::Value {
		tagged_value: TaggedValue::String(x),
		exposed: false,
	} = &document_node.inputs[index]
	{
		widgets.extend_from_slice(&[
			WidgetHolder::unrelated_seperator(),
			WidgetHolder::new(Widget::TextInput(TextInput {
				value: x.clone(),
				on_update: update_value(|x: &TextInput| TaggedValue::String(x.value.clone()), node_id, index),
				..TextInput::default()
			})),
		])
	}
	widgets
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

pub fn adjust_hsl_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let hue_shift = number_range_widget(document_node, node_id, 1, "Hue Shift", Some(-180.), Some(180.), "°".into(), false);
	let saturation_shift = number_range_widget(document_node, node_id, 2, "Saturation Shift", Some(-100.), Some(100.), "%".into(), false);
	let lightness_shift = number_range_widget(document_node, node_id, 3, "Lightness Shift", Some(-100.), Some(100.), "%".into(), false);

	vec![
		LayoutGroup::Row { widgets: hue_shift },
		LayoutGroup::Row { widgets: saturation_shift },
		LayoutGroup::Row { widgets: lightness_shift },
	]
}

pub fn brighten_image_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let brightness = number_range_widget(document_node, node_id, 1, "Brightness", Some(-255.), Some(255.), "".into(), false);
	let contrast = number_range_widget(document_node, node_id, 2, "Contrast", Some(-255.), Some(255.), "".into(), false);

	vec![LayoutGroup::Row { widgets: brightness }, LayoutGroup::Row { widgets: contrast }]
}

pub fn adjust_gamma_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let gamma = number_range_widget(document_node, node_id, 1, "Gamma", Some(0.01), None, "".into(), false);

	vec![LayoutGroup::Row { widgets: gamma }]
}

pub fn gpu_map_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let map = text_widget(document_node, node_id, 1, "Map");

	vec![LayoutGroup::Row { widgets: map }]
}

pub fn multiply_opacity(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let gamma = number_range_widget(document_node, node_id, 1, "Factor", Some(0.), Some(1.), "".into(), false);

	vec![LayoutGroup::Row { widgets: gamma }]
}

pub fn posterize_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let value = number_range_widget(document_node, node_id, 1, "Levels", Some(2.), Some(255.), "".into(), true);

	vec![LayoutGroup::Row { widgets: value }]
}

pub fn exposure_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let value = number_range_widget(document_node, node_id, 1, "Value", Some(-3.), Some(3.), "".into(), false);

	vec![LayoutGroup::Row { widgets: value }]
}

pub fn add_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let operand = |name: &str, index| {
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
					mode: NumberInputMode::Increment,
					on_update: update_value(|number_input: &NumberInput| TaggedValue::F64(number_input.value.unwrap()), node_id, index),
					..NumberInput::default()
				})),
			]);
		}

		LayoutGroup::Row { widgets }
	};
	vec![operand("Input", 0), operand("Addend", 1)]
}

pub fn transform_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
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

pub fn imaginate_properties(document_node: &DocumentNode, node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let server_status = {
		let status = match &context.persistent_data.imaginate_server_status {
			ImaginateServerStatus::Unknown => {
				context.responses.push_back(PortfolioMessage::ImaginateCheckServerStatus.into());
				"Checking..."
			}
			ImaginateServerStatus::Checking => "Checking...",
			ImaginateServerStatus::Unavailable => "Unavailable",
			ImaginateServerStatus::Connected => "Connected",
		};
		let widgets = vec![
			WidgetHolder::text_widget("Server"),
			WidgetHolder::unrelated_seperator(),
			WidgetHolder::new(Widget::IconButton(IconButton {
				size: 24,
				icon: "Settings".into(),
				tooltip: "Preferences: Imaginate".into(),
				on_update: WidgetCallback::new(|_| DialogMessage::RequestPreferencesDialog.into()),
				..Default::default()
			})),
			WidgetHolder::related_seperator(),
			WidgetHolder::bold_text(status),
			WidgetHolder::related_seperator(),
			WidgetHolder::new(Widget::IconButton(IconButton {
				size: 24,
				icon: "Reload".into(),
				tooltip: "Refresh connection status".into(),
				on_update: WidgetCallback::new(|_| PortfolioMessage::ImaginateCheckServerStatus.into()),
				..Default::default()
			})),
		];
		LayoutGroup::Row { widgets }.with_tooltip("Connection status to the server that computes generated images")
	};

	let progress = {
		let widgets = vec![
			WidgetHolder::text_widget("Progress"),
			WidgetHolder::unrelated_seperator(),
			WidgetHolder::bold_text({
				// // Since we don't serialize the status, we need to derive from other state whether the Idle state is actually supposed to be the Terminated state
				// let mut interpreted_status = imaginate_layer.status.clone();
				// if imaginate_layer.status == ImaginateStatus::Idle && imaginate_layer.blob_url.is_some() && imaginate_layer.percent_complete > 0. && imaginate_layer.percent_complete < 100. {
				// 	interpreted_status = ImaginateStatus::Terminated;
				// }

				// match interpreted_status {
				// 	ImaginateStatus::Idle => match imaginate_layer.blob_url {
				// 		Some(_) => "Done".into(),
				// 		None => "Ready".into(),
				// 	},
				// 	ImaginateStatus::Beginning => "Beginning...".into(),
				// 	ImaginateStatus::Uploading(percent) => format!("Uploading Base Image: {:.0}%", percent),
				// 	ImaginateStatus::Generating => format!("Generating: {:.0}%", imaginate_layer.percent_complete),
				// 	ImaginateStatus::Terminating => "Terminating...".into(),
				// 	ImaginateStatus::Terminated => format!("{:.0}% (Terminated)", imaginate_layer.percent_complete),
				// }
				"hi"
			}),
		];
		LayoutGroup::Row { widgets }.with_tooltip("When generating, the percentage represents how many sampling steps have so far been processed out of the target number")
	};

	/*let layer_reference_input_layer = imaginate_layer
		.mask_layer_ref
		.as_ref()
		.and_then(|path| document.layer(path).ok())
		.map(|layer| (layer.name.clone().unwrap_or_default(), LayerDataTypeDiscriminant::from(&layer.data)));

	let layer_reference_input_layer_is_some = layer_reference_input_layer.is_some();

	let layer_reference_input_layer_name = layer_reference_input_layer.as_ref().map(|(layer_name, _)| layer_name);
	let layer_reference_input_layer_type = layer_reference_input_layer.as_ref().map(|(_, layer_type)| layer_type);

	let mut layout = vec![


		LayoutGroup::Row {
			widgets: [
				vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Image".into(),
						tooltip: "Buttons that control the image generation process".into(),
						..Default::default()
					})),
					WidgetHolder::unrelated_seperator(),
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
								tooltip: "Generate with a new random seed".into(),
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
					WidgetHolder::unrelated_seperator(),
					WidgetHolder::new(Widget::IconButton(IconButton {
						size: 24,
						icon: "Regenerate".into(),
						tooltip: "Set a new random seed".into(),
						on_update: WidgetCallback::new(|_| PropertiesPanelMessage::SetImaginateSeedRandomize.into()),
						..Default::default()
					})),
					WidgetHolder::unrelated_seperator(),
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
					WidgetHolder::unrelated_seperator(),
					WidgetHolder::new(Widget::IconButton(IconButton {
						size: 24,
						icon: "Rescale".into(),
						tooltip: "Set the layer scale to this resolution".into(),
						on_update: WidgetCallback::new(|_| PropertiesPanelMessage::SetImaginateScaleFromResolution.into()),
						..Default::default()
					})),
					WidgetHolder::unrelated_seperator(),
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
					WidgetHolder::unrelated_seperator(),
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
					WidgetHolder::unrelated_seperator(),
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
				let tooltip = "
					Amplification of the text prompt's influence over the outcome. At 0, the prompt is entirely ignored.\n\
					\n\
					Lower values are more creative and exploratory. Higher values are more literal and uninspired, but may be lower quality.\n\
					\n\
					This parameter is otherwise known as CFG (classifier-free guidance).
					"
				.trim()
				.to_string();

				vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Text Guidance".into(),
						tooltip: tooltip.to_string(),
						..Default::default()
					})),
					WidgetHolder::unrelated_seperator(),
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
				let tooltip = "Generate an image based upon the artwork beneath this frame in the containing folder".to_string();

				vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Use Base Image".into(),
						tooltip: tooltip.clone(),
						..Default::default()
					})),
					WidgetHolder::unrelated_seperator(),
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
				Strength of the artistic liberties allowing changes from the base image. The image is unchanged at 0% and completely different at 100%.\n\
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
					WidgetHolder::unrelated_seperator(),
					WidgetHolder::new(Widget::NumberInput(NumberInput {
						value: Some(imaginate_layer.denoising_strength * 100.),
						unit: "%".into(),
						mode: NumberInputMode::Range,
						range_min: Some(0.),
						range_max: Some(100.),
						min: Some(0.),
						max: Some(100.),
						display_decimal_places: 2,
						disabled: !imaginate_layer.use_img2img,
						tooltip,
						on_update: WidgetCallback::new(move |number_input: &NumberInput| {
							PropertiesPanelMessage::SetImaginateDenoisingStrength {
								denoising_strength: number_input.value.unwrap() / 100.,
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
				Reference to a layer or folder which masks parts of the base image. Image generation is constrained to masked areas.\n\
				\n\
				Black shapes represent the masked regions. Lighter shades of gray act as a partial mask, and colors become grayscale.
				"
				.trim()
				.to_string();

				vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Masking Layer".into(),
						tooltip: tooltip.clone(),
						..Default::default()
					})),
					WidgetHolder::unrelated_seperator(),
					WidgetHolder::new(Widget::LayerReferenceInput(LayerReferenceInput {
						value: imaginate_layer.mask_layer_ref.clone(),
						tooltip,
						layer_name: layer_reference_input_layer_name.cloned(),
						layer_type: layer_reference_input_layer_type.cloned(),
						disabled: !imaginate_layer.use_img2img,
						on_update: WidgetCallback::new(move |val: &LayerReferenceInput| PropertiesPanelMessage::SetImaginateLayerPath { layer_path: val.value.clone() }.into()),
						..Default::default()
					})),
				]
			},
		},
	];

	if imaginate_layer.use_img2img && imaginate_layer.mask_layer_ref.is_some() && layer_reference_input_layer_is_some {
		layout.extend(vec![
			LayoutGroup::Row {
				widgets: {
					let tooltip = "
					Constrain image generation to the interior (inpaint) or exterior (outpaint) of the mask, while referencing the other unchanged parts as context imagery.\n\
					\n\
					An unwanted part of an image can be replaced by drawing around it with a black shape and inpainting with that mask layer.\n\
					\n\
					An image can be uncropped by resizing the Imaginate layer to the target bounds and outpainting with a black rectangle mask matching the original image bounds.
					"
					.trim()
					.to_string();

					vec![
						WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: "Mask Direction".to_string(),
							tooltip: tooltip.clone(),
							..Default::default()
						})),
						WidgetHolder::new(Widget::Separator(Separator {
							separator_type: SeparatorType::Unrelated,
							direction: SeparatorDirection::Horizontal,
						})),
						WidgetHolder::new(Widget::RadioInput(RadioInput {
							entries: [(ImaginateMaskPaintMode::Inpaint, "Inpaint"), (ImaginateMaskPaintMode::Outpaint, "Outpaint")]
								.into_iter()
								.map(|(paint, name)| RadioEntryData {
									label: name.to_string(),
									on_update: WidgetCallback::new(move |_| PropertiesPanelMessage::SetImaginateMaskPaintMode { paint }.into()),
									tooltip: tooltip.clone(),
									..Default::default()
								})
								.collect(),
							selected_index: imaginate_layer.mask_paint_mode as u32,
							..Default::default()
						})),
					]
				},
			},
			LayoutGroup::Row {
				widgets: {
					let tooltip = "Blur radius for the mask. Useful for softening sharp edges to blend the masked area with the rest of the image.".to_string();

					vec![
						WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: "Mask Blur".to_string(),
							tooltip: tooltip.clone(),
							..Default::default()
						})),
						WidgetHolder::new(Widget::Separator(Separator {
							separator_type: SeparatorType::Unrelated,
							direction: SeparatorDirection::Horizontal,
						})),
						WidgetHolder::new(Widget::NumberInput(NumberInput {
							value: Some(imaginate_layer.mask_blur_px as f64),
							unit: " px".into(),
							mode: NumberInputMode::Range,
							range_min: Some(0.),
							range_max: Some(25.),
							min: Some(0.),
							is_integer: true,
							tooltip,
							on_update: WidgetCallback::new(move |number_input: &NumberInput| {
								PropertiesPanelMessage::SetImaginateMaskBlurPx {
									mask_blur_px: number_input.value.unwrap() as u32,
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
					Begin in/outpainting the masked areas using this fill content as the starting base image.\n\
					\n\
					Each option can be visualized by generating with 'Sampling Steps' set to 0.
					"
					.trim()
					.to_string();

					let mask_fill_content_modes = ImaginateMaskFillContent::list();
					let mut entries = Vec::with_capacity(mask_fill_content_modes.len());
					for mode in mask_fill_content_modes {
						entries.push(DropdownEntryData {
							label: mode.to_string(),
							on_update: WidgetCallback::new(move |_| PropertiesPanelMessage::SetImaginateMaskFillContent { mode }.into()),
							..DropdownEntryData::default()
						});
					}
					let entries = vec![entries];

					vec![
						WidgetHolder::new(Widget::TextLabel(TextLabel {
							value: "Mask Starting Fill".to_string(),
							tooltip: tooltip.clone(),
							..Default::default()
						})),
						WidgetHolder::new(Widget::Separator(Separator {
							separator_type: SeparatorType::Unrelated,
							direction: SeparatorDirection::Horizontal,
						})),
						WidgetHolder::new(Widget::DropdownInput(DropdownInput {
							entries,
							selected_index: Some(imaginate_layer.mask_fill_content as u32),
							tooltip,
							..Default::default()
						})),
					]
				},
			},
		]);
	}

	layout.extend(vec![
		LayoutGroup::Row {
			widgets: {
				let tooltip = "
				Postprocess human (or human-like) faces to look subtly less distorted.\n\
				\n\
				This filter can be used on its own by enabling 'Use Base Image' and setting 'Sampling Steps' to 0.
				"
				.to_string();

				vec![
					WidgetHolder::new(Widget::TextLabel(TextLabel {
						value: "Improve Faces".into(),
						tooltip: tooltip.clone(),
						..Default::default()
					})),
					WidgetHolder::unrelated_seperator(),
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
					WidgetHolder::unrelated_seperator(),
					WidgetHolder::new(Widget::CheckboxInput(CheckboxInput {
						checked: imaginate_layer.tiling,
						tooltip,
						on_update: WidgetCallback::new(move |checkbox_input: &CheckboxInput| PropertiesPanelMessage::SetImaginateTiling { tiling: checkbox_input.checked }.into()),
						..Default::default()
					})),
				]
			},
		},
	]);

	LayoutGroup::Section { name: "Imaginate".into(), layout }*/
	vec![server_status]
}

fn unknown_node_properties(document_node: &DocumentNode) -> Vec<LayoutGroup> {
	string_properties(format!("Node '{}' cannot be found in library", document_node.name))
}

pub fn no_properties(_document_node: &DocumentNode, _node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	string_properties("Node has no properties")
}

pub fn generate_node_properties(document_node: &DocumentNode, node_id: NodeId, context: &mut NodePropertiesContext) -> LayoutGroup {
	let name = document_node.name.clone();
	let layout = match super::document_node_types::resolve_document_node_type(&name) {
		Some(document_node_type) => (document_node_type.properties)(document_node, node_id, context),
		None => unknown_node_properties(document_node),
	};
	LayoutGroup::Section { name, layout }
}
