use crate::messages::frontend::utility_types::FrontendImageData;
use crate::messages::portfolio::document::node_graph::wrap_network_in_scope;
use crate::messages::portfolio::document::utility_types::misc::DocumentRenderMode;
use crate::messages::portfolio::utility_types::PersistentData;
use crate::messages::prelude::*;

use document_legacy::{document::pick_safe_imaginate_resolution, layers::layer_info::LayerDataType};
use document_legacy::{LayerId, Operation};
use graph_craft::document::{generate_uuid, NodeId, NodeInput, NodeNetwork, NodeOutput};
use graph_craft::executor::Compiler;
use graphene_core::raster::{Image, ImageFrame};
use interpreted_executor::executor::DynamicExecutor;

use glam::{DAffine2, DVec2};
use std::borrow::Cow;

#[derive(Debug, Clone, Default)]
pub struct NodeGraphExecutor {
	executor: DynamicExecutor,
}

impl NodeGraphExecutor {
	/// Execute the network by flattening it and creating a borrow stack. Casts the output to the generic `T`.
	fn execute_network<T: dyn_any::StaticType>(&mut self, network: NodeNetwork, image_frame: ImageFrame) -> Result<T, String> {
		let mut scoped_network = wrap_network_in_scope(network);

		scoped_network.duplicate_outputs(&mut generate_uuid);
		scoped_network.remove_dead_nodes();

		debug!("Execute document network:\n{scoped_network:#?}");

		// We assume only one output
		assert_eq!(scoped_network.outputs.len(), 1, "Graph with multiple outputs not yet handled");
		let c = Compiler {};
		let proto_network = c.compile_single(scoped_network, true)?;
		debug!("Execute proto network:\n{proto_network}");
		assert_ne!(proto_network.nodes.len(), 0, "No protonodes exist?");
		if let Err(e) = self.executor.update(proto_network) {
			error!("Failed to update executor:\n{}", e);
			return Err(e);
		}

		use dyn_any::IntoDynAny;
		use graph_craft::executor::Executor;

		let boxed = self.executor.execute(image_frame.into_dyn()).map_err(|e| e.to_string())?;

		dyn_any::downcast::<T>(boxed).map(|v| *v)
	}

	/// Computes an input for a node in the graph
	pub fn compute_input<T: dyn_any::StaticType>(&mut self, old_network: &NodeNetwork, node_path: &[NodeId], mut input_index: usize, image_frame: Cow<ImageFrame>) -> Result<T, String> {
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
			}
		}

		self.execute_network(network, image_frame.into_owned())
	}

	/// Encodes an image into a format using the image crate
	fn encode_img(image: Image, resize: Option<DVec2>, format: image::ImageOutputFormat) -> Result<(Vec<u8>, (u32, u32)), String> {
		use image::{ImageBuffer, Rgba};
		use std::io::Cursor;

		let (result_bytes, width, height) = image.as_flat_u8();

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

	fn generate_imaginate(
		&mut self,
		network: NodeNetwork,
		imaginate_node: Vec<NodeId>,
		(document, document_id): (&mut DocumentMessageHandler, u64),
		layer_path: Vec<LayerId>,
		image_frame: ImageFrame,
		(preferences, persistent_data): (&PreferencesMessageHandler, &PersistentData),
	) -> Result<Message, String> {
		use crate::messages::portfolio::document::node_graph::IMAGINATE_NODE;
		use graph_craft::imaginate_input::*;

		let get = |name: &str| IMAGINATE_NODE.inputs.iter().position(|input| input.name == name).unwrap_or_else(|| panic!("Input {name} not found"));

		// Get the node graph layer
		let layer = document.document_legacy.layer(&layer_path).map_err(|e| format!("No layer: {e:?}"))?;
		let transform = layer.transform;

		let resolution: Option<glam::DVec2> = self.compute_input(&network, &imaginate_node, get("Resolution"), Cow::Borrowed(&image_frame))?;
		let resolution = resolution.unwrap_or_else(|| {
			let (x, y) = pick_safe_imaginate_resolution((transform.transform_vector2(DVec2::new(1., 0.)).length(), transform.transform_vector2(DVec2::new(0., 1.)).length()));
			DVec2::new(x as f64, y as f64)
		});

		let parameters = ImaginateGenerationParameters {
			seed: self.compute_input::<f64>(&network, &imaginate_node, get("Seed"), Cow::Borrowed(&image_frame))? as u64,
			resolution: resolution.as_uvec2().into(),
			samples: self.compute_input::<f64>(&network, &imaginate_node, get("Samples"), Cow::Borrowed(&image_frame))? as u32,
			sampling_method: self
				.compute_input::<ImaginateSamplingMethod>(&network, &imaginate_node, get("Sampling Method"), Cow::Borrowed(&image_frame))?
				.api_value()
				.to_string(),
			text_guidance: self.compute_input(&network, &imaginate_node, get("Prompt Guidance"), Cow::Borrowed(&image_frame))?,
			text_prompt: self.compute_input(&network, &imaginate_node, get("Prompt"), Cow::Borrowed(&image_frame))?,
			negative_prompt: self.compute_input(&network, &imaginate_node, get("Negative Prompt"), Cow::Borrowed(&image_frame))?,
			image_creativity: Some(self.compute_input::<f64>(&network, &imaginate_node, get("Image Creativity"), Cow::Borrowed(&image_frame))? / 100.),
			restore_faces: self.compute_input(&network, &imaginate_node, get("Improve Faces"), Cow::Borrowed(&image_frame))?,
			tiling: self.compute_input(&network, &imaginate_node, get("Tiling"), Cow::Borrowed(&image_frame))?,
		};
		let use_base_image = self.compute_input::<bool>(&network, &imaginate_node, get("Adapt Input Image"), Cow::Borrowed(&image_frame))?;

		let base_image = if use_base_image {
			let image: Image = self.compute_input(&network, &imaginate_node, get("Input Image"), Cow::Borrowed(&image_frame))?;
			// Only use if has size
			if image.width > 0 && image.height > 0 {
				let (image_data, size) = Self::encode_img(image, Some(resolution), image::ImageOutputFormat::Png)?;
				let size = DVec2::new(size.0 as f64, size.1 as f64);
				let mime = "image/png".to_string();
				Some(ImaginateBaseImage { image_data, size, mime })
			} else {
				None
			}
		} else {
			None
		};

		let mask_image = if base_image.is_some() {
			let mask_path: Option<Vec<LayerId>> = self.compute_input(&network, &imaginate_node, get("Masking Layer"), Cow::Borrowed(&image_frame))?;

			// Calculate the size of the node graph frame
			let size = DVec2::new(transform.transform_vector2(DVec2::new(1., 0.)).length(), transform.transform_vector2(DVec2::new(0., 1.)).length());

			// Render the masking layer within the node graph frame
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
			mask_image
		} else {
			None
		};

		Ok(FrontendMessage::TriggerImaginateGenerate {
			parameters: Box::new(parameters),
			base_image: base_image.map(Box::new),
			mask_image: mask_image.map(Box::new),
			mask_paint_mode: if self.compute_input::<bool>(&network, &imaginate_node, get("Inpaint"), Cow::Borrowed(&image_frame))? {
				ImaginateMaskPaintMode::Inpaint
			} else {
				ImaginateMaskPaintMode::Outpaint
			},
			mask_blur_px: self.compute_input::<f64>(&network, &imaginate_node, get("Mask Blur"), Cow::Borrowed(&image_frame))? as u32,
			imaginate_mask_starting_fill: self.compute_input(&network, &imaginate_node, get("Mask Starting Fill"), Cow::Borrowed(&image_frame))?,
			hostname: preferences.imaginate_server_hostname.clone(),
			refresh_frequency: preferences.imaginate_refresh_frequency,
			document_id,
			layer_path,
			node_path: imaginate_node,
		}
		.into())
	}

	/// Evaluates a node graph, computing either the imaginate node or the entire graph
	pub fn evaluate_node_graph(
		&mut self,
		(document_id, documents): (u64, &mut HashMap<u64, DocumentMessageHandler>),
		layer_path: Vec<LayerId>,
		(image_data, (width, height)): (Vec<u8>, (u32, u32)),
		imaginate_node: Option<Vec<NodeId>>,
		persistent_data: (&PreferencesMessageHandler, &PersistentData),
		responses: &mut VecDeque<Message>,
	) -> Result<(), String> {
		// Reformat the input image data into an f32 image
		let image = graphene_core::raster::Image::from_image_data(&image_data, width, height);

		// Get the node graph layer
		let document = documents.get_mut(&document_id).ok_or_else(|| "Invalid document".to_string())?;
		let layer = document.document_legacy.layer(&layer_path).map_err(|e| format!("No layer: {e:?}"))?;

		// Construct the input image frame
		let transform = layer.transform;
		let image_frame = ImageFrame { image, transform };

		let node_graph_frame = match &layer.data {
			LayerDataType::NodeGraphFrame(frame) => Ok(frame),
			_ => Err("Invalid layer type".to_string()),
		}?;
		let network = node_graph_frame.network.clone();

		// Execute the node graph
		if let Some(imaginate_node) = imaginate_node {
			responses.push_back(self.generate_imaginate(network, imaginate_node, (document, document_id), layer_path, image_frame, persistent_data)?);
		} else {
			let ImageFrame { image, transform } = self.execute_network(network, image_frame)?;

			// If no image was generated, clear the frame
			if image.width == 0 || image.height == 0 {
				responses.push_back(DocumentMessage::FrameClear.into());
			} else {
				// Update the image data
				let (image_data, _size) = Self::encode_img(image, None, image::ImageOutputFormat::Bmp)?;

				responses.push_back(
					Operation::SetNodeGraphFrameImageData {
						layer_path: layer_path.clone(),
						image_data: image_data.clone(),
					}
					.into(),
				);
				let mime = "image/bmp".to_string();
				let image_data = std::sync::Arc::new(image_data);
				let image_data = vec![FrontendImageData {
					path: layer_path.clone(),
					image_data,
					mime,
				}];
				responses.push_back(FrontendMessage::UpdateImageData { document_id, image_data }.into());
			}

			// Don't update the frame's transform if the new transform is DAffine2::ZERO.
			if !transform.abs_diff_eq(DAffine2::ZERO, f64::EPSILON) {
				// Update the transform based on the graph output
				let transform = transform.to_cols_array();
				responses.push_back(Operation::SetLayerTransform { path: layer_path.clone(), transform }.into());
				responses.push_back(Operation::SetLayerVisibility { path: layer_path, visible: true }.into());
			}
		}

		Ok(())
	}
}
