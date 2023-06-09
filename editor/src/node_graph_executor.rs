use crate::messages::frontend::utility_types::FrontendImageData;
use crate::messages::portfolio::document::node_graph::wrap_network_in_scope;

use crate::messages::portfolio::utility_types::PersistentData;
use crate::messages::prelude::*;

use document_legacy::layers::layer_info::LayerDataType;
use document_legacy::{LayerId, Operation};

use graph_craft::document::value::TaggedValue;
use graph_craft::document::{generate_uuid, DocumentNodeImplementation, NodeId, NodeNetwork};
use graph_craft::graphene_compiler::Compiler;
use graph_craft::imaginate_input::ImaginatePreferences;
use graph_craft::{concrete, Type, TypeDescriptor};
use graphene_core::application_io::{ApplicationIo, NodeGraphUpdateMessage, NodeGraphUpdateSender};
use graphene_core::raster::{Image, ImageFrame};
use graphene_core::renderer::{SvgSegment, SvgSegmentList};
use graphene_core::text::FontCache;
use graphene_core::vector::style::ViewMode;

use graphene_core::{Color, SurfaceFrame, SurfaceId};
use graphene_std::wasm_application_io::{WasmApplicationIo, WasmEditorApi};
use interpreted_executor::dynamic_executor::DynamicExecutor;

use glam::{DAffine2, DVec2};
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

pub struct NodeRuntime {
	pub(crate) executor: DynamicExecutor,
	font_cache: FontCache,
	receiver: Receiver<NodeRuntimeMessage>,
	sender: InternalNodeGraphUpdateSender,
	wasm_io: Option<WasmApplicationIo>,
	imaginate_preferences: ImaginatePreferences,
	pub(crate) thumbnails: HashMap<LayerId, HashMap<NodeId, SvgSegmentList>>,
	canvas_cache: HashMap<Vec<LayerId>, SurfaceId>,
}

enum NodeRuntimeMessage {
	GenerationRequest(GenerationRequest),
	FontCacheUpdate(FontCache),
	ImaginatePreferencesUpdate(ImaginatePreferences),
}

pub(crate) struct GenerationRequest {
	generation_id: u64,
	graph: NodeNetwork,
	path: Vec<LayerId>,
	image_frame: Option<ImageFrame<Color>>,
}

pub(crate) struct GenerationResponse {
	generation_id: u64,
	result: Result<TaggedValue, String>,
	updates: VecDeque<Message>,
	new_thumbnails: HashMap<LayerId, HashMap<NodeId, SvgSegmentList>>,
}

enum NodeGraphUpdate {
	GenerationResponse(GenerationResponse),
	NodeGraphUpdateMessage(NodeGraphUpdateMessage),
}

struct InternalNodeGraphUpdateSender(Sender<NodeGraphUpdate>);

impl InternalNodeGraphUpdateSender {
	fn send_generation_response(&self, response: GenerationResponse) {
		self.0.send(NodeGraphUpdate::GenerationResponse(response)).expect("Failed to send response")
	}
}

impl NodeGraphUpdateSender for InternalNodeGraphUpdateSender {
	fn send(&self, message: NodeGraphUpdateMessage) {
		self.0.send(NodeGraphUpdate::NodeGraphUpdateMessage(message)).expect("Failed to send response")
	}
}

thread_local! {
	pub(crate) static NODE_RUNTIME: Rc<RefCell<Option<NodeRuntime>>> = Rc::new(RefCell::new(None));
}

impl NodeRuntime {
	fn new(receiver: Receiver<NodeRuntimeMessage>, sender: Sender<NodeGraphUpdate>) -> Self {
		let executor = DynamicExecutor::default();
		Self {
			executor,
			receiver,
			sender: InternalNodeGraphUpdateSender(sender),
			font_cache: FontCache::default(),
			imaginate_preferences: Default::default(),
			thumbnails: Default::default(),
			wasm_io: None,
			canvas_cache: Default::default(),
		}
	}
	pub async fn run(&mut self) {
		let mut requests = self.receiver.try_iter().collect::<Vec<_>>();
		// TODO: Currently we still render the document after we submit the node graph execution request.
		// This should be avoided in the future.
		requests.reverse();
		requests.dedup_by_key(|x| match x {
			NodeRuntimeMessage::GenerationRequest(x) => Some(x.path.clone()),
			_ => None,
		});
		requests.reverse();
		for request in requests {
			match request {
				NodeRuntimeMessage::FontCacheUpdate(font_cache) => self.font_cache = font_cache,
				NodeRuntimeMessage::ImaginatePreferencesUpdate(preferences) => self.imaginate_preferences = preferences,
				NodeRuntimeMessage::GenerationRequest(GenerationRequest {
					generation_id,
					graph,
					image_frame,
					path,
					..
				}) => {
					let (network, monitor_nodes) = Self::wrap_network(graph);

					let result = self.execute_network(&path, network, image_frame).await;
					let mut responses = VecDeque::new();
					self.update_thumbnails(&path, monitor_nodes, &mut responses);
					let response = GenerationResponse {
						generation_id,
						result,
						updates: responses,
						new_thumbnails: self.thumbnails.clone(),
					};
					self.sender.send_generation_response(response);
				}
			}
		}
	}

	/// Wraps a network in a scope and returns the new network and the paths to the monitor nodes.
	fn wrap_network(network: NodeNetwork) -> (NodeNetwork, Vec<Vec<NodeId>>) {
		let scoped_network = wrap_network_in_scope(network);

		//scoped_network.generate_node_paths(&[]);
		let monitor_nodes = scoped_network
			.recursive_nodes()
			.filter(|(node, _, _)| node.implementation == DocumentNodeImplementation::proto("graphene_std::memo::MonitorNode<_>"))
			.map(|(_, _, path)| path)
			.collect();

		(scoped_network, monitor_nodes)
	}

	async fn execute_network<'a>(&'a mut self, path: &[LayerId], scoped_network: NodeNetwork, image_frame: Option<ImageFrame<Color>>) -> Result<TaggedValue, String> {
		if self.wasm_io.is_none() {
			self.wasm_io = Some(WasmApplicationIo::new().await);
		}

		let editor_api = WasmEditorApi {
			font_cache: &self.font_cache,
			image_frame,
			application_io: &self.wasm_io.as_ref().unwrap(),
			node_graph_message_sender: &self.sender,
			imaginate_preferences: &self.imaginate_preferences,
		};

		// We assume only one output
		assert_eq!(scoped_network.outputs.len(), 1, "Graph with multiple outputs not yet handled");
		let c = Compiler {};
		let proto_network = c.compile_single(scoped_network, true)?;

		assert_ne!(proto_network.nodes.len(), 0, "No protonodes exist?");
		if let Err(e) = self.executor.update(proto_network).await {
			error!("Failed to update executor:\n{}", e);
			return Err(e);
		}

		use graph_craft::graphene_compiler::Executor;

		let result = match self.executor.input_type() {
			Some(t) if t == concrete!(WasmEditorApi) => (&self.executor).execute(editor_api).await.map_err(|e| e.to_string()),
			Some(t) if t == concrete!(()) => (&self.executor).execute(()).await.map_err(|e| e.to_string()),
			_ => Err("Invalid input type".to_string()),
		}?;

		if let TaggedValue::SurfaceFrame(SurfaceFrame { surface_id, transform: _ }) = result {
			let old_id = self.canvas_cache.insert(path.to_vec(), surface_id);
			if let Some(old_id) = old_id {
				if old_id != surface_id {
					self.wasm_io.as_ref().map(|io| io.destroy_surface(old_id));
				}
			}
		}
		Ok(result)
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
			let create_image_data = |(node_id, image)| NodeGraphExecutor::to_frontend_image_data(image, None, layer_path, Some(node_id), resize).ok();
			image_data.extend(render.image_data.into_iter().filter_map(create_image_data))
		}
		if !image_data.is_empty() {
			responses.add(FrontendMessage::UpdateImageData { document_id: 0, image_data });
		} else if thumbnails_changed {
			responses.add(NodeGraphMessage::SendGraph { should_rerender: false });
		}
	}
}
pub fn introspect_node(path: &[NodeId]) -> Option<Arc<dyn std::any::Any>> {
	NODE_RUNTIME
		.try_with(|runtime| {
			let runtime = runtime.try_borrow();
			if let Ok(ref runtime) = runtime {
				if let Some(ref mut runtime) = runtime.as_ref() {
					return runtime.executor.introspect(path).flatten();
				}
			}
			None
		})
		.unwrap_or(None)
}

pub async fn run_node_graph() {
	let result = NODE_RUNTIME.try_with(|runtime| {
		let runtime = runtime.clone();
		async move {
			let mut runtime = runtime.try_borrow_mut();
			if let Ok(ref mut runtime) = runtime {
				if let Some(ref mut runtime) = runtime.as_mut() {
					runtime.run().await;
				}
			}
		}
	});
	if let Ok(result) = result {
		result.await;
	}
}

#[derive(Debug)]
pub struct NodeGraphExecutor {
	sender: Sender<NodeRuntimeMessage>,
	receiver: Receiver<NodeGraphUpdate>,
	// TODO: This is a memory leak since layers are never removed
	pub(crate) last_output_type: HashMap<Vec<LayerId>, Option<Type>>,
	pub(crate) thumbnails: HashMap<LayerId, HashMap<NodeId, SvgSegmentList>>,
	futures: HashMap<u64, ExecutionContext>,
}

#[derive(Debug, Clone)]
struct ExecutionContext {
	layer_path: Vec<LayerId>,
	document_id: u64,
}

impl Default for NodeGraphExecutor {
	fn default() -> Self {
		let (request_sender, request_reciever) = std::sync::mpsc::channel();
		let (response_sender, response_reciever) = std::sync::mpsc::channel();
		NODE_RUNTIME.with(|runtime| {
			runtime.borrow_mut().replace(NodeRuntime::new(request_reciever, response_sender));
		});

		Self {
			futures: Default::default(),
			sender: request_sender,
			receiver: response_reciever,
			last_output_type: Default::default(),
			thumbnails: Default::default(),
		}
	}
}

impl NodeGraphExecutor {
	/// Execute the network by flattening it and creating a borrow stack.
	fn queue_execution(&self, network: NodeNetwork, image_frame: Option<ImageFrame<Color>>, layer_path: Vec<LayerId>) -> u64 {
		let generation_id = generate_uuid();
		let request = GenerationRequest {
			path: layer_path,
			graph: network,
			image_frame,
			generation_id,
		};
		self.sender.send(NodeRuntimeMessage::GenerationRequest(request)).expect("Failed to send generation request");

		generation_id
	}

	pub fn introspect_node(&self, path: &[NodeId]) -> Option<Arc<dyn std::any::Any>> {
		introspect_node(path)
	}

	pub fn update_font_cache(&self, font_cache: FontCache) {
		self.sender.send(NodeRuntimeMessage::FontCacheUpdate(font_cache)).expect("Failed to send font cache update");
	}

	pub fn update_imaginate_preferences(&self, imaginate_preferences: ImaginatePreferences) {
		self.sender
			.send(NodeRuntimeMessage::ImaginatePreferencesUpdate(imaginate_preferences))
			.expect("Failed to send imaginate preferences");
	}

	pub fn previous_output_type(&self, path: &[LayerId]) -> Option<Type> {
		self.last_output_type.get(path).cloned().flatten()
	}

	pub fn introspect_node_in_network<T: std::any::Any + core::fmt::Debug, U, F1: FnOnce(&NodeNetwork) -> Option<NodeId>, F2: FnOnce(&T) -> U>(
		&mut self,
		network: &NodeNetwork,
		node_path: &[NodeId],
		find_node: F1,
		extract_data: F2,
	) -> Option<U> {
		let wrapping_document_node = network.nodes.get(node_path.last()?)?;
		let DocumentNodeImplementation::Network(wrapped_network) = &wrapping_document_node.implementation else { return None; };
		let introspection_node = find_node(&wrapped_network)?;
		let introspection = self.introspect_node(&[node_path, &[introspection_node]].concat())?;
		let downcasted: &T = <dyn std::any::Any>::downcast_ref(introspection.as_ref())?;
		Some(extract_data(downcasted))
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

	/// Evaluates a node graph, computing the entire graph
	pub fn submit_node_graph_evaluation(
		&mut self,
		(document_id, documents): (u64, &mut HashMap<u64, DocumentMessageHandler>),
		layer_path: Vec<LayerId>,
		(input_image_data, (width, height)): (Vec<u8>, (u32, u32)),
		_persistent_data: (&PreferencesMessageHandler, &PersistentData),
		_responses: &mut VecDeque<Message>,
	) -> Result<(), String> {
		// Reformat the input image data into an RGBA f32 image
		let image = graphene_core::raster::Image::from_image_data(&input_image_data, width, height);

		// Get the node graph layer
		let document = documents.get_mut(&document_id).ok_or_else(|| "Invalid document".to_string())?;
		let network = if layer_path.is_empty() {
			document.document_legacy.document_network.clone()
		} else {
			let layer = document.document_legacy.layer(&layer_path).map_err(|e| format!("No layer: {e:?}"))?;

			let layer_layer = match &layer.data {
				LayerDataType::Layer(layer) => Ok(layer),
				_ => Err("Invalid layer type".to_string()),
			}?;
			layer_layer.network.clone()
		};

		// Construct the input image frame
		let transform = DAffine2::IDENTITY;
		let image_frame = ImageFrame { image, transform };

		// Execute the node graph
		let generation_id = self.queue_execution(network, Some(image_frame), layer_path.clone());

		self.futures.insert(generation_id, ExecutionContext { layer_path, document_id });

		Ok(())
	}

	pub fn poll_node_graph_evaluation(&mut self, responses: &mut VecDeque<Message>) -> Result<(), String> {
		let results = self.receiver.try_iter().collect::<Vec<_>>();
		for response in results {
			match response {
				NodeGraphUpdate::GenerationResponse(GenerationResponse {
					generation_id,
					result,
					updates,
					new_thumbnails,
				}) => {
					self.thumbnails = new_thumbnails;
					let node_graph_output = result.map_err(|e| format!("Node graph evaluation failed: {:?}", e))?;
					let execution_context = self.futures.remove(&generation_id).ok_or_else(|| "Invalid generation ID".to_string())?;
					responses.extend(updates);
					self.process_node_graph_output(node_graph_output, execution_context.layer_path.clone(), responses, execution_context.document_id)?;
					responses.add(DocumentMessage::LayerChanged {
						affected_layer_path: execution_context.layer_path,
					});
					responses.add(DocumentMessage::RenderDocument);
					responses.add(ArtboardMessage::RenderArtboards);
					responses.add(DocumentMessage::DocumentStructureChanged);
					responses.add(BroadcastEvent::DocumentIsDirty);
					responses.add(DocumentMessage::DirtyRenderDocument);
					responses.add(DocumentMessage::Overlays(OverlaysMessage::Rerender));
				}
				NodeGraphUpdate::NodeGraphUpdateMessage(NodeGraphUpdateMessage::ImaginateStatusUpdate) => {
					responses.add(DocumentMessage::PropertiesPanel(PropertiesPanelMessage::ResendActiveProperties))
				}
			}
		}
		Ok(())
	}

	fn process_node_graph_output(&mut self, node_graph_output: TaggedValue, layer_path: Vec<LayerId>, responses: &mut VecDeque<Message>, document_id: u64) -> Result<(), String> {
		self.last_output_type.insert(layer_path.clone(), Some(node_graph_output.ty()));
		match node_graph_output {
			TaggedValue::VectorData(vector_data) => {
				// Update the cached vector data on the layer
				let transform = vector_data.transform.to_cols_array();
				responses.add(Operation::SetLayerTransform { path: layer_path.clone(), transform });
				responses.add(Operation::SetVectorData { path: layer_path, vector_data });
			}
			TaggedValue::SurfaceFrame(SurfaceFrame { surface_id, transform }) => {
				let transform = transform.to_cols_array();
				responses.add(Operation::SetLayerTransform { path: layer_path.clone(), transform });
				responses.add(Operation::SetSurface { path: layer_path, surface_id });
			}
			TaggedValue::ImageFrame(ImageFrame { image, transform }) => {
				// Don't update the frame's transform if the new transform is DAffine2::ZERO.
				let transform = (!transform.abs_diff_eq(DAffine2::ZERO, f64::EPSILON)).then_some(transform.to_cols_array());

				// If no image was generated, clear the frame
				if image.width == 0 || image.height == 0 {
					responses.add(DocumentMessage::FrameClear);

					// Update the transform based on the graph output
					if let Some(transform) = transform {
						responses.add(Operation::SetLayerTransform { path: layer_path, transform });
					}
				} else {
					// Update the image data
					let image_data = vec![Self::to_frontend_image_data(image, transform, &layer_path, None, None)?];
					responses.add(FrontendMessage::UpdateImageData { document_id, image_data });
				}
			}
			TaggedValue::Artboard(artboard) => {
				info!("{artboard:#?}");
				return Err("Artboard (see console)".to_string());
			}
			TaggedValue::GraphicGroup(graphic_group) => {
				info!("{graphic_group:#?}");
				return Err("Graphic group (see console)".to_string());
			}
			_ => {
				return Err(format!("Invalid node graph output type: {:#?}", node_graph_output));
			}
		};
		Ok(())
	}

	/// When a blob url for a thumbnail is loaded, update the state and the UI.
	pub fn insert_thumbnail_blob_url(&mut self, blob_url: String, layer_id: LayerId, node_id: NodeId, responses: &mut VecDeque<Message>) {
		if let Some(layer) = self.thumbnails.get_mut(&layer_id) {
			if let Some(segment) = layer.values_mut().flat_map(|segments| segments.iter_mut()).find(|segment| **segment == SvgSegment::BlobUrl(node_id)) {
				*segment = SvgSegment::String(blob_url);
				responses.add(NodeGraphMessage::SendGraph { should_rerender: false });
			}
		}
	}
}
