use crate::messages::frontend::utility_types::FrontendImageData;
use crate::messages::portfolio::document::node_graph::wrap_network_in_scope;
use crate::messages::portfolio::document::utility_types::misc::DocumentRenderMode;
use crate::messages::portfolio::utility_types::PersistentData;
use crate::messages::prelude::*;

use document_legacy::{document::pick_safe_imaginate_resolution, layers::layer_info::LayerDataType};
use document_legacy::{LayerId, Operation};
use dyn_any::DynAny;
use graph_craft::document::{generate_uuid, DocumentNodeImplementation, NodeId, NodeInput, NodeNetwork, NodeOutput};
use graph_craft::executor::Compiler;
use graph_craft::imaginate_input::*;
use graph_craft::{concrete, Type, TypeDescriptor};
use graphene_core::raster::{Image, ImageFrame};
use graphene_core::renderer::SvgSegment;
use graphene_core::renderer::SvgSegmentList;
use graphene_core::vector::style::ViewMode;
use graphene_core::vector::VectorData;
use graphene_core::{Color, EditorApi};
use interpreted_executor::executor::DynamicExecutor;

use glam::{DAffine2, DVec2};
use std::borrow::Cow;
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub struct NodeGraphExecutor {
	pub(crate) executor: DynamicExecutor,
	// TODO: This is a memory leak since layers are never removed
	pub(crate) last_output_type: HashMap<Vec<LayerId>, Option<Type>>,
	pub(crate) thumbnails: HashMap<LayerId, HashMap<NodeId, SvgSegmentList>>,
}

fn get_imaginate_index(name: &str) -> usize {
	use crate::messages::portfolio::document::node_graph::IMAGINATE_NODE;
	IMAGINATE_NODE.inputs.iter().position(|input| input.name == name).unwrap_or_else(|| panic!("Input {name} not found"))
}

impl NodeGraphExecutor {
	/// Wraps a network in a scope and returns the new network and the paths to the monitor nodes.
	fn wrap_network(network: NodeNetwork) -> (NodeNetwork, Vec<Vec<NodeId>>) {
		let mut scoped_network = wrap_network_in_scope(network);

		scoped_network.generate_node_paths(&[]);
		let monitor_nodes = scoped_network
			.recursive_nodes()
			.filter(|(node, _, _)| node.implementation == DocumentNodeImplementation::proto("graphene_std::memo::MonitorNode<_>"))
			.map(|(_, _, path)| path)
			.collect();
		scoped_network.duplicate_outputs(&mut generate_uuid);
		scoped_network.remove_dead_nodes();

		(scoped_network, monitor_nodes)
	}

	/// Execute the network by flattening it and creating a borrow stack.
	fn execute_network<'a>(&'a mut self, scoped_network: NodeNetwork, editor_api: EditorApi<'a>) -> Result<Box<dyn dyn_any::DynAny + 'a>, String> {
		// We assume only one output
		assert_eq!(scoped_network.outputs.len(), 1, "Graph with multiple outputs not yet handled");
		let c = Compiler {};
		let proto_network = c.compile_single(scoped_network, true)?;

		assert_ne!(proto_network.nodes.len(), 0, "No protonodes exist?");
		if let Err(e) = self.executor.update(proto_network) {
			error!("Failed to update executor:\n{}", e);
			return Err(e);
		}

		use dyn_any::IntoDynAny;
		use graph_craft::executor::Executor;

		match self.executor.input_type() {
			Some(t) if t == concrete!(EditorApi) => self.executor.execute(editor_api.into_dyn()).map_err(|e| e.to_string()),
			Some(t) if t == concrete!(()) => self.executor.execute(().into_dyn()).map_err(|e| e.to_string()),
			_ => Err("Invalid input type".to_string()),
		}
	}

	pub fn introspect_node(&self, path: &[NodeId]) -> Option<Arc<dyn std::any::Any>> {
		self.executor.introspect(path).flatten()
	}

	pub fn previous_output_type(&self, path: &[LayerId]) -> Option<Type> {
		self.last_output_type.get(path).cloned().flatten()
	}

	/// Computes an input for a node in the graph
	pub fn compute_input<T: dyn_any::StaticType>(&mut self, old_network: &NodeNetwork, node_path: &[NodeId], mut input_index: usize, editor_api: Cow<EditorApi<'_>>) -> Result<T, String> {
		let mut network = old_network.clone();
		// Adjust the output of the graph so we find the relevant output
		'outer: for end in (0..node_path.len()).rev() {
			let mut inner_network = &mut network;
			for &node_id in &node_path[..end] {
				inner_network.outputs[0] = NodeOutput::new(node_id, 0);

				let Some(new_inner) = inner_network.nodes.get_mut(&node_id).and_then(|node| node.implementation.get_network_mut()) else {
					return Err("Failed to find network".to_string());
				};
				inner_network = new_inner;
			}
			match &inner_network.nodes.get(&node_path[end]).unwrap().inputs[input_index] {
				// If the input is from a parent network then adjust the input index and continue iteration
				NodeInput::Network(_) => {
					input_index = inner_network
						.inputs
						.iter()
						.enumerate()
						.filter(|&(_index, &id)| id == node_path[end])
						.nth(input_index)
						.ok_or_else(|| "Invalid network input".to_string())?
						.0;
				}
				// If the input is just a value, return that value
				NodeInput::Value { tagged_value, .. } => return dyn_any::downcast::<T>(tagged_value.clone().to_any()).map(|v| *v),
				// If the input is from a node, set the node to be the output (so that is what is evaluated)
				NodeInput::Node { node_id, output_index, .. } => {
					inner_network.outputs[0] = NodeOutput::new(*node_id, *output_index);
					break 'outer;
				}
				NodeInput::ShortCircut(_) => (),
			}
		}

		let (network, _) = Self::wrap_network(network);
		let boxed = self.execute_network(network, editor_api.into_owned())?;

		dyn_any::downcast::<T>(boxed).map(|v| *v)
	}

	/// Encodes an image into a format using the image crate
	fn encode_img(image: Image<Color>, resize: Option<DVec2>, format: image::ImageOutputFormat) -> Result<(Vec<u8>, (u32, u32)), String> {
		use image::{ImageBuffer, Rgba};
		use std::io::Cursor;

		let (result_bytes, width, height) = image.into_flat_u8();

		let mut output: ImageBuffer<Rgba<u8>, _> = image::ImageBuffer::from_raw(width, height, result_bytes).ok_or_else(|| "Invalid image size".to_string())?;
		if let Some(size) = resize {
			let size = size.as_uvec2();
			if size.x > 0 && size.y > 0 {
				output = image::imageops::resize(&output, size.x, size.y, image::imageops::Triangle);
			}
		}
		let size = output.dimensions();
		let mut image_data: Vec<u8> = Vec::new();
		output.write_to(&mut Cursor::new(&mut image_data), format).map_err(|e| e.to_string())?;
		Ok::<_, String>((image_data, size))
	}

	fn imaginate_parameters(&mut self, network: &NodeNetwork, node_path: &[LayerId], resolution: DVec2, editor_api: &EditorApi) -> Result<ImaginateGenerationParameters, String> {
		let get = get_imaginate_index;
		Ok(ImaginateGenerationParameters {
			seed: self.compute_input::<f64>(network, node_path, get("Seed"), Cow::Borrowed(editor_api))? as u64,
			resolution: resolution.as_uvec2().into(),
			samples: self.compute_input::<f64>(network, node_path, get("Samples"), Cow::Borrowed(editor_api))? as u32,
			sampling_method: self
				.compute_input::<ImaginateSamplingMethod>(network, node_path, get("Sampling Method"), Cow::Borrowed(editor_api))?
				.api_value()
				.to_string(),
			text_guidance: self.compute_input(network, node_path, get("Prompt Guidance"), Cow::Borrowed(editor_api))?,
			text_prompt: self.compute_input(network, node_path, get("Prompt"), Cow::Borrowed(editor_api))?,
			negative_prompt: self.compute_input(network, node_path, get("Negative Prompt"), Cow::Borrowed(editor_api))?,
			image_creativity: Some(self.compute_input::<f64>(network, node_path, get("Image Creativity"), Cow::Borrowed(editor_api))? / 100.),
			restore_faces: self.compute_input(network, node_path, get("Improve Faces"), Cow::Borrowed(editor_api))?,
			tiling: self.compute_input(network, node_path, get("Tiling"), Cow::Borrowed(editor_api))?,
		})
	}

	fn imaginate_base_image(&mut self, network: &NodeNetwork, imaginate_node_path: &[LayerId], resolution: DVec2, editor_api: &EditorApi) -> Result<Option<(ImaginateBaseImage, DAffine2)>, String> {
		let use_base_image = self.compute_input::<bool>(&network, &imaginate_node_path, get_imaginate_index("Adapt Input Image"), Cow::Borrowed(editor_api))?;
		let input_image_frame: Option<ImageFrame<Color>> = if use_base_image {
			Some(self.compute_input::<ImageFrame<Color>>(&network, &imaginate_node_path, get_imaginate_index("Input Image"), Cow::Borrowed(editor_api))?)
		} else {
			None
		};

		let base_image = if let Some(ImageFrame { image, transform }) = input_image_frame {
			// Only use if has size
			if image.width > 0 && image.height > 0 {
				let (image_data, size) = Self::encode_img(image, Some(resolution), image::ImageOutputFormat::Png)?;
				let size = DVec2::new(size.0 as f64, size.1 as f64);
				let mime = "image/png".to_string();
				Some((ImaginateBaseImage { image_data, size, mime }, transform))
			} else {
				info!("Base image is input but has no size.");
				None
			}
		} else {
			None
		};
		Ok(base_image)
	}

	fn imaginate_mask_image(
		&mut self,
		network: &NodeNetwork,
		node_path: &[LayerId],
		editor_api: &EditorApi<'_>,
		image_transform: Option<DAffine2>,
		document: &mut DocumentMessageHandler,
		persistent_data: &PersistentData,
	) -> Result<Option<ImaginateMaskImage>, String> {
		if let Some(transform) = image_transform {
			let mask_path: Option<Vec<LayerId>> = self.compute_input(&network, &node_path, get_imaginate_index("Masking Layer"), Cow::Borrowed(&editor_api))?;

			// Calculate the size of the frame
			let size = DVec2::new(transform.transform_vector2(DVec2::new(1., 0.)).length(), transform.transform_vector2(DVec2::new(0., 1.)).length());

			// Render the masking layer within the frame
			let old_transforms = document.remove_document_transform();
			let mask_is_some = mask_path.is_some();
			let mask_image = mask_path.filter(|mask_layer_path| document.document_legacy.layer(mask_layer_path).is_ok()).map(|mask_layer_path| {
				let render_mode = DocumentRenderMode::LayerCutout(&mask_layer_path, graphene_core::raster::color::Color::WHITE);
				let svg = document.render_document(size, transform.inverse(), persistent_data, render_mode);

				ImaginateMaskImage { svg, size }
			});

			if mask_is_some && mask_image.is_none() {
				return Err(
					"Imagination masking layer is missing.\nIt may have been deleted or moved. Please drag a new layer reference\ninto the 'Masking Layer' parameter input, then generate again."
						.to_string(),
				);
			}

			document.restore_document_transform(old_transforms);
			Ok(mask_image)
		} else {
			Ok(None)
		}
	}

	fn generate_imaginate(
		&mut self,
		network: NodeNetwork,
		imaginate_node_path: Vec<NodeId>,
		(document, document_id): (&mut DocumentMessageHandler, u64),
		layer_path: Vec<LayerId>,
		mut editor_api: EditorApi<'_>,
		(preferences, persistent_data): (&PreferencesMessageHandler, &PersistentData),
	) -> Result<Message, String> {
		let image = editor_api.image_frame.take();

		// Get the node graph layer
		let layer = document.document_legacy.layer(&layer_path).map_err(|e| format!("No layer: {e:?}"))?;
		let transform = layer.transform;

		let resolution: Option<glam::DVec2> = self.compute_input(&network, &imaginate_node_path, get_imaginate_index("Resolution"), Cow::Borrowed(&editor_api))?;
		let resolution = resolution.unwrap_or_else(|| {
			let (x, y) = pick_safe_imaginate_resolution((transform.transform_vector2(DVec2::new(1., 0.)).length(), transform.transform_vector2(DVec2::new(0., 1.)).length()));
			DVec2::new(x as f64, y as f64)
		});

		let parameters = self.imaginate_parameters(&network, &imaginate_node_path, resolution, &editor_api)?;

		editor_api.image_frame = image;
		let base = self.imaginate_base_image(&network, &imaginate_node_path, resolution, &editor_api)?;
		let image_transform = base.as_ref().map(|base| base.1);
		let base_image = base.map(|base| base.0);

		let mask_image = self.imaginate_mask_image(&network, &imaginate_node_path, &editor_api, image_transform, document, persistent_data)?;

		Ok(FrontendMessage::TriggerImaginateGenerate {
			parameters: Box::new(parameters),
			base_image: base_image.map(Box::new),
			mask_image: mask_image.map(Box::new),
			mask_paint_mode: if self.compute_input::<bool>(&network, &imaginate_node_path, get_imaginate_index("Inpaint"), Cow::Borrowed(&editor_api))? {
				ImaginateMaskPaintMode::Inpaint
			} else {
				ImaginateMaskPaintMode::Outpaint
			},
			mask_blur_px: self.compute_input::<f64>(&network, &imaginate_node_path, get_imaginate_index("Mask Blur"), Cow::Borrowed(&editor_api))? as u32,
			imaginate_mask_starting_fill: self.compute_input(&network, &imaginate_node_path, get_imaginate_index("Mask Starting Fill"), Cow::Borrowed(&editor_api))?,
			hostname: preferences.imaginate_server_hostname.clone(),
			refresh_frequency: preferences.imaginate_refresh_frequency,
			document_id,
			layer_path,
			node_path: imaginate_node_path,
		}
		.into())
	}

	/// Generate a new [`FrontendImageData`] from the [`Image`].
	fn to_frontend_image_data(image: Image<Color>, transform: Option<[f64; 6]>, layer_path: &[LayerId], node_id: Option<u64>, resize: Option<DVec2>) -> Result<FrontendImageData, String> {
		let (image_data, _size) = Self::encode_img(image, resize, image::ImageOutputFormat::Bmp)?;

		let mime = "image/bmp".to_string();
		let image_data = std::sync::Arc::new(image_data);

		Ok(FrontendImageData {
			path: layer_path.to_vec(),
			node_id,
			image_data,
			mime,
			transform,
		})
	}

	/// Evaluates a node graph, computing either the Imaginate node or the entire graph
	pub fn evaluate_node_graph(
		&mut self,
		(document_id, documents): (u64, &mut HashMap<u64, DocumentMessageHandler>),
		layer_path: Vec<LayerId>,
		(input_image_data, (width, height)): (Vec<u8>, (u32, u32)),
		imaginate_node: Option<Vec<NodeId>>,
		persistent_data: (&PreferencesMessageHandler, &PersistentData),
		responses: &mut VecDeque<Message>,
	) -> Result<(), String> {
		// Reformat the input image data into an RGBA f32 image
		let image = graphene_core::raster::Image::from_image_data(&input_image_data, width, height);

		// Get the node graph layer
		let document = documents.get_mut(&document_id).ok_or_else(|| "Invalid document".to_string())?;
		let layer = document.document_legacy.layer(&layer_path).map_err(|e| format!("No layer: {e:?}"))?;

		// Construct the input image frame
		let transform = DAffine2::IDENTITY;
		let image_frame = ImageFrame { image, transform };
		let editor_api = EditorApi {
			image_frame: Some(image_frame),
			font_cache: Some(&persistent_data.1.font_cache),
		};

		let layer_layer = match &layer.data {
			LayerDataType::Layer(layer) => Ok(layer),
			_ => Err("Invalid layer type".to_string()),
		}?;
		let network = layer_layer.network.clone();

		// Special execution path for generating Imaginate (as generation requires IO from outside node graph)
		if let Some(imaginate_node) = imaginate_node {
			responses.add(self.generate_imaginate(network, imaginate_node, (document, document_id), layer_path, editor_api, persistent_data)?);
			return Ok(());
		}
		// Execute the node graph
		let (network, monitor_nodes) = Self::wrap_network(network);
		let boxed_node_graph_output = self.execute_network(network, editor_api)?;

		// Check if the output is vector data
		if core::any::TypeId::of::<VectorData>() == DynAny::type_id(boxed_node_graph_output.as_ref()) {
			// Update the cached vector data on the layer
			let vector_data: VectorData = dyn_any::downcast(boxed_node_graph_output).map(|v| *v)?;
			let transform = vector_data.transform.to_cols_array();
			self.last_output_type.insert(layer_path.clone(), Some(concrete!(VectorData)));
			responses.add(Operation::SetLayerTransform { path: layer_path.clone(), transform });
			responses.add(Operation::SetVectorData { path: layer_path, vector_data });
		} else if core::any::TypeId::of::<ImageFrame<Color>>() == DynAny::type_id(boxed_node_graph_output.as_ref()) {
			// Attempt to downcast to an image frame
			let ImageFrame { image, transform } = dyn_any::downcast(boxed_node_graph_output).map(|image_frame| *image_frame)?;
			self.last_output_type.insert(layer_path.clone(), Some(concrete!(ImageFrame<Color>)));

			// Don't update the frame's transform if the new transform is DAffine2::ZERO.
			let transform = (!transform.abs_diff_eq(DAffine2::ZERO, f64::EPSILON)).then_some(transform.to_cols_array());

			// If no image was generated, clear the frame
			if image.width == 0 || image.height == 0 {
				responses.add(DocumentMessage::FrameClear);

				// Update the transform based on the graph output
				if let Some(transform) = transform {
					responses.add(Operation::SetLayerTransform { path: layer_path.clone(), transform });
				}
			} else {
				let image_data = vec![Self::to_frontend_image_data(image, transform, &layer_path, None, None)?];
				responses.add(FrontendMessage::UpdateImageData { document_id, image_data });
			}
		} else if core::any::TypeId::of::<graphene_core::Artboard>() == DynAny::type_id(boxed_node_graph_output.as_ref()) {
			let artboard: graphene_core::Artboard = dyn_any::downcast(boxed_node_graph_output).map(|artboard| *artboard)?;
			info!("{artboard:#?}");
			self.update_thumbnails(&layer_path, monitor_nodes, responses);

			return Err(format!("Artboard (see console)"));
		} else if core::any::TypeId::of::<graphene_core::GraphicGroup>() == DynAny::type_id(boxed_node_graph_output.as_ref()) {
			let graphic_group: graphene_core::GraphicGroup = dyn_any::downcast(boxed_node_graph_output).map(|graphic| *graphic)?;
			info!("{graphic_group:#?}");
			self.update_thumbnails(&layer_path, monitor_nodes, responses);

			return Err(format!("Graphic group (see console)"));
		}

		Ok(())
	}

	/// Recomputes the thumbnails for the layers in the graph, modifying the state and updating the UI.
	pub fn update_thumbnails(&mut self, layer_path: &[LayerId], monitor_nodes: Vec<Vec<u64>>, responses: &mut VecDeque<Message>) {
		let mut thumbnails_changed: bool = false;
		let mut image_data: Vec<_> = Vec::new();
		for node_path in monitor_nodes {
			let Some(value) = self.executor.introspect(&node_path).flatten() else {
				warn!("No introspect");
				continue;
			};
			let Some(graphic_group) = value.downcast_ref::<graphene_core::GraphicGroup>() else {
				warn!("Not graphic");
				continue;
			};
			use graphene_core::renderer::*;
			let bounds = graphic_group.bounding_box(DAffine2::IDENTITY);
			let render_params = RenderParams::new(ViewMode::Normal, bounds, true);
			let mut render = SvgRender::new();
			graphic_group.render_svg(&mut render, &render_params);
			let [min, max] = bounds.unwrap_or_default();
			render.format_svg(min, max);
			info!("SVG {}", render.svg);

			if let (Some(layer_id), Some(node_id)) = (layer_path.last().copied(), node_path.get(node_path.len() - 2).copied()) {
				let old_thumbnail = self.thumbnails.entry(layer_id).or_default().entry(node_id).or_default();
				if *old_thumbnail != render.svg {
					*old_thumbnail = render.svg;
					thumbnails_changed = true;
				}
			}
			let resize = Some(DVec2::splat(100.));
			let create_image_data = |(node_id, image)| Self::to_frontend_image_data(image, None, layer_path, Some(node_id), resize).ok();
			image_data.extend(render.image_data.into_iter().filter_map(create_image_data))
		}
		if !image_data.is_empty() {
			responses.add(FrontendMessage::UpdateImageData { document_id: 0, image_data });
		} else if thumbnails_changed {
			responses.add(NodeGraphMessage::SendGraph { should_rerender: false });
		}
	}

	/// When a blob url for a thumbnail is loaded, update the state and the UI.
	pub fn insert_thumbnail_bloburl(&mut self, blob_url: String, layer_id: LayerId, node_id: NodeId, responses: &mut VecDeque<Message>) {
		if let Some(layer) = self.thumbnails.get_mut(&layer_id) {
			if let Some(segment) = layer.values_mut().flat_map(|segments| segments.iter_mut()).find(|segment| **segment == SvgSegment::BlobUrl(node_id)) {
				*segment = SvgSegment::String(blob_url);
				responses.add(NodeGraphMessage::SendGraph { should_rerender: false });
				return;
			}
		}
	}
}
