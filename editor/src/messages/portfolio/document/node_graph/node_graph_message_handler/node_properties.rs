use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::utility_types::ImaginateServerStatus;
use crate::messages::prelude::*;

use document_legacy::layers::layer_info::LayerDataTypeDiscriminant;
use document_legacy::Operation;
use glam::DVec2;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{generate_uuid, DocumentNode, NodeId, NodeInput};
use graph_craft::imaginate_input::*;

use super::document_node_types::NodePropertiesContext;
use super::{FrontendGraphDataType, IMAGINATE_NODE};

pub fn string_properties(text: impl Into<String>) -> Vec<LayoutGroup> {
	let widget = WidgetHolder::text_widget(text);
	vec![LayoutGroup::Row { widgets: vec![widget] }]
}

fn update_value<T>(value: impl Fn(&T) -> TaggedValue + 'static + Send + Sync, node_id: NodeId, input_index: usize) -> impl Fn(&T) -> Message + 'static + Send + Sync {
	move |input_value: &T| {
		NodeGraphMessage::SetInputValue {
			node_id,
			input_index,
			value: value(input_value),
		}
		.into()
	}
}

fn expose_widget(node_id: NodeId, index: usize, data_type: FrontendGraphDataType, exposed: bool) -> WidgetHolder {
	ParameterExposeButton::new()
		.exposed(exposed)
		.data_type(data_type)
		.tooltip("Expose this parameter input in node graph")
		.on_update(move |_parameter| {
			NodeGraphMessage::ExposeInput {
				node_id,
				input_index: index,
				new_exposed: !exposed,
			}
			.into()
		})
		.widget_holder()
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

// fn text_widget(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool) -> Vec<WidgetHolder> {
// 	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::Text, blank_assist);

// 	if let NodeInput::Value {
// 		tagged_value: TaggedValue::String(x),
// 		exposed: false,
// 	} = &document_node.inputs[index]
// 	{
// 		widgets.extend_from_slice(&[
// 			WidgetHolder::unrelated_separator(),
// 			TextInput::new(x.clone())
// 				.on_update(update_value(|x: &TextInput| TaggedValue::String(x.value.clone()), node_id, index))
// 				.widget_holder(),
// 		])
// 	}
// 	widgets
// }

fn text_area_widget(document_node: &DocumentNode, node_id: NodeId, index: usize, name: &str, blank_assist: bool) -> Vec<WidgetHolder> {
	let mut widgets = start_widgets(document_node, node_id, index, name, FrontendGraphDataType::Text, blank_assist);

	if let NodeInput::Value {
		tagged_value: TaggedValue::String(x),
		exposed: false,
	} = &document_node.inputs[index]
	{
		widgets.extend_from_slice(&[
			WidgetHolder::unrelated_separator(),
			TextAreaInput::new(x.clone())
				.on_update(update_value(|x: &TextAreaInput| TaggedValue::String(x.value.clone()), node_id, index))
				.widget_holder(),
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
			CheckboxInput::new(*x)
				.on_update(update_value(|x: &CheckboxInput| TaggedValue::Bool(x.checked), node_id, index))
				.widget_holder(),
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
			number_props
				.value(Some(x))
				.on_update(update_value(|x: &NumberInput| TaggedValue::F64(x.value.unwrap()), node_id, index))
				.widget_holder(),
		])
	} else if let NodeInput::Value {
		tagged_value: TaggedValue::U32(x),
		exposed: false,
	} = document_node.inputs[index]
	{
		widgets.extend_from_slice(&[
			WidgetHolder::unrelated_separator(),
			number_props
				.value(Some(x as f64))
				.on_update(update_value(|x: &NumberInput| TaggedValue::U32(x.value.unwrap() as u32), node_id, index))
				.widget_holder(),
		])
	}
	widgets
}

/// Properties for the input node, with information describing how frames work and a refresh button
pub fn input_properties(_document_node: &DocumentNode, _node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let information = WidgetHolder::text_widget("The graph's input is the artwork under the frame layer");
	let refresh_button = TextButton::new("Refresh Input")
		.tooltip("Refresh the artwork under the frame")
		.on_update(|_| DocumentMessage::NodeGraphFrameGenerate.into())
		.widget_holder();
	vec![LayoutGroup::Row { widgets: vec![information] }, LayoutGroup::Row { widgets: vec![refresh_button] }]
}

pub fn adjust_hsl_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let hue_shift = number_widget(document_node, node_id, 1, "Hue Shift", NumberInput::default().min(-180.).max(180.).unit("°"), true);
	let saturation_shift = number_widget(document_node, node_id, 2, "Saturation Shift", NumberInput::default().min(-100.).max(100.).unit("%"), true);
	let lightness_shift = number_widget(document_node, node_id, 3, "Lightness Shift", NumberInput::default().min(-100.).max(100.).unit("%"), true);

	vec![
		LayoutGroup::Row { widgets: hue_shift },
		LayoutGroup::Row { widgets: saturation_shift },
		LayoutGroup::Row { widgets: lightness_shift },
	]
}

pub fn brighten_image_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let brightness = number_widget(document_node, node_id, 1, "Brightness", NumberInput::default().min(-255.).max(255.), true);
	let contrast = number_widget(document_node, node_id, 2, "Contrast", NumberInput::default().min(-255.).max(255.), true);

	vec![LayoutGroup::Row { widgets: brightness }, LayoutGroup::Row { widgets: contrast }]
}

pub fn blur_image_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let radius = number_widget(document_node, node_id, 1, "Radius", NumberInput::default().min(0.).max(20.).int(), true);
	let sigma = number_widget(document_node, node_id, 2, "Sigma", NumberInput::default().min(0.).max(10000.), true);

	vec![LayoutGroup::Row { widgets: radius }, LayoutGroup::Row { widgets: sigma }]
}

pub fn adjust_threshold_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let thereshold = number_widget(document_node, node_id, 1, "Threshold", NumberInput::default().min(0.).max(1.), true);

	vec![LayoutGroup::Row { widgets: thereshold }]
}

pub fn adjust_vibrance_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let vibrance = number_widget(document_node, node_id, 1, "Vibrance", NumberInput::default().min(-100.).max(100.).unit("%"), true);

	vec![LayoutGroup::Row { widgets: vibrance }]
}

#[cfg(feature = "gpu")]
pub fn gpu_map_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let map = text_widget(document_node, node_id, 1, "Map", true);

	vec![LayoutGroup::Row { widgets: map }]
}

pub fn multiply_opacity(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let gamma = number_widget(document_node, node_id, 1, "Factor", NumberInput::default().min(0.).max(1.), true);

	vec![LayoutGroup::Row { widgets: gamma }]
}

pub fn posterize_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let value = number_widget(document_node, node_id, 1, "Levels", NumberInput::default().min(2.).max(255.).int(), true);

	vec![LayoutGroup::Row { widgets: value }]
}

#[cfg(feature = "quantization")]
pub fn quantize_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let value = number_widget(document_node, node_id, 1, "Levels", NumberInput::default().min(1.).max(1000.).int(), true);
	let index = number_widget(document_node, node_id, 1, "Fit Fn Index", NumberInput::default().min(0.).max(2.).int(), true);

	vec![LayoutGroup::Row { widgets: value }, LayoutGroup::Row { widgets: index }]
}
pub fn exposure_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let exposure = number_widget(document_node, node_id, 1, "Exposure", NumberInput::default().min(-20.).max(20.), true);
	let offset = number_widget(document_node, node_id, 2, "Offset", NumberInput::default().min(-0.5).max(0.5), true);
	let gamma_correction = number_widget(
		document_node,
		node_id,
		3,
		"Gamma Correction",
		NumberInput::default().min(0.01).max(9.99).mode_increment().increment_step(0.1),
		true,
	);

	vec![
		LayoutGroup::Row { widgets: exposure },
		LayoutGroup::Row { widgets: offset },
		LayoutGroup::Row { widgets: gamma_correction },
	]
}

pub fn add_properties(document_node: &DocumentNode, node_id: NodeId, _context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let operand = |name: &str, index| {
		let widgets = number_widget(document_node, node_id, index, name, NumberInput::default(), true);

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
				NumberInput::new(Some(vec2.x))
					.label("X")
					.unit(" px")
					.on_update(update_value(move |input: &NumberInput| TaggedValue::DVec2(DVec2::new(input.value.unwrap(), vec2.y)), node_id, index))
					.widget_holder(),
				WidgetHolder::unrelated_separator(),
				NumberInput::new(Some(vec2.y))
					.label("Y")
					.unit(" px")
					.on_update(update_value(move |input: &NumberInput| TaggedValue::DVec2(DVec2::new(vec2.x, input.value.unwrap())), node_id, index))
					.widget_holder(),
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
				NumberInput::new(Some(val.to_degrees()))
					.unit("°")
					.mode(NumberInputMode::Range)
					.range_min(Some(-180.))
					.range_max(Some(180.))
					.on_update(update_value(|number_input: &NumberInput| TaggedValue::F64(number_input.value.unwrap().to_radians()), node_id, index))
					.widget_holder(),
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
				NumberInput::new(Some(vec2.x))
					.label("X")
					.on_update(update_value(move |input: &NumberInput| TaggedValue::DVec2(DVec2::new(input.value.unwrap(), vec2.y)), node_id, index))
					.widget_holder(),
				WidgetHolder::unrelated_separator(),
				NumberInput::new(Some(vec2.y))
					.label("Y")
					.on_update(update_value(move |input: &NumberInput| TaggedValue::DVec2(DVec2::new(vec2.x, input.value.unwrap())), node_id, index))
					.widget_holder(),
			]);
		}

		LayoutGroup::Row { widgets }
	};
	vec![translation, rotation, scale]
}

pub fn imaginate_properties(document_node: &DocumentNode, node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
	let imaginate_node = [context.nested_path, &[node_id]].concat();
	let layer_path = context.layer_path.to_vec();

	let resolve_input = |name: &str| IMAGINATE_NODE.inputs.iter().position(|input| input.name == name).unwrap_or_else(|| panic!("Input {name} not found"));
	let seed_index = resolve_input("Seed");
	let resolution_index = resolve_input("Resolution");
	let samples_index = resolve_input("Samples");
	let sampling_method_index = resolve_input("Sampling Method");
	let text_guidance_index = resolve_input("Prompt Guidance");
	let text_index = resolve_input("Prompt");
	let neg_index = resolve_input("Negative Prompt");
	let base_img_index = resolve_input("Adapt Input Image");
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
			IconButton::new("Settings", 24)
				.tooltip("Preferences: Imaginate")
				.on_update(|_| DialogMessage::RequestPreferencesDialog.into())
				.widget_holder(),
			WidgetHolder::unrelated_separator(),
			WidgetHolder::bold_text(status),
			WidgetHolder::related_separator(),
			IconButton::new("Reload", 24)
				.tooltip("Refresh connection status")
				.on_update(|_| PortfolioMessage::ImaginateCheckServerStatus.into())
				.widget_holder(),
		];
		LayoutGroup::Row { widgets }.with_tooltip("Connection status to the server that computes generated images")
	};

	let &NodeInput::Value {tagged_value: TaggedValue::ImaginateStatus( imaginate_status),..} = status_value else {
		panic!("Invalid status input")
	};
	let NodeInput::Value {tagged_value: TaggedValue::RcImage( cached_data),..} = cached_value else {
		panic!("Invalid cached image input, recieved {:?}, index: {}", cached_value, cached_index)
	};
	let &NodeInput::Value {tagged_value: TaggedValue::F64( percent_complete),..} = complete_value else {
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
			ImaginateStatus::Uploading(percent) => format!("Uploading Input Image: {percent:.0}%"),
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
				widgets.push(TextButton::new("Beginning...").tooltip("Sending image generation request to the server").disabled(true).widget_holder());
			}
			ImaginateStatus::Generating => {
				widgets.extend_from_slice(&assist_separators);
				widgets.push(
					TextButton::new("Terminate")
						.tooltip("Cancel the in-progress image generation and keep the latest progress")
						.on_update({
							let imaginate_node = imaginate_node.clone();
							move |_| {
								DocumentMessage::NodeGraphFrameImaginateTerminate {
									layer_path: layer_path.clone(),
									node_path: imaginate_node.clone(),
								}
								.into()
							}
						})
						.widget_holder(),
				);
			}
			ImaginateStatus::Terminating => {
				widgets.extend_from_slice(&assist_separators);
				widgets.push(
					TextButton::new("Terminating...")
						.tooltip("Waiting on the final image generated after termination")
						.disabled(true)
						.widget_holder(),
				);
			}
			ImaginateStatus::Idle | ImaginateStatus::Terminated => widgets.extend_from_slice(&[
				IconButton::new("Random", 24)
					.tooltip("Generate with a new random seed")
					.on_update({
						let imaginate_node = imaginate_node.clone();
						move |_| {
							DocumentMessage::NodeGraphFrameImaginateRandom {
								imaginate_node: imaginate_node.clone(),
							}
							.into()
						}
					})
					.widget_holder(),
				WidgetHolder::unrelated_separator(),
				TextButton::new("Generate")
					.tooltip("Fill layer frame by generating a new image")
					.on_update({
						let imaginate_node = imaginate_node.clone();
						move |_| {
							DocumentMessage::NodeGraphFrameImaginate {
								imaginate_node: imaginate_node.clone(),
							}
							.into()
						}
					})
					.widget_holder(),
				WidgetHolder::related_separator(),
				TextButton::new("Clear")
					.tooltip("Remove generated image from the layer frame")
					.disabled(cached_data.is_none())
					.on_update(update_value(|_| TaggedValue::RcImage(None), node_id, cached_index))
					.widget_holder(),
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
				IconButton::new("Regenerate", 24)
					.tooltip("Set a new random seed")
					.on_update(update_value(move |_| TaggedValue::F64((generate_uuid() >> 1) as f64), node_id, seed_index))
					.widget_holder(),
				WidgetHolder::unrelated_separator(),
				NumberInput::new(Some(seed))
					.min(0.)
					.int()
					.on_update(update_value(move |input: &NumberInput| TaggedValue::F64(input.value.unwrap()), node_id, seed_index))
					.widget_holder(),
			])
		}
		// Note: Limited by f64. You cannot even have all the possible u64 values :)
		LayoutGroup::Row { widgets }.with_tooltip("Seed determines the random outcome, enabling limitless unique variations")
	};

	// Get the existing layer transform
	let transform = context.document.root.transform.inverse() * context.document.multiply_transforms(context.layer_path).unwrap();
	// Create the input to the graph using an empty image
	let image_frame = std::borrow::Cow::Owned(graphene_core::raster::ImageFrame {
		image: graphene_core::raster::Image::empty(),
		transform,
	});
	// Comput the transform input to the node graph frame
	let transform: glam::DAffine2 = context.executor.compute_input(context.network, &imaginate_node, 1, image_frame).unwrap_or_default();

	let resolution = {
		use document_legacy::document::pick_safe_imaginate_resolution;

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
				let w = transform.transform_vector2(DVec2::new(1., 0.)).length();
				let h = transform.transform_vector2(DVec2::new(0., 1.)).length();

				let (x, y) = pick_safe_imaginate_resolution((w, h));

				DVec2::new(x as f64, y as f64)
			});

			let layer_path = context.layer_path.to_vec();
			widgets.extend_from_slice(&[
				WidgetHolder::unrelated_separator(),
				IconButton::new("Rescale", 24)
					.tooltip("Set the Node Graph Frame layer dimensions to this resolution")
					.on_update(move |_| {
						Operation::SetLayerScaleAroundPivot {
							path: layer_path.clone(),
							new_scale: vec2.into(),
						}
						.into()
					})
					.widget_holder(),
				WidgetHolder::unrelated_separator(),
				CheckboxInput::new(!dimensions_is_auto)
					.icon("Edit")
					.tooltip("Set a custom resolution instead of using the frame's rounded dimensions")
					.on_update(update_value(
						move |checkbox_input: &CheckboxInput| {
							if checkbox_input.checked {
								TaggedValue::OptionalDVec2(Some(vec2))
							} else {
								TaggedValue::OptionalDVec2(None)
							}
						},
						node_id,
						resolution_index,
					))
					.widget_holder(),
				WidgetHolder::unrelated_separator(),
				NumberInput::new(Some(vec2.x))
					.label("W")
					.unit(" px")
					.disabled(dimensions_is_auto)
					.on_update(update_value(
						move |number_input: &NumberInput| TaggedValue::OptionalDVec2(round(DVec2::new(number_input.value.unwrap(), vec2.y))),
						node_id,
						resolution_index,
					))
					.widget_holder(),
				WidgetHolder::related_separator(),
				NumberInput::new(Some(vec2.y))
					.label("H")
					.unit(" px")
					.disabled(dimensions_is_auto)
					.on_update(update_value(
						move |number_input: &NumberInput| TaggedValue::OptionalDVec2(round(DVec2::new(vec2.x, number_input.value.unwrap()))),
						node_id,
						resolution_index,
					))
					.widget_holder(),
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
		let widgets = number_widget(document_node, node_id, samples_index, "Sampling Steps", NumberInput::default().min(0.).max(150.).int(), true);
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
				entries.push(DropdownEntryData::new(method.to_string()).on_update(update_value(move |_| TaggedValue::ImaginateSamplingMethod(method), node_id, sampling_method_index)));
			}
			let entries = vec![entries];

			widgets.extend_from_slice(&[
				WidgetHolder::unrelated_separator(),
				DropdownInput::new(entries).selected_index(Some(sampling_method as u32)).widget_holder(),
			]);
		}
		LayoutGroup::Row { widgets }.with_tooltip("Algorithm used to generate the image during each sampling step")
	};

	let text_guidance = {
		let widgets = number_widget(document_node, node_id, text_guidance_index, "Prompt Guidance", NumberInput::default().min(0.).max(30.), true);
		LayoutGroup::Row { widgets }.with_tooltip(
			"Amplification of the text prompt's influence over the outcome. At 0, the prompt is entirely ignored.\n\
			\n\
			Lower values are more creative and exploratory. Higher values are more literal and uninspired.\n\
			\n\
			This parameter is otherwise known as CFG (classifier-free guidance).",
		)
	};

	let text_prompt = {
		let widgets = text_area_widget(document_node, node_id, text_index, "Prompt", true);
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
		let widgets = bool_widget(document_node, node_id, base_img_index, "Adapt Input Image", true);
		LayoutGroup::Row { widgets }.with_tooltip("Generate an image based upon the bitmap data plugged into this node")
	};
	let image_creativity = {
		let props = NumberInput::default().percentage().disabled(!use_base_image);
		let widgets = number_widget(document_node, node_id, img_creativity_index, "Image Creativity", props, true);
		LayoutGroup::Row { widgets }.with_tooltip(
			"Strength of the artistic liberties allowing changes from the input image. The image is unchanged at 0% and completely different at 100%.\n\
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
				LayerReferenceInput::new(layer_path.clone(), layer_reference_input_layer_name.cloned(), layer_reference_input_layer_type.cloned())
					.disabled(!use_base_image)
					.on_update(update_value(|input: &LayerReferenceInput| TaggedValue::LayerPath(input.value.clone()), node_id, mask_index))
					.widget_holder(),
			]);
		}
		LayoutGroup::Row { widgets }.with_tooltip(
			"Reference to a layer or folder which masks parts of the input image. Image generation is constrained to masked areas.\n\
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
					RadioInput::new(
						[(true, "Inpaint"), (false, "Outpaint")]
							.into_iter()
							.map(|(paint, name)| RadioEntryData::new(name).on_update(update_value(move |_| TaggedValue::Bool(paint), node_id, inpaint_index)))
							.collect(),
					)
					.selected_index(1 - in_paint as u32)
					.widget_holder(),
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
			let widgets = number_widget(document_node, node_id, mask_blur_index, "Mask Blur", NumberInput::default().unit(" px").min(0.).max(25.).int(), true);
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
					entries.push(DropdownEntryData::new(mode.to_string()).on_update(update_value(move |_| TaggedValue::ImaginateMaskStartingFill(mode), node_id, mask_fill_index)));
				}
				let entries = vec![entries];

				widgets.extend_from_slice(&[
					WidgetHolder::unrelated_separator(),
					DropdownInput::new(entries).selected_index(Some(starting_fill as u32)).widget_holder(),
				]);
			}
			LayoutGroup::Row { widgets }.with_tooltip(
				"Begin in/outpainting the masked areas using this fill content as the starting input image.\n\
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
			This filter can be used on its own by enabling 'Adapt Input Image' and setting 'Sampling Steps' to 0.",
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
