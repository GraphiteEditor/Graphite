//! This has all been copied out of node_properties.rs to avoid leaving hundreds of lines of commented out code in that file. It's left here instead for future reference.

// pub fn imaginate_sampling_method(parameter_widgets_info: ParameterWidgetsInfo) -> LayoutGroup {
// 	let ParameterWidgetsInfo { node_id, index, .. } = parameter_widgets_info;

// 	vec![
// 		DropdownInput::new(
// 			ImaginateSamplingMethod::list()
// 				.into_iter()
// 				.map(|method| {
// 					vec![
// 						MenuListEntry::new(format!("{:?}", method))
// 							.label(method.to_string())
// 							.on_update(update_value(move |_| TaggedValue::ImaginateSamplingMethod(method), node_id, index)),
// 					]
// 				})
// 				.collect(),
// 		)
// 		.widget_holder(),
// 	]
// 	.into()
// }

// pub fn imaginate_mask_starting_fill(parameter_widgets_info: ParameterWidgetsInfo) -> LayoutGroup {
// 	let ParameterWidgetsInfo { node_id, index, .. } = parameter_widgets_info;

// 	vec![
// 		DropdownInput::new(
// 			ImaginateMaskStartingFill::list()
// 				.into_iter()
// 				.map(|fill| {
// 					vec![
// 						MenuListEntry::new(format!("{:?}", fill))
// 							.label(fill.to_string())
// 							.on_update(update_value(move |_| TaggedValue::ImaginateMaskStartingFill(fill), node_id, index)),
// 					]
// 				})
// 				.collect(),
// 		)
// 		.widget_holder(),
// 	]
// 	.into()
// }

// pub(crate) fn imaginate_properties(node_id: NodeId, context: &mut NodePropertiesContext) -> Vec<LayoutGroup> {
// 	let imaginate_node = [context.selection_network_path, &[node_id]].concat();

// 	let resolve_input = |name: &str| {
// 		IMAGINATE_NODE
// 			.default_node_template()
// 			.persistent_node_metadata
// 			.input_properties
// 			.iter()
// 			.position(|row| row.input_name.as_str() == name)
// 			.unwrap_or_else(|| panic!("Input {name} not found"))
// 	};
// 	let seed_index = resolve_input("Seed");
// 	let resolution_index = resolve_input("Resolution");
// 	let samples_index = resolve_input("Samples");
// 	let sampling_method_index = resolve_input("Sampling Method");
// 	let text_guidance_index = resolve_input("Prompt Guidance");
// 	let text_index = resolve_input("Prompt");
// 	let neg_index = resolve_input("Negative Prompt");
// 	let base_img_index = resolve_input("Adapt Input Image");
// 	let img_creativity_index = resolve_input("Image Creativity");
// 	// let mask_index = resolve_input("Masking Layer");
// 	// let inpaint_index = resolve_input("Inpaint");
// 	// let mask_blur_index = resolve_input("Mask Blur");
// 	// let mask_fill_index = resolve_input("Mask Starting Fill");
// 	let faces_index = resolve_input("Improve Faces");
// 	let tiling_index = resolve_input("Tiling");

// 	let document_node = match get_document_node(node_id, context) {
// 		Ok(document_node) => document_node,
// 		Err(err) => {
// 			log::error!("Could not get document node in imaginate_properties: {err}");
// 			return Vec::new();
// 		}
// 	};
// 	let controller = &document_node.inputs[resolve_input("Controller")];

// 	let server_status = {
// 		let server_status = context.persistent_data.imaginate.server_status();
// 		let status_text = server_status.to_text();
// 		let mut widgets = vec![
// 			TextLabel::new("Server").widget_holder(),
// 			Separator::new(SeparatorType::Unrelated).widget_holder(),
// 			IconButton::new("Settings", 24)
// 				.tooltip("Preferences: Imaginate")
// 				.on_update(|_| DialogMessage::RequestPreferencesDialog.into())
// 				.widget_holder(),
// 			Separator::new(SeparatorType::Unrelated).widget_holder(),
// 			TextLabel::new(status_text).bold(true).widget_holder(),
// 			Separator::new(SeparatorType::Related).widget_holder(),
// 			IconButton::new("Reload", 24)
// 				.tooltip("Refresh connection status")
// 				.on_update(|_| PortfolioMessage::ImaginateCheckServerStatus.into())
// 				.widget_holder(),
// 		];
// 		if let ImaginateServerStatus::Unavailable | ImaginateServerStatus::Failed(_) = server_status {
// 			widgets.extend([
// 				Separator::new(SeparatorType::Unrelated).widget_holder(),
// 				TextButton::new("Server Help")
// 					.tooltip("Learn how to connect Imaginate to an image generation server")
// 					.on_update(|_| {
// 						FrontendMessage::TriggerVisitLink {
// 							url: "https://github.com/GraphiteEditor/Graphite/discussions/1089".to_string(),
// 						}
// 						.into()
// 					})
// 					.widget_holder(),
// 			]);
// 		}
// 		LayoutGroup::Row { widgets }.with_tooltip("Connection status to the server that computes generated images")
// 	};

// 	let Some(TaggedValue::ImaginateController(controller)) = controller.as_value() else {
// 		panic!("Invalid output status input")
// 	};
// 	let imaginate_status = controller.get_status();

// 	let use_base_image = if let Some(&TaggedValue::Bool(use_base_image)) = &document_node.inputs[base_img_index].as_value() {
// 		use_base_image
// 	} else {
// 		true
// 	};

// 	let transform_not_connected = false;

// 	let progress = {
// 		let mut widgets = vec![TextLabel::new("Progress").widget_holder(), Separator::new(SeparatorType::Unrelated).widget_holder()];
// 		add_blank_assist(&mut widgets);
// 		let status = imaginate_status.to_text();
// 		widgets.push(TextLabel::new(status.as_ref()).bold(true).widget_holder());
// 		LayoutGroup::Row { widgets }.with_tooltip(match imaginate_status {
// 			ImaginateStatus::Failed(_) => status.as_ref(),
// 			_ => "When generating, the percentage represents how many sampling steps have so far been processed out of the target number",
// 		})
// 	};

// 	let image_controls = {
// 		let mut widgets = vec![TextLabel::new("Image").widget_holder(), Separator::new(SeparatorType::Unrelated).widget_holder()];

// 		match &imaginate_status {
// 			ImaginateStatus::Beginning | ImaginateStatus::Uploading => {
// 				add_blank_assist(&mut widgets);
// 				widgets.push(TextButton::new("Beginning...").tooltip("Sending image generation request to the server").disabled(true).widget_holder());
// 			}
// 			ImaginateStatus::Generating(_) => {
// 				add_blank_assist(&mut widgets);
// 				widgets.push(
// 					TextButton::new("Terminate")
// 						.tooltip("Cancel the in-progress image generation and keep the latest progress")
// 						.on_update({
// 							let controller = controller.clone();
// 							move |_| {
// 								controller.request_termination();
// 								Message::NoOp
// 							}
// 						})
// 						.widget_holder(),
// 				);
// 			}
// 			ImaginateStatus::Terminating => {
// 				add_blank_assist(&mut widgets);
// 				widgets.push(
// 					TextButton::new("Terminating...")
// 						.tooltip("Waiting on the final image generated after termination")
// 						.disabled(true)
// 						.widget_holder(),
// 				);
// 			}
// 			ImaginateStatus::Ready | ImaginateStatus::ReadyDone | ImaginateStatus::Terminated | ImaginateStatus::Failed(_) => widgets.extend_from_slice(&[
// 				IconButton::new("Random", 24)
// 					.tooltip("Generate with a new random seed")
// 					.on_update({
// 						let imaginate_node = imaginate_node.clone();
// 						let controller = controller.clone();
// 						move |_| {
// 							controller.trigger_regenerate();
// 							DocumentMessage::ImaginateRandom {
// 								imaginate_node: imaginate_node.clone(),
// 								then_generate: true,
// 							}
// 							.into()
// 						}
// 					})
// 					.widget_holder(),
// 				Separator::new(SeparatorType::Unrelated).widget_holder(),
// 				TextButton::new("Generate")
// 					.tooltip("Fill layer frame by generating a new image")
// 					.on_update({
// 						let controller = controller.clone();
// 						let imaginate_node = imaginate_node.clone();
// 						move |_| {
// 							controller.trigger_regenerate();
// 							DocumentMessage::ImaginateGenerate {
// 								imaginate_node: imaginate_node.clone(),
// 							}
// 							.into()
// 						}
// 					})
// 					.widget_holder(),
// 				Separator::new(SeparatorType::Related).widget_holder(),
// 				TextButton::new("Clear")
// 					.tooltip("Remove generated image from the layer frame")
// 					.disabled(!matches!(imaginate_status, ImaginateStatus::ReadyDone))
// 					.on_update({
// 						let controller = controller.clone();
// 						let imaginate_node = imaginate_node.clone();
// 						move |_| {
// 							controller.set_status(ImaginateStatus::Ready);
// 							DocumentMessage::ImaginateGenerate {
// 								imaginate_node: imaginate_node.clone(),
// 							}
// 							.into()
// 						}
// 					})
// 					.widget_holder(),
// 			]),
// 		}
// 		LayoutGroup::Row { widgets }.with_tooltip("Buttons that control the image generation process")
// 	};

// 	// Requires custom layout for the regenerate button
// 	let seed = {
// 		let mut widgets = start_widgets(document_node, node_id, seed_index, "Seed", FrontendGraphDataType::Number, false);

// 		let Some(input) = document_node.inputs.get(seed_index) else {
// 			log::warn!("A widget failed to be built because its node's input index is invalid.");
// 			return vec![];
// 		};
// 		if let Some(&TaggedValue::F64(seed)) = &input.as_non_exposed_value() {
// 			widgets.extend_from_slice(&[
// 				Separator::new(SeparatorType::Unrelated).widget_holder(),
// 				IconButton::new("Resync", 24)
// 					.tooltip("Set a new random seed")
// 					.on_update({
// 						let imaginate_node = imaginate_node.clone();
// 						move |_| {
// 							DocumentMessage::ImaginateRandom {
// 								imaginate_node: imaginate_node.clone(),
// 								then_generate: false,
// 							}
// 							.into()
// 						}
// 					})
// 					.widget_holder(),
// 				Separator::new(SeparatorType::Unrelated).widget_holder(),
// 				NumberInput::new(Some(seed))
// 					.int()
// 					.min(-((1_u64 << f64::MANTISSA_DIGITS) as f64))
// 					.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
// 					.on_update(update_value(move |input: &NumberInput| TaggedValue::F64(input.value.unwrap()), node_id, seed_index))
// 					.on_commit(commit_value)
// 					.mode(NumberInputMode::Increment)
// 					.widget_holder(),
// 			])
// 		}
// 		// Note: Limited by f64. You cannot even have all the possible u64 values :)
// 		LayoutGroup::Row { widgets }.with_tooltip("Seed determines the random outcome, enabling limitless unique variations")
// 	};

// 	// let transform = context
// 	// 	.executor
// 	// 	.introspect_node_in_network(context.network, &imaginate_node, |network| network.inputs.first().copied(), |frame: &ImageFrame<Color>| frame.transform)
// 	// 	.unwrap_or_default();
// 	let image_size = context
// 		.executor
// 		.introspect_node_in_network(
// 			context.network_interface.document_network().unwrap(),
// 			&imaginate_node,
// 			|network| {
// 				network
// 					.nodes
// 					.iter()
// 					.find(|node| {
// 						node.1
// 							.inputs
// 							.iter()
// 							.any(|node_input| if let NodeInput::Network { import_index, .. } = node_input { *import_index == 0 } else { false })
// 					})
// 					.map(|(node_id, _)| node_id)
// 					.copied()
// 			},
// 			|frame: &IORecord<(), ImageFrame<Color>>| (frame.output.image.width, frame.output.image.height),
// 		)
// 		.unwrap_or_default();

// 	let document_node = match get_document_node(node_id, context) {
// 		Ok(document_node) => document_node,
// 		Err(err) => {
// 			log::error!("Could not get document node in imaginate_properties: {err}");
// 			return Vec::new();
// 		}
// 	};

// 	let resolution = {
// 		let mut widgets = start_widgets(document_node, node_id, resolution_index, "Resolution", FrontendGraphDataType::Number, false);

// 		let round = |size: DVec2| {
// 			let (x, y) = graphene_std::imaginate::pick_safe_imaginate_resolution(size.into());
// 			DVec2::new(x as f64, y as f64)
// 		};

// 		let Some(input) = document_node.inputs.get(resolution_index) else {
// 			log::warn!("A widget failed to be built because its node's input index is invalid.");
// 			return vec![];
// 		};
// 		if let Some(&TaggedValue::OptionalDVec2(vec2)) = &input.as_non_exposed_value() {
// 			let dimensions_is_auto = vec2.is_none();
// 			let vec2 = vec2.unwrap_or_else(|| round((image_size.0 as f64, image_size.1 as f64).into()));

// 			widgets.extend_from_slice(&[
// 				Separator::new(SeparatorType::Unrelated).widget_holder(),
// 				IconButton::new("FrameAll", 24)
// 					.tooltip("Set the layer dimensions to this resolution")
// 					.on_update(move |_| DialogMessage::RequestComingSoonDialog { issue: None }.into())
// 					.widget_holder(),
// 				Separator::new(SeparatorType::Unrelated).widget_holder(),
// 				CheckboxInput::new(!dimensions_is_auto || transform_not_connected)
// 					.icon("Edit12px")
// 					.tooltip({
// 						let message = "Set a custom resolution instead of using the input's dimensions (rounded to the nearest 64)";
// 						let manual_message = "Set a custom resolution instead of using the input's dimensions (rounded to the nearest 64).\n\
// 							\n\
// 							(Resolution must be set manually while the 'Transform' input is disconnected.)";

// 						if transform_not_connected {
// 							manual_message
// 						} else {
// 							message
// 						}
// 					})
// 					.disabled(transform_not_connected)
// 					.on_update(update_value(
// 						move |checkbox_input: &CheckboxInput| TaggedValue::OptionalDVec2(if checkbox_input.checked { Some(vec2) } else { None }),
// 						node_id,
// 						resolution_index,
// 					))
// 					.on_commit(commit_value)
// 					.for_label(checkbox_id.clone())
// 					.widget_holder(),
// 				Separator::new(SeparatorType::Related).widget_holder(),
// 				NumberInput::new(Some(vec2.x))
// 					.label("W")
// 					.min(64.)
// 					.step(64.)
// 					.unit(" px")
// 					.disabled(dimensions_is_auto && !transform_not_connected)
// 					.on_update(update_value(
// 						move |number_input: &NumberInput| TaggedValue::OptionalDVec2(Some(round(DVec2::new(number_input.value.unwrap(), vec2.y)))),
// 						node_id,
// 						resolution_index,
// 					))
// 					.on_commit(commit_value)
// 					.widget_holder(),
// 				Separator::new(SeparatorType::Related).widget_holder(),
// 				NumberInput::new(Some(vec2.y))
// 					.label("H")
// 					.min(64.)
// 					.step(64.)
// 					.unit(" px")
// 					.disabled(dimensions_is_auto && !transform_not_connected)
// 					.on_update(update_value(
// 						move |number_input: &NumberInput| TaggedValue::OptionalDVec2(Some(round(DVec2::new(vec2.x, number_input.value.unwrap())))),
// 						node_id,
// 						resolution_index,
// 					))
// 					.on_commit(commit_value)
// 					.widget_holder(),
// 			])
// 		}
// 		LayoutGroup::Row { widgets }.with_tooltip(
// 			"Width and height of the image that will be generated. Larger resolutions take longer to compute.\n\
// 			\n\
// 			512x512 yields optimal results because the AI is trained to understand that scale best. Larger sizes may tend to integrate the prompt's subject more than once. Small sizes are often incoherent.\n\
// 			\n\
// 			Dimensions must be a multiple of 64, so these are set by rounding the layer dimensions. A resolution exceeding 1 megapixel is reduced below that limit because larger sizes may exceed available GPU memory on the server.")
// 	};

// 	let sampling_steps = {
// 		let widgets = number_widget(document_node, node_id, samples_index, "Sampling Steps", NumberInput::default().min(0.).max(150.).int(), true);
// 		LayoutGroup::Row { widgets }.with_tooltip("Number of iterations to improve the image generation quality, with diminishing returns around 40 when using the Euler A sampling method")
// 	};

// 	let sampling_method = {
// 		let mut widgets = start_widgets(document_node, node_id, sampling_method_index, "Sampling Method", FrontendGraphDataType::General, true);

// 		let Some(input) = document_node.inputs.get(sampling_method_index) else {
// 			log::warn!("A widget failed to be built because its node's input index is invalid.");
// 			return vec![];
// 		};
// 		if let Some(&TaggedValue::ImaginateSamplingMethod(sampling_method)) = &input.as_non_exposed_value() {
// 			let sampling_methods = ImaginateSamplingMethod::list();
// 			let mut entries = Vec::with_capacity(sampling_methods.len());
// 			for method in sampling_methods {
// 				entries.push(
// 					MenuListEntry::new(format!("{method:?}"))
// 						.label(method.to_string())
// 						.on_update(update_value(move |_| TaggedValue::ImaginateSamplingMethod(method), node_id, sampling_method_index))
// 						.on_commit(commit_value),
// 				);
// 			}
// 			let entries = vec![entries];

// 			widgets.extend_from_slice(&[
// 				Separator::new(SeparatorType::Unrelated).widget_holder(),
// 				DropdownInput::new(entries).selected_index(Some(sampling_method as u32)).widget_holder(),
// 			]);
// 		}
// 		LayoutGroup::Row { widgets }.with_tooltip("Algorithm used to generate the image during each sampling step")
// 	};

// 	let text_guidance = {
// 		let widgets = number_widget(document_node, node_id, text_guidance_index, "Prompt Guidance", NumberInput::default().min(0.).max(30.), true);
// 		LayoutGroup::Row { widgets }.with_tooltip(
// 			"Amplification of the text prompt's influence over the outcome. At 0, the prompt is entirely ignored.\n\
// 			\n\
// 			Lower values are more creative and exploratory. Higher values are more literal and uninspired.\n\
// 			\n\
// 			This parameter is otherwise known as CFG (classifier-free guidance).",
// 		)
// 	};

// 	let text_prompt = {
// 		let widgets = text_area_widget(document_node, node_id, text_index, "Prompt", true);
// 		LayoutGroup::Row { widgets }.with_tooltip(
// 			"Description of the desired image subject and style.\n\
// 			\n\
// 			Include an artist name like \"Rembrandt\" or art medium like \"watercolor\" or \"photography\" to influence the look. List multiple to meld styles.\n\
// 			\n\
// 			To boost (or lessen) the importance of a word or phrase, wrap it in parentheses ending with a colon and a multiplier, for example:\n\
// 			\"Colorless green ideas (sleep:1.3) furiously\"",
// 		)
// 	};
// 	let negative_prompt = {
// 		let widgets = text_area_widget(document_node, node_id, neg_index, "Negative Prompt", true);
// 		LayoutGroup::Row { widgets }.with_tooltip("A negative text prompt can be used to list things like objects or colors to avoid")
// 	};
// 	let base_image = {
// 		let widgets = bool_widget(document_node, node_id, base_img_index, "Adapt Input Image", CheckboxInput::default().for_label(checkbox_id.clone()), true);
// 		LayoutGroup::Row { widgets }.with_tooltip("Generate an image based upon the bitmap data plugged into this node")
// 	};
// 	let image_creativity = {
// 		let props = NumberInput::default().percentage().disabled(!use_base_image);
// 		let widgets = number_widget(document_node, node_id, img_creativity_index, "Image Creativity", props, true);
// 		LayoutGroup::Row { widgets }.with_tooltip(
// 			"Strength of the artistic liberties allowing changes from the input image. The image is unchanged at 0% and completely different at 100%.\n\
// 			\n\
// 			This parameter is otherwise known as denoising strength.",
// 		)
// 	};

// 	let mut layout = vec![
// 		server_status,
// 		progress,
// 		image_controls,
// 		seed,
// 		resolution,
// 		sampling_steps,
// 		sampling_method,
// 		text_guidance,
// 		text_prompt,
// 		negative_prompt,
// 		base_image,
// 		image_creativity,
// 		// layer_mask,
// 	];

// 	// if use_base_image && layer_reference_input_layer_is_some {
// 	// 	let in_paint = {
// 	// 		let mut widgets = start_widgets(document_node, node_id, inpaint_index, "Inpaint", FrontendGraphDataType::Boolean, true);

// 	// 		if let Some(& TaggedValue::Bool(in_paint)
// 	//)/ 		} = &document_node.inputs[inpaint_index].as_non_exposed_value()
// 	// 		{
// 	// 			widgets.extend_from_slice(&[
// 	// 				Separator::new(SeparatorType::Unrelated).widget_holder(),
// 	// 				RadioInput::new(
// 	// 					[(true, "Inpaint"), (false, "Outpaint")]
// 	// 						.into_iter()
// 	// 						.map(|(paint, name)| RadioEntryData::new(name).label(name).on_update(update_value(move |_| TaggedValue::Bool(paint), node_id, inpaint_index)))
// 	// 						.collect(),
// 	// 				)
// 	// 				.selected_index(Some(1 - in_paint as u32))
// 	// 				.widget_holder(),
// 	// 			]);
// 	// 		}
// 	// 		LayoutGroup::Row { widgets }.with_tooltip(
// 	// 			"Constrain image generation to the interior (inpaint) or exterior (outpaint) of the mask, while referencing the other unchanged parts as context imagery.\n\
// 	// 			\n\
// 	// 			An unwanted part of an image can be replaced by drawing around it with a black shape and inpainting with that mask layer.\n\
// 	// 			\n\
// 	// 			An image can be uncropped by resizing the Imaginate layer to the target bounds and outpainting with a black rectangle mask matching the original image bounds.",
// 	// 		)
// 	// 	};

// 	// 	let blur_radius = {
// 	// 		let number_props = NumberInput::default().unit(" px").min(0.).max(25.).int();
// 	// 		let widgets = number_widget(document_node, node_id, mask_blur_index, "Mask Blur", number_props, true);
// 	// 		LayoutGroup::Row { widgets }.with_tooltip("Blur radius for the mask. Useful for softening sharp edges to blend the masked area with the rest of the image.")
// 	// 	};

// 	// 	let mask_starting_fill = {
// 	// 		let mut widgets = start_widgets(document_node, node_id, mask_fill_index, "Mask Starting Fill", FrontendGraphDataType::General, true);

// 	// 		if let Some(& TaggedValue::ImaginateMaskStartingFill(starting_fill)
// 	//)/ 		} = &document_node.inputs[mask_fill_index].as_non_exposed_value()
// 	// 		{
// 	// 			let mask_fill_content_modes = ImaginateMaskStartingFill::list();
// 	// 			let mut entries = Vec::with_capacity(mask_fill_content_modes.len());
// 	// 			for mode in mask_fill_content_modes {
// 	// 				entries.push(MenuListEntry::new(format!("{mode:?}")).label(mode.to_string()).on_update(update_value(move |_| TaggedValue::ImaginateMaskStartingFill(mode), node_id, mask_fill_index)));
// 	// 			}
// 	// 			let entries = vec![entries];

// 	// 			widgets.extend_from_slice(&[
// 	// 				Separator::new(SeparatorType::Unrelated).widget_holder(),
// 	// 				DropdownInput::new(entries).selected_index(Some(starting_fill as u32)).widget_holder(),
// 	// 			]);
// 	// 		}
// 	// 		LayoutGroup::Row { widgets }.with_tooltip(
// 	// 			"Begin in/outpainting the masked areas using this fill content as the starting input image.\n\
// 	// 			\n\
// 	// 			Each option can be visualized by generating with 'Sampling Steps' set to 0.",
// 	// 		)
// 	// 	};
// 	// 	layout.extend_from_slice(&[in_paint, blur_radius, mask_starting_fill]);
// 	// }

// 	let improve_faces = {
// 		let widgets = bool_widget(document_node, node_id, faces_index, "Improve Faces", CheckboxInput::default().for_label(checkbox_id.clone()), true);
// 		LayoutGroup::Row { widgets }.with_tooltip(
// 			"Postprocess human (or human-like) faces to look subtly less distorted.\n\
// 			\n\
// 			This filter can be used on its own by enabling 'Adapt Input Image' and setting 'Sampling Steps' to 0.",
// 		)
// 	};
// 	let tiling = {
// 		let widgets = bool_widget(document_node, node_id, tiling_index, "Tiling", CheckboxInput::default().for_label(checkbox_id.clone()), true);
// 		LayoutGroup::Row { widgets }.with_tooltip("Generate the image so its edges loop seamlessly to make repeatable patterns or textures")
// 	};
// 	layout.extend_from_slice(&[improve_faces, tiling]);

// 	layout
// }
