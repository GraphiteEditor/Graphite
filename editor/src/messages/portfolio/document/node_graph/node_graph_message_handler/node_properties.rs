use crate::messages::layout::utility_types::layout_widget::*;
use crate::messages::layout::utility_types::widgets::{button_widgets::*, input_widgets::*};
use crate::messages::portfolio::utility_types::ImaginateServerStatus;
use crate::messages::prelude::*;

use glam::DVec2;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{generate_uuid, DocumentNode, NodeId, NodeInput};
use graph_craft::imaginate_input::*;
use graphene::layers::layer_info::LayerDataTypeDiscriminant;
use graphene::Operation;

use super::document_node_types::NodePropertiesContext;
use super::{FrontendGraphDataType, IMAGINATE_NODE};

pub fn string_properties(text: impl Into<String>) -> Vec<LayoutGroup> {
	let widget = WidgetHolder::text_widget(text);
	vec![LayoutGroup::Row { widgets: vec![widget] }]
}

fn update_value<T, F: Fn(&T) -> TaggedValue + 'static + Send + Sync>(value: F, node_id: NodeId, input_index: usize) -> WidgetCallback<T> {
	widget_callback!(move |input_value: &T| {
		NodeGraphMessage::SetInputValue {
			node: node_id,
			input_index,
			value: value(input_value),
		}
		.into()
	})
}

fn expose_widget(node_id: NodeId, index: usize, data_type: FrontendGraphDataType, exposed: bool) -> WidgetHolder {
	WidgetHolder::new(Widget::ParameterExposeButton(ParameterExposeButton {
		exposed,
		data_type,
		tooltip: "Expose this parameter input in node graph".into(),
		on_update: widget_callback!(move |_parameter| {
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

fn start_widgets(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, data_type: FrontendGraphDataType, blank_assist: bool) -> Vec<WidgetHolder> {
	let input = document_node.inputs.get(index).unwrap();
	let mut widgets = vec![
		expose_widget(node_id, index, data_type, input.is_exposed()),
		WidgetHolder::unrelated_separator(),
		WidgetHolder::text_widget(name),
	];
	if blank_assist {
		widgets.extend_from_slice(&[
			WidgetHolder::unrelated_separator(), // TODO: These three separators add up to 24px,
			WidgetHolder::unrelated_separator(), // TODO: which is the width of the Assist area.
			WidgetHolder::unrelated_separator(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
			WidgetHolder::unrelated_separator(), // TODO: This last one is the separator after the 24px assist.
		]);
	}
	widgets
}

fn text_widget(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool) -> Vec<WidgetHolder> {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::Text, blank_assist);

	if let NodeInput::Value {
		tagged_value: TaggedValue::String(x),
		exposed: false,
	} = &document_node.inputs[index]
	{
		widgets.extend_from_slice(&[
			WidgetHolder::unrelated_separator(),
			WidgetHolder::new(Widget::TextInput(TextInput {
				value: x.clone(),
				on_update: update_value(|x: &TextInput| TaggedValue::String(x.value.clone()), node_id, index),
				..TextInput::default()
			})),
		])
	}
	widgets
}

fn text_area_widget(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool) -> Vec<WidgetHolder> {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::Text, blank_assist);

	if let NodeInput::Value {
		tagged_value: TaggedValue::String(x),
		exposed: false,
	} = &document_node.inputs[index]
	{
		widgets.extend_from_slice(&[
			WidgetHolder::unrelated_separator(),
			WidgetHolder::new(Widget::TextAreaInput(TextAreaInput {
				value: x.clone(),
				on_update: update_value(|x: &TextAreaInput| TaggedValue::String(x.value.clone()), node_id, index),
				..TextAreaInput::default()
			})),
		])
	}
	widgets
}

fn bool_widget(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool) -> Vec<WidgetHolder> {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::Boolean, blank_assist);

	if let NodeInput::Value {
		tagged_value: TaggedValue::Bool(x),
		exposed: false,
	} = &document_node.inputs[index]
	{
		widgets.extend_from_slice(&[
			WidgetHolder::unrelated_separator(),
			WidgetHolder::new(Widget::CheckboxInput(CheckboxInput {
				checked: *x,
				on_update: update_value(|x: &CheckboxInput| TaggedValue::Bool(x.checked), node_id, index),
				..CheckboxInput::default()
			})),
		])
	}
	widgets
}

fn number_widget(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, number_props: NumberInput, blank_assist: bool) -> Vec<WidgetHolder> {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::Number, blank_assist);

	if let NodeInput::Value {
		tagged_value: TaggedValue::F64(x),
		exposed: false,
	} = document_node.inputs[index]
	{
		widgets.extend_from_slice(&[
			WidgetHolder::unrelated_separator(),
			WidgetHolder::new(Widget::NumberInput(NumberInput {
				value: Some(x),
				on_update: update_value(|x: &NumberInput| TaggedValue::F64(x.value.unwrap()), node_id, index),
				..number_props
			})),
		])
	}
	widgets
}

pub fn adjust_hsl_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let hue_shift = number_widget(document_node, node_id, 1, "Hue Shift", NumberInput::new().min(-180.).max(180.).unit("°"), true);
	let saturation_shift = number_widget(document_node, node_id, 2, "Saturation Shift", NumberInput::new().min(-100.).max(100.).unit("%"), true);
	let lightness_shift = number_widget(document_node, node_id, 3, "Lightness Shift", NumberInput::new().min(-100.).max(100.).unit("%"), true);

	vec![
		LayoutGroup::Row { widgets: hue_shift },
		LayoutGroup::Row { widgets: saturation_shift },
		LayoutGroup::Row { widgets: lightness_shift },
	]
}

pub fn brighten_image_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let brightness = number_widget(document_node, node_id, 1, "Brightness", NumberInput::new().min(-255.).max(255.), true);
	let contrast = number_widget(document_node, node_id, 2, "Contrast", NumberInput::new().min(-255.).max(255.), true);

	vec![LayoutGroup::Row { widgets: brightness }, LayoutGroup::Row { widgets: contrast }]
}

pub fn adjust_gamma_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let gamma = number_widget(document_node, node_id, 1, "Gamma", NumberInput::new().min(0.01), true);

	vec![LayoutGroup::Row { widgets: gamma }]
}

pub fn gpu_map_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let map = text_widget(document_node, node_id, 1, "Map", true);

	vec![LayoutGroup::Row { widgets: map }]
}

pub fn multiply_opacity(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let gamma = number_widget(document_node, node_id, 1, "Factor", NumberInput::new().min(0.).max(1.), true);

	vec![LayoutGroup::Row { widgets: gamma }]
}

pub fn posterize_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let value = number_widget(document_node, node_id, 1, "Levels", NumberInput::new().min(2.).max(255.).int(), true);

	vec![LayoutGroup::Row { widgets: value }]
}

pub fn exposure_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let value = number_widget(document_node, node_id, 1, "Value", NumberInput::new().min(-3.).max(3.), true);

	vec![LayoutGroup::Row { widgets: value }]
}

pub fn add_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let operand = |name: &str, index| {
		let widgets = number_widget(document_node, node_id, index, name, NumberInput::new(), true);

		LayoutGroup::Row { widgets }
	};
	vec![operand("Input", 0), operand("Addend", 1)]
}

pub fn _transform_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext, blank_assist: bool) -> Vec<LayoutGroup> {
	let translation = {
		let index = 1;

		let mut widgets = start_widgets(document_node, node_id, index, "Translation", FrontendGraphDataType::Vector, blank_assist);

		if let NodeInput::Value {
			tagged_value: TaggedValue::DVec2(vec2),
			exposed: false,
		} = document_node.inputs[index]
		{
			widgets.extend_from_slice(&[
				WidgetHolder::unrelated_separator(),
				WidgetHolder::new(Widget::NumberInput(NumberInput {
					value: Some(vec2.x),
					label: "X".into(),
					unit: " px".into(),
					on_update: update_value(move |number_input: &NumberInput| TaggedValue::DVec2(DVec2::new(number_input.value.unwrap(), vec2.y)), node_id, index),
					..NumberInput::default()
				})),
				WidgetHolder::unrelated_separator(),
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

		let mut widgets = start_widgets(document_node, node_id, index, "Rotation", FrontendGraphDataType::Number, blank_assist);

		if let NodeInput::Value {
			tagged_value: TaggedValue::F64(val),
			exposed: false,
		} = document_node.inputs[index]
		{
			widgets.extend_from_slice(&[
				WidgetHolder::unrelated_separator(),
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

		let mut widgets = start_widgets(document_node, node_id, index, "Scale", FrontendGraphDataType::Vector, blank_assist);

		if let NodeInput::Value {
			tagged_value: TaggedValue::DVec2(vec2),
			exposed: false,
		} = document_node.inputs[index]
		{
			widgets.extend_from_slice(&[
				WidgetHolder::unrelated_separator(),
				WidgetHolder::new(Widget::NumberInput(NumberInput {
					value: Some(vec2.x),
					label: "X".into(),
					unit: "".into(),
					on_update: update_value(move |number_input: &NumberInput| TaggedValue::DVec2(DVec2::new(number_input.value.unwrap(), vec2.y)), node_id, index),
					..NumberInput::default()
				})),
				WidgetHolder::unrelated_separator(),
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
	let imaginate_node = [context.nested_path, &[node_id]].concat();
	let imaginate_node_1 = imaginate_node.clone();
	let layer_path = context.layer_path.to_vec();

	let resolve_input = |name: &str| IMAGINATE_NODE.inputs.iter().position(|input| input.name == name).unwrap_or_else(|| panic!("Input {name} not found"));
	let seed_index = resolve_input("Seed");
	let resolution_index = resolve_input("Resolution");
	let samples_index = resolve_input("Samples");
	let sampling_method_index = resolve_input("Sampling Method");
	let text_guidance_index = resolve_input("Text Guidance");
	let text_index = resolve_input("Text Prompt");
	let neg_index = resolve_input("Negative Prompt");
	let base_img_index = resolve_input("Use Base Image");
	let img_creativity_index = resolve_input("Image Creativity");
	let mask_index = resolve_input("Masking Layer");
	let inpaint_index = resolve_input("Inpaint");
	let mask_blur_index = resolve_input("Mask Blur");
	let mask_fill_index = resolve_input("Mask Starting Fill");
	let faces_index = resolve_input("Improve Faces");
	let tiling_index = resolve_input("Tiling");
	let cached_index = resolve_input("Cached Data");

	let cached_value = &document_node.inputs[cached_index];
	let complete_value = &document_node.inputs[resolve_input("Percent Complete")];
	let status_value = &document_node.inputs[resolve_input("Status")];

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
			WidgetHolder::unrelated_separator(),
			WidgetHolder::new(Widget::IconButton(IconButton {
				size: 24,
				icon: "Settings".into(),
				tooltip: "Preferences: Imaginate".into(),
				on_update: widget_callback!(|_| DialogMessage::RequestPreferencesDialog.into()),
				..Default::default()
			})),
			WidgetHolder::unrelated_separator(),
			WidgetHolder::bold_text(status),
			WidgetHolder::related_separator(),
			WidgetHolder::new(Widget::IconButton(IconButton {
				size: 24,
				icon: "Reload".into(),
				tooltip: "Refresh connection status".into(),
				on_update: widget_callback!(|_| PortfolioMessage::ImaginateCheckServerStatus.into()),
				..Default::default()
			})),
		];
		LayoutGroup::Row { widgets }.with_tooltip("Connection status to the server that computes generated images")
	};

	let &NodeInput::Value {tagged_value: TaggedValue::ImaginateStatus( imaginate_status),..} = status_value else{
		panic!("Invalid status input")
	};
	let NodeInput::Value {tagged_value: TaggedValue::RcImage( cached_data),..} = cached_value else{
		panic!("Invalid cached image input")
	};
	let &NodeInput::Value {tagged_value: TaggedValue::F64( percent_complete),..} = complete_value else{
		panic!("Invalid percent complete input")
	};
	let use_base_image = if let &NodeInput::Value {
		tagged_value: TaggedValue::Bool(use_base_image),
		..
	} = &document_node.inputs[base_img_index]
	{
		use_base_image
	} else {
		true
	};
	let progress = {
		// Since we don't serialize the status, we need to derive from other state whether the Idle state is actually supposed to be the Terminated state
		let mut interpreted_status = imaginate_status;
		if imaginate_status == ImaginateStatus::Idle && cached_data.is_some() && percent_complete > 0. && percent_complete < 100. {
			interpreted_status = ImaginateStatus::Terminated;
		}

		let status = match interpreted_status {
			ImaginateStatus::Idle => match cached_data {
				Some(_) => "Done".into(),
				None => "Ready".into(),
			},
			ImaginateStatus::Beginning => "Beginning...".into(),
			ImaginateStatus::Uploading(percent) => format!("Uploading Base Image: {percent:.0}%"),
			ImaginateStatus::Generating => format!("Generating: {percent_complete:.0}%"),
			ImaginateStatus::Terminating => "Terminating...".into(),
			ImaginateStatus::Terminated => format!("{percent_complete:.0}% (Terminated)"),
		};
		let widgets = vec![
			WidgetHolder::text_widget("Progress"),
			WidgetHolder::unrelated_separator(),
			WidgetHolder::unrelated_separator(), // TODO: These three separators add up to 24px,
			WidgetHolder::unrelated_separator(), // TODO: which is the width of the Assist area.
			WidgetHolder::unrelated_separator(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
			WidgetHolder::unrelated_separator(),
			WidgetHolder::bold_text(status),
		];
		LayoutGroup::Row { widgets }.with_tooltip("When generating, the percentage represents how many sampling steps have so far been processed out of the target number")
	};

	let image_controls = {
		let mut widgets = vec![WidgetHolder::text_widget("Image"), WidgetHolder::unrelated_separator()];
		let assist_separators = vec![
			WidgetHolder::unrelated_separator(), // TODO: These three separators add up to 24px,
			WidgetHolder::unrelated_separator(), // TODO: which is the width of the Assist area.
			WidgetHolder::unrelated_separator(), // TODO: Remove these when we have proper entry row formatting that includes room for Assists.
			WidgetHolder::unrelated_separator(),
		];

		match imaginate_status {
			ImaginateStatus::Beginning | ImaginateStatus::Uploading(_) => {
				widgets.extend_from_slice(&assist_separators);
				widgets.push(WidgetHolder::new(Widget::TextButton(TextButton {
					label: "Beginning...".into(),
					tooltip: "Sending image generation request to the server".into(),
					disabled: true,
					..Default::default()
				})));
			}
			ImaginateStatus::Generating => {
				widgets.extend_from_slice(&assist_separators);
				widgets.push(WidgetHolder::new(Widget::TextButton(TextButton {
					label: "Terminate".into(),
					tooltip: "Cancel the in-progress image generation and keep the latest progress".into(),
					on_update: widget_callback!(move |_| {
						DocumentMessage::NodeGraphFrameImaginateTerminate {
							layer_path: layer_path.clone(),
							node_path: imaginate_node.clone(),
						}
						.into()
					}),
					..Default::default()
				})));
			}
			ImaginateStatus::Terminating => {
				widgets.extend_from_slice(&assist_separators);
				widgets.push(WidgetHolder::new(Widget::TextButton(TextButton {
					label: "Terminating...".into(),
					tooltip: "Waiting on the final image generated after termination".into(),
					disabled: true,
					..Default::default()
				})));
			}
			ImaginateStatus::Idle | ImaginateStatus::Terminated => widgets.extend_from_slice(&[
				WidgetHolder::new(Widget::IconButton(IconButton {
					size: 24,
					icon: "Random".into(),
					tooltip: "Generate with a new random seed".into(),
					on_update: widget_callback!(move |_| {
						DocumentMessage::NodeGraphFrameImaginateRandom {
							imaginate_node: imaginate_node.clone(),
						}
						.into()
					}),
					..Default::default()
				})),
				WidgetHolder::unrelated_separator(),
				WidgetHolder::new(Widget::TextButton(TextButton {
					label: "Generate".into(),
					tooltip: "Fill layer frame by generating a new image".into(),
					on_update: widget_callback!(move |_| {
						DocumentMessage::NodeGraphFrameImaginate {
							imaginate_node: imaginate_node_1.clone(),
						}
						.into()
					}),
					..Default::default()
				})),
				WidgetHolder::related_separator(),
				WidgetHolder::new(Widget::TextButton(TextButton {
					label: "Clear".into(),
					tooltip: "Remove generated image from the layer frame".into(),
					disabled: cached_data.is_none(),
					on_update: update_value(|_| TaggedValue::RcImage(None), node_id, cached_index),
					..Default::default()
				})),
			]),
		}
		LayoutGroup::Row { widgets }.with_tooltip("Buttons that control the image generation process")
	};

	// Requires custom layout for the regenerate button
	let seed = {
		let mut widgets = start_widgets(document_node, node_id, seed_index, "Seed", FrontendGraphDataType::Number, false);

		if let &NodeInput::Value {
			tagged_value: TaggedValue::F64(seed),
			exposed: false,
		} = &document_node.inputs[seed_index]
		{
			widgets.extend_from_slice(&[
				WidgetHolder::unrelated_separator(),
				WidgetHolder::new(Widget::IconButton(IconButton {
					size: 24,
					icon: "Regenerate".into(),
					tooltip: "Set a new random seed".into(),
					on_update: update_value(move |_| TaggedValue::F64((generate_uuid() >> 1) as f64), node_id, seed_index),
					..Default::default()
				})),
				WidgetHolder::unrelated_separator(),
				WidgetHolder::new(Widget::NumberInput(NumberInput {
					value: Some(seed),
					min: Some(0.),
					is_integer: true,
					on_update: update_value(move |input: &NumberInput| TaggedValue::F64(input.value.unwrap()), node_id, seed_index),
					..Default::default()
				})),
			])
		}
		// Note: Limited by f64. You cannot even have all the possible u64 values :)
		LayoutGroup::Row { widgets }.with_tooltip("Seed determines the random outcome, enabling limitless unique variations")
	};

	let resolution = {
		use graphene::document::pick_safe_imaginate_resolution;

		let mut widgets = start_widgets(document_node, node_id, resolution_index, "Resolution", FrontendGraphDataType::Vector, false);

		let round = |x: DVec2| {
			let (x, y) = pick_safe_imaginate_resolution(x.into());
			Some(DVec2::new(x as f64, y as f64))
		};

		if let &NodeInput::Value {
			tagged_value: TaggedValue::OptionalDVec2(vec2),
			exposed: false,
		} = &document_node.inputs[resolution_index]
		{
			let dimensions_is_auto = vec2.is_none();
			let vec2 = vec2.unwrap_or_else(|| {
				let transform = context.document.root.transform.inverse() * context.document.multiply_transforms(context.layer_path).unwrap();
				let w = transform.transform_vector2(DVec2::new(1., 0.)).length();
				let h = transform.transform_vector2(DVec2::new(0., 1.)).length();

				let (x, y) = pick_safe_imaginate_resolution((w, h));

				DVec2::new(x as f64, y as f64)
			});

			let layer_path = context.layer_path.to_vec();
			widgets.extend_from_slice(&[
				WidgetHolder::unrelated_separator(),
				WidgetHolder::new(Widget::IconButton(IconButton {
					size: 24,
					icon: "Rescale".into(),
					tooltip: "Set the Node Graph Frame layer dimensions to this resolution".into(),
					on_update: widget_callback!(move |_| {
						Operation::SetLayerScaleAroundPivot {
							path: layer_path.clone(),
							new_scale: vec2.into(),
						}
						.into()
					}),
					..Default::default()
				})),
				WidgetHolder::unrelated_separator(),
				WidgetHolder::new(Widget::CheckboxInput(CheckboxInput {
					checked: !dimensions_is_auto,
					icon: "Edit".into(),
					tooltip: "Set a custom resolution instead of using the frame's rounded dimensions".into(),
					on_update: update_value(
						move |checkbox_input: &CheckboxInput| {
							if checkbox_input.checked {
								TaggedValue::OptionalDVec2(Some(vec2))
							} else {
								TaggedValue::OptionalDVec2(None)
							}
						},
						node_id,
						resolution_index,
					),
					..CheckboxInput::default()
				})),
				WidgetHolder::unrelated_separator(),
				WidgetHolder::new(Widget::NumberInput(NumberInput {
					value: Some(vec2.x),
					label: "W".into(),
					unit: " px".into(),
					disabled: dimensions_is_auto,
					on_update: update_value(
						move |number_input: &NumberInput| TaggedValue::OptionalDVec2(round(DVec2::new(number_input.value.unwrap(), vec2.y))),
						node_id,
						resolution_index,
					),
					..NumberInput::default()
				})),
				WidgetHolder::related_separator(),
				WidgetHolder::new(Widget::NumberInput(NumberInput {
					value: Some(vec2.y),
					label: "H".into(),
					unit: " px".into(),
					disabled: dimensions_is_auto,
					on_update: update_value(
						move |number_input: &NumberInput| TaggedValue::OptionalDVec2(round(DVec2::new(vec2.x, number_input.value.unwrap()))),
						node_id,
						resolution_index,
					),
					..NumberInput::default()
				})),
			])
		}
		LayoutGroup::Row { widgets }.with_tooltip(
			"Width and height of the image that will be generated. Larger resolutions take longer to compute.\n\
			\n\
			512x512 yields optimal results because the AI is trained to understand that scale best. Larger sizes may tend to integrate the prompt's subject more than once. Small sizes are often incoherent.\n\
			\n\
			Dimensions must be a multiple of 64, so these are set by rounding the layer dimensions. A resolution exceeding 1 megapixel is reduced below that limit because larger sizes may exceed available GPU memory on the server.")
	};

	let sampling_steps = {
		let widgets = number_widget(document_node, node_id, samples_index, "Sampling Steps", NumberInput::new().min(0.).max(150.).int(), true);
		LayoutGroup::Row { widgets }.with_tooltip("Number of iterations to improve the image generation quality, with diminishing returns around 40 when using the Euler A sampling method")
	};

	let sampling_method = {
		let mut widgets = start_widgets(document_node, node_id, sampling_method_index, "Sampling Method", FrontendGraphDataType::General, true);

		if let &NodeInput::Value {
			tagged_value: TaggedValue::ImaginateSamplingMethod(sampling_method),
			exposed: false,
		} = &document_node.inputs[sampling_method_index]
		{
			let sampling_methods = ImaginateSamplingMethod::list();
			let mut entries = Vec::with_capacity(sampling_methods.len());
			for method in sampling_methods {
				entries.push(DropdownEntryData {
					label: method.to_string(),
					on_update: update_value(move |_| TaggedValue::ImaginateSamplingMethod(method), node_id, sampling_method_index),
					..DropdownEntryData::default()
				});
			}
			let entries = vec![entries];

			widgets.extend_from_slice(&[
				WidgetHolder::unrelated_separator(),
				WidgetHolder::new(Widget::DropdownInput(DropdownInput {
					entries,
					selected_index: Some(sampling_method as u32),
					..Default::default()
				})),
			]);
		}
		LayoutGroup::Row { widgets }.with_tooltip("Algorithm used to generate the image during each sampling step")
	};

	let text_guidance = {
		let widgets = number_widget(document_node, node_id, text_guidance_index, "Text Guidance", NumberInput::new().min(0.).max(30.), true);
		LayoutGroup::Row { widgets }.with_tooltip(
			"Amplification of the text prompt's influence over the outcome. At 0, the prompt is entirely ignored.\n\
			\n\
			Lower values are more creative and exploratory. Higher values are more literal and uninspired, but may be lower quality.\n\
			\n\
			This parameter is otherwise known as CFG (classifier-free guidance).",
		)
	};

	let text_prompt = {
		let widgets = text_area_widget(document_node, node_id, text_index, "Text Prompt", true);
		LayoutGroup::Row { widgets }.with_tooltip(
			"Description of the desired image subject and style.\n\
			\n\
			Include an artist name like \"Rembrandt\" or art medium like \"watercolor\" or \"photography\" to influence the look. List multiple to meld styles.\n\
			\n\
			To boost (or lessen) the importance of a word or phrase, wrap it in parentheses ending with a colon and a multiplier, for example:\n\
			\"Colorless green ideas (sleep:1.3) furiously\"",
		)
	};
	let negative_prompt = {
		let widgets = text_area_widget(document_node, node_id, neg_index, "Negative Prompt", true);
		LayoutGroup::Row { widgets }.with_tooltip("A negative text prompt can be used to list things like objects or colors to avoid")
	};
	let base_image = {
		let widgets = bool_widget(document_node, node_id, base_img_index, "Use Base Image", true);
		LayoutGroup::Row { widgets }.with_tooltip("Generate an image based upon some raster data")
	};
	let image_creativity = {
		let props = NumberInput::new().percentage().disabled(!use_base_image);
		let widgets = number_widget(document_node, node_id, img_creativity_index, "Image Creativity", props, true);
		LayoutGroup::Row { widgets }.with_tooltip(
			"Strength of the artistic liberties allowing changes from the base image. The image is unchanged at 0% and completely different at 100%.\n\
			\n\
			This parameter is otherwise known as denoising strength.",
		)
	};

	let mut layer_reference_input_layer_is_some = false;
	let layer_mask = {
		let mut widgets = start_widgets(document_node, node_id, mask_index, "Masking Layer", FrontendGraphDataType::General, true);

		if let NodeInput::Value {
			tagged_value: TaggedValue::LayerPath(layer_path),
			exposed: false,
		} = &document_node.inputs[mask_index]
		{
			let layer_reference_input_layer = layer_path
				.as_ref()
				.and_then(|path| context.document.layer(path).ok())
				.map(|layer| (layer.name.clone().unwrap_or_default(), LayerDataTypeDiscriminant::from(&layer.data)));

			layer_reference_input_layer_is_some = layer_reference_input_layer.is_some();

			let layer_reference_input_layer_name = layer_reference_input_layer.as_ref().map(|(layer_name, _)| layer_name);
			let layer_reference_input_layer_type = layer_reference_input_layer.as_ref().map(|(_, layer_type)| layer_type);

			widgets.extend_from_slice(&[
				WidgetHolder::unrelated_separator(),
				WidgetHolder::new(Widget::LayerReferenceInput(LayerReferenceInput {
					value: layer_path.clone(),
					layer_name: layer_reference_input_layer_name.cloned(),
					layer_type: layer_reference_input_layer_type.cloned(),
					disabled: !use_base_image,
					on_update: update_value(|input: &LayerReferenceInput| TaggedValue::LayerPath(input.value.clone()), node_id, mask_index),
					..Default::default()
				})),
			]);
		}
		LayoutGroup::Row { widgets }.with_tooltip(
			"Reference to a layer or folder which masks parts of the base image. Image generation is constrained to masked areas.\n\
			\n\
			Black shapes represent the masked regions. Lighter shades of gray act as a partial mask, and colors become grayscale.",
		)
	};

	let mut layout = vec![
		server_status,
		progress,
		image_controls,
		seed,
		resolution,
		sampling_steps,
		sampling_method,
		text_guidance,
		text_prompt,
		negative_prompt,
		base_image,
		image_creativity,
		layer_mask,
	];

	if use_base_image && layer_reference_input_layer_is_some {
		let in_paint = {
			let mut widgets = start_widgets(document_node, node_id, inpaint_index, "Inpaint", FrontendGraphDataType::Boolean, true);

			if let &NodeInput::Value {
				tagged_value: TaggedValue::Bool(in_paint),
				exposed: false,
			} = &document_node.inputs[inpaint_index]
			{
				widgets.extend_from_slice(&[
					WidgetHolder::unrelated_separator(),
					WidgetHolder::new(Widget::RadioInput(RadioInput {
						entries: [(true, "Inpaint"), (false, "Outpaint")]
							.into_iter()
							.map(|(paint, name)| RadioEntryData {
								label: name.to_string(),
								on_update: update_value(move |_| TaggedValue::Bool(paint), node_id, inpaint_index),
								..Default::default()
							})
							.collect(),
						selected_index: 1 - in_paint as u32,
						..Default::default()
					})),
				]);
			}
			LayoutGroup::Row { widgets }.with_tooltip(
				"Constrain image generation to the interior (inpaint) or exterior (outpaint) of the mask, while referencing the other unchanged parts as context imagery.\n\
				\n\
				An unwanted part of an image can be replaced by drawing around it with a black shape and inpainting with that mask layer.\n\
				\n\
				An image can be uncropped by resizing the Imaginate layer to the target bounds and outpainting with a black rectangle mask matching the original image bounds.",
			)
		};

		let blur_radius = {
			let widgets = number_widget(document_node, node_id, mask_blur_index, "Mask Blur", NumberInput::new().unit(" px").min(0.).max(25.).int(), true);
			LayoutGroup::Row { widgets }.with_tooltip("Blur radius for the mask. Useful for softening sharp edges to blend the masked area with the rest of the image.")
		};

		let mask_starting_fill = {
			let mut widgets = start_widgets(document_node, node_id, mask_fill_index, "Mask Starting Fill", FrontendGraphDataType::General, true);

			if let &NodeInput::Value {
				tagged_value: TaggedValue::ImaginateMaskStartingFill(starting_fill),
				exposed: false,
			} = &document_node.inputs[mask_fill_index]
			{
				let mask_fill_content_modes = ImaginateMaskStartingFill::list();
				let mut entries = Vec::with_capacity(mask_fill_content_modes.len());
				for mode in mask_fill_content_modes {
					entries.push(DropdownEntryData {
						label: mode.to_string(),
						on_update: update_value(move |_| TaggedValue::ImaginateMaskStartingFill(mode), node_id, mask_fill_index),
						..DropdownEntryData::default()
					});
				}
				let entries = vec![entries];

				widgets.extend_from_slice(&[
					WidgetHolder::unrelated_separator(),
					WidgetHolder::new(Widget::DropdownInput(DropdownInput {
						entries,
						selected_index: Some(starting_fill as u32),
						..Default::default()
					})),
				]);
			}
			LayoutGroup::Row { widgets }.with_tooltip(
				"Begin in/outpainting the masked areas using this fill content as the starting base image.\n\
				\n\
				Each option can be visualized by generating with 'Sampling Steps' set to 0.",
			)
		};
		layout.extend_from_slice(&[in_paint, blur_radius, mask_starting_fill]);
	}

	let improve_faces = {
		let widgets = bool_widget(document_node, node_id, faces_index, "Improve Faces", true);
		LayoutGroup::Row { widgets }.with_tooltip(
			"Postprocess human (or human-like) faces to look subtly less distorted.\n\
			\n\
			This filter can be used on its own by enabling 'Use Base Image' and setting 'Sampling Steps' to 0.",
		)
	};
	let tiling = {
		let widgets = bool_widget(document_node, node_id, tiling_index, "Tiling", true);
		LayoutGroup::Row { widgets }.with_tooltip("Generate the image so its edges loop seamlessly to make repeatable patterns or textures")
	};
	layout.extend_from_slice(&[improve_faces, tiling]);

	layout
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
