use crate::messages::frontend::utility_types::FrontendImageData;
use crate::messages::portfolio::document::node_graph::wrap_network_in_scope;
use crate::messages::portfolio::document::utility_types::misc::{LayerMetadata, LayerPanelEntry};
use crate::messages::prelude::*;

use document_legacy::document::Document as DocumentLegacy;
use document_legacy::document_metadata::LayerNodeIdentifier;
use document_legacy::layers::layer_info::{LayerDataType, LayerDataTypeDiscriminant};
use document_legacy::{LayerId, Operation};

use graph_craft::document::value::TaggedValue;
use graph_craft::document::{generate_uuid, DocumentNodeImplementation, NodeId, NodeNetwork};
use graph_craft::graphene_compiler::Compiler;
use graph_craft::imaginate_input::ImaginatePreferences;
use graph_craft::{concrete, Type};
use graphene_core::application_io::{ApplicationIo, NodeGraphUpdateMessage, NodeGraphUpdateSender, RenderConfig};
use graphene_core::memo::IORecord;
use graphene_core::raster::{Image, ImageFrame};
use graphene_core::renderer::{ClickTarget, GraphicElementRendered, SvgSegment, SvgSegmentList};
use graphene_core::text::FontCache;
use graphene_core::transform::{Footprint, Transform};
use graphene_core::vector::style::ViewMode;
use graphene_core::vector::VectorData;

use graphene_core::{Color, GraphicElement, SurfaceFrame, SurfaceId};
use graphene_std::wasm_application_io::{WasmApplicationIo, WasmEditorApi};
use interpreted_executor::dynamic_executor::DynamicExecutor;

use glam::{DAffine2, DVec2, UVec2};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

/// Identifies a node graph, either the document graph or a node graph associated with a legacy layer.
#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub enum GraphIdentifier {
	DocumentGraph,
	LayerGraph(LayerId),
}

impl GraphIdentifier {
	pub const fn new(layer_id: Option<LayerId>) -> Self {
		match layer_id {
			Some(layer_id) => Self::LayerGraph(layer_id),
			None => Self::DocumentGraph,
		}
	}
}

pub struct NodeRuntime {
	pub(crate) executor: DynamicExecutor,
	font_cache: FontCache,
	receiver: Receiver<NodeRuntimeMessage>,
	sender: InternalNodeGraphUpdateSender,
	wasm_io: Option<WasmApplicationIo>,
	imaginate_preferences: ImaginatePreferences,
	pub(crate) thumbnails: HashMap<NodeId, SvgSegmentList>,
	pub(crate) click_targets: HashMap<NodeId, Vec<ClickTarget>>,
	pub(crate) upstream_transforms: HashMap<NodeId, (Footprint, DAffine2)>,
	graph_hash: Option<u64>,
	canvas_cache: HashMap<Vec<LayerId>, SurfaceId>,
	monitor_nodes: Vec<Vec<NodeId>>,
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
	render_config: RenderConfig,
}

pub(crate) struct GenerationResponse {
	generation_id: u64,
	result: Result<TaggedValue, String>,
	updates: VecDeque<Message>,
	new_thumbnails: HashMap<NodeId, SvgSegmentList>,
	new_click_targets: HashMap<LayerNodeIdentifier, Vec<ClickTarget>>,
	new_upstream_transforms: HashMap<NodeId, (Footprint, DAffine2)>,
	transform: DAffine2,
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
			canvas_cache: HashMap::new(),
			click_targets: HashMap::new(),
			graph_hash: None,
			upstream_transforms: HashMap::new(),
			monitor_nodes: Vec::new(),
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
					render_config,
					path,
					..
				}) => {
					let transform = render_config.viewport.transform;
					let result = self.execute_network(&path, graph, render_config).await;
					let mut responses = VecDeque::new();

					self.update_thumbnails(&path, &mut responses);
					self.update_upstream_transforms();

					let response = GenerationResponse {
						generation_id,
						result,
						updates: responses,
						new_thumbnails: self.thumbnails.clone(),
						new_click_targets: self.click_targets.clone().into_iter().map(|(id, targets)| (LayerNodeIdentifier::new_unchecked(id), targets)).collect(),
						new_upstream_transforms: self.upstream_transforms.clone(),
						transform,
					};
					self.sender.send_generation_response(response);
				}
			}
		}
	}

	async fn execute_network<'a>(&'a mut self, path: &[LayerId], graph: NodeNetwork, render_config: RenderConfig) -> Result<TaggedValue, String> {
		if self.wasm_io.is_none() {
			self.wasm_io = Some(WasmApplicationIo::new().await);
		}
		let editor_api = WasmEditorApi {
			font_cache: &self.font_cache,
			application_io: self.wasm_io.as_ref().unwrap(),
			node_graph_message_sender: &self.sender,
			imaginate_preferences: &self.imaginate_preferences,
			render_config,
			image_frame: None,
		};

		use std::collections::hash_map::DefaultHasher;
		use std::hash::Hash;
		use std::hash::Hasher;
		// Required to ensure that the appropriate protonodes are reinserted when the Editor API changes.
		let mut graph_input_hash = DefaultHasher::new();
		editor_api.font_cache.hash(&mut graph_input_hash);
		let font_hash_code = graph_input_hash.finish();
		graph.hash(&mut graph_input_hash);
		let hash_code = graph_input_hash.finish();

		if self.graph_hash != Some(hash_code) {
			self.graph_hash = None;
		}

		if self.graph_hash.is_none() {
			let scoped_network = wrap_network_in_scope(graph, font_hash_code);

			self.monitor_nodes = scoped_network
				.recursive_nodes()
				.filter(|(_, node)| node.implementation == DocumentNodeImplementation::proto("graphene_core::memo::MonitorNode<_, _, _>"))
				.map(|(_, node)| node.path.clone().unwrap_or_default())
				.collect::<Vec<_>>();

			// We assume only one output
			assert_eq!(scoped_network.outputs.len(), 1, "Graph with multiple outputs not yet handled");
			let c = Compiler {};
			let proto_network = match c.compile_single(scoped_network) {
				Ok(network) => network,
				Err(e) => return Err(e),
			};

			assert_ne!(proto_network.nodes.len(), 0, "No protonodes exist?");
			if let Err(e) = self.executor.update(proto_network).await {
				error!("Failed to update executor:\n{e}");
				return Err(e);
			}

			self.graph_hash = Some(hash_code);
		}

		use graph_craft::graphene_compiler::Executor;

		let result = match self.executor.input_type() {
			Some(t) if t == concrete!(WasmEditorApi) => (&self.executor).execute(editor_api).await.map_err(|e| e.to_string()),
			Some(t) if t == concrete!(()) => (&self.executor).execute(()).await.map_err(|e| e.to_string()),
			Some(t) => Err(format!("Invalid input type {t:?}")),
			_ => Err("No input type".to_string()),
		};
		let result = match result {
			Ok(value) => value,
			Err(e) => return Err(e),
		};

		if let TaggedValue::SurfaceFrame(SurfaceFrame { surface_id, transform: _ }) = result {
			let old_id = self.canvas_cache.insert(path.to_vec(), surface_id);
			if let Some(old_id) = old_id {
				if old_id != surface_id {
					if let Some(io) = self.wasm_io.as_ref() {
						io.destroy_surface(old_id)
					}
				}
			}
		}
		Ok(result)
	}

	/// Recomputes the thumbnails for the layers in the graph, modifying the state and updating the UI.
	pub fn update_thumbnails(&mut self, layer_path: &[LayerId], responses: &mut VecDeque<Message>) {
		let mut image_data: Vec<_> = Vec::new();
		self.thumbnails.retain(|id, _| self.monitor_nodes.iter().any(|node_path| node_path.contains(id)));
		for node_path in &self.monitor_nodes {
			let Some(node_id) = node_path.get(node_path.len() - 2).copied() else {
				warn!("Monitor node has invalid node id");
				continue;
			};
			let Some(value) = self.executor.introspect(node_path).flatten() else {
				warn!("Failed to introspect monitor node for thumbnail");
				continue;
			};

			let Some(io_data) = value.downcast_ref::<IORecord<Footprint, graphene_core::GraphicElement>>() else {
				continue;
			};
			let graphic_element = &io_data.output;
			use graphene_core::renderer::*;
			let bounds = graphic_element.bounding_box(DAffine2::IDENTITY);
			let render_params = RenderParams::new(ViewMode::Normal, ImageRenderMode::BlobUrl, bounds, true);
			let mut render = SvgRender::new();
			graphic_element.render_svg(&mut render, &render_params);
			let [min, max] = bounds.unwrap_or_default();
			render.format_svg(min, max);

			let click_targets = self.click_targets.entry(node_id).or_default();
			click_targets.clear();
			// Add the graphic element data's click targets to the click targets vector
			graphic_element.add_click_targets(click_targets);

			let old_thumbnail = self.thumbnails.entry(node_id).or_default();
			if *old_thumbnail != render.svg {
				responses.add(FrontendMessage::UpdateNodeThumbnail {
					id: node_id,
					value: render.svg.to_string(),
				});
				*old_thumbnail = render.svg;
			}

			let resize = Some(DVec2::splat(100.));
			let create_image_data = |(node_id, image)| NodeGraphExecutor::to_frontend_image_data(image, None, layer_path, Some(node_id), resize).ok();
			image_data.extend(render.image_data.into_iter().filter_map(create_image_data))
		}
		if !image_data.is_empty() {
			responses.add(FrontendMessage::UpdateImageData { document_id: 0, image_data });
		}
	}

	pub fn update_upstream_transforms(&mut self) {
		for node_path in &self.monitor_nodes {
			let Some(node_id) = node_path.get(node_path.len() - 2).copied() else {
				warn!("Monitor node has invalid node id");
				continue;
			};
			let Some(value) = self.executor.introspect(node_path).flatten() else {
				warn!("Failed to introspect monitor node for upstream transforms");
				continue;
			};

			fn try_downcast<T: Transform + 'static>(value: &dyn std::any::Any) -> Option<(Footprint, DAffine2)> {
				let io_data = value.downcast_ref::<IORecord<Footprint, T>>()?;
				let transform = io_data.output.transform();
				Some((io_data.input, transform))
			}

			let Some(transform) = try_downcast::<VectorData>(value.as_ref())
				.or_else(|| try_downcast::<ImageFrame<Color>>(value.as_ref()))
				.or_else(|| try_downcast::<GraphicElement>(value.as_ref()))
			else {
				warn!("Failed to downcast transform input");
				continue;
			};
			self.upstream_transforms.insert(node_id, transform);
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
	pub(crate) thumbnails: HashMap<NodeId, SvgSegmentList>,
	futures: HashMap<u64, ExecutionContext>,
}

#[derive(Debug, Clone)]
struct ExecutionContext {
	layer_path: Vec<LayerId>,
}

impl Default for NodeGraphExecutor {
	fn default() -> Self {
		let (request_sender, request_receiver) = std::sync::mpsc::channel();
		let (response_sender, response_receiver) = std::sync::mpsc::channel();
		NODE_RUNTIME.with(|runtime| {
			runtime.borrow_mut().replace(NodeRuntime::new(request_receiver, response_sender));
		});

		Self {
			futures: Default::default(),
			sender: request_sender,
			receiver: response_receiver,
			last_output_type: Default::default(),
			thumbnails: Default::default(),
		}
	}
}

impl NodeGraphExecutor {
	/// Execute the network by flattening it and creating a borrow stack.
	fn queue_execution(&self, network: NodeNetwork, layer_path: Vec<LayerId>, render_config: RenderConfig) -> u64 {
		let generation_id = generate_uuid();
		let request = GenerationRequest {
			path: layer_path,
			graph: network,
			generation_id,
			render_config,
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
		let DocumentNodeImplementation::Network(wrapped_network) = &wrapping_document_node.implementation else {
			return None;
		};
		let introspection_node = find_node(wrapped_network)?;
		let introspection = self.introspect_node(&[node_path, &[introspection_node]].concat())?;
		let downcasted: &T = <dyn std::any::Any>::downcast_ref(introspection.as_ref())?;
		Some(extract_data(downcasted))
	}

	/// Encodes an image into a format using the image crate
	fn encode_img(image: Image<Color>, resize: Option<DVec2>, format: image::ImageOutputFormat) -> Result<(Vec<u8>, (u32, u32)), String> {
		use image::{ImageBuffer, Rgba};
		use std::io::Cursor;

		let (result_bytes, width, height) = image.to_flat_u8();

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
	pub fn submit_node_graph_evaluation(&mut self, document: &mut DocumentMessageHandler, layer_path: Vec<LayerId>, viewport_resolution: UVec2) -> Result<(), String> {
		// Get the node graph layer
		let network = if layer_path.is_empty() {
			document.network().clone()
		} else {
			let layer = document.document_legacy.layer(&layer_path).map_err(|e| format!("No layer: {e:?}"))?;

			let layer_layer = match &layer.data {
				LayerDataType::Layer(layer) => Ok(layer),
				_ => Err("Invalid layer type".to_string()),
			}?;
			layer_layer.network.clone()
		};

		let render_config = RenderConfig {
			viewport: Footprint {
				transform: document.document_legacy.metadata.document_to_viewport,
				resolution: viewport_resolution,
				..Default::default()
			},
			#[cfg(any(feature = "resvg", feature = "vello"))]
			export_format: graphene_core::application_io::ExportFormat::Canvas,
			#[cfg(not(any(feature = "resvg", feature = "vello")))]
			export_format: graphene_core::application_io::ExportFormat::Svg,
			view_mode: document.view_mode,
		};

		// Execute the node graph
		let generation_id = self.queue_execution(network, layer_path.clone(), render_config);

		self.futures.insert(generation_id, ExecutionContext { layer_path });

		Ok(())
	}

	pub fn poll_node_graph_evaluation(&mut self, document: &mut DocumentLegacy, responses: &mut VecDeque<Message>) -> Result<(), String> {
		let results = self.receiver.try_iter().collect::<Vec<_>>();
		for response in results {
			match response {
				NodeGraphUpdate::GenerationResponse(GenerationResponse {
					generation_id,
					result,
					updates,
					new_thumbnails,
					new_click_targets,
					new_upstream_transforms,
					transform,
				}) => {
					for (&node_id, svg) in &new_thumbnails {
						if !document.document_network.nodes.contains_key(&node_id) {
							warn!("Missing node");
							continue;
						}
						let layer = LayerNodeIdentifier::new(node_id, &document.document_network);
						responses.add(FrontendMessage::UpdateDocumentLayerDetails {
							data: LayerPanelEntry {
								name: document.document_network.nodes.get(&node_id).map(|node| node.alias.clone()).unwrap_or_default(),
								tooltip: if cfg!(debug_assertions) { format!("Layer ID: {node_id}") } else { "".into() },
								visible: !document.document_network.disabled.contains(&layer.to_node()),
								layer_type: if document.metadata.is_artboard(layer) {
									LayerDataTypeDiscriminant::Artboard
								} else if document.metadata.is_folder(layer) {
									LayerDataTypeDiscriminant::Folder
								} else {
									LayerDataTypeDiscriminant::Layer
								},
								layer_metadata: LayerMetadata {
									expanded: layer.has_children(&document.metadata) && !document.collapsed_folders.contains(&layer),
									selected: document.metadata.selected_layers_contains(layer),
								},
								path: vec![node_id],
								thumbnail: svg.to_string(),
							},
						});
					}
					self.thumbnails = new_thumbnails;
					document.metadata.update_transforms(new_upstream_transforms);
					document.metadata.update_click_targets(new_click_targets);
					let node_graph_output = result.map_err(|e| format!("Node graph evaluation failed: {e:?}"))?;
					let execution_context = self.futures.remove(&generation_id).ok_or_else(|| "Invalid generation ID".to_string())?;
					responses.extend(updates);
					self.process_node_graph_output(node_graph_output, execution_context.layer_path.clone(), transform, responses)?;
					responses.add(DocumentMessage::LayerChanged {
						affected_layer_path: execution_context.layer_path,
					});
					responses.add(DocumentMessage::RenderDocument);
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

	fn render(render_object: impl GraphicElementRendered, transform: DAffine2, responses: &mut VecDeque<Message>) {
		use graphene_core::renderer::{ImageRenderMode, RenderParams, SvgRender};

		// Setup rendering
		let mut render = SvgRender::new();
		let render_params = RenderParams::new(ViewMode::Normal, ImageRenderMode::BlobUrl, None, false);

		// Render SVG
		render_object.render_svg(&mut render, &render_params);

		// Concatenate the defs and the SVG into one string
		render.wrap_with_transform(transform);
		let svg = render.svg.to_string();

		// Send to frontend
		responses.add(FrontendMessage::UpdateDocumentNodeRender { svg });
	}

	fn process_node_graph_output(&mut self, node_graph_output: TaggedValue, layer_path: Vec<LayerId>, transform: DAffine2, responses: &mut VecDeque<Message>) -> Result<(), String> {
		self.last_output_type.insert(layer_path.clone(), Some(node_graph_output.ty()));
		match node_graph_output {
			TaggedValue::SurfaceFrame(SurfaceFrame { surface_id, transform }) => {
				let transform = transform.to_cols_array();
				responses.add(Operation::SetLayerTransform { path: layer_path.clone(), transform });
				responses.add(Operation::SetSurface { path: layer_path, surface_id });
			}
			TaggedValue::RenderOutput(graphene_std::wasm_application_io::RenderOutput::Svg(svg)) => {
				// Send to frontend
				responses.add(FrontendMessage::UpdateDocumentNodeRender { svg });
				responses.add(DocumentMessage::RenderScrollbars);
			}
			TaggedValue::RenderOutput(graphene_std::wasm_application_io::RenderOutput::CanvasFrame(frame)) => {
				// Send to frontend
				responses.add(DocumentMessage::RenderScrollbars);
				let matrix = frame
					.transform
					.to_cols_array()
					.iter()
					.enumerate()
					.fold(String::new(), |val, (i, entry)| val + &(entry.to_string() + if i == 5 { "" } else { "," }));
				let svg = format!(
					r#"
					<svg><foreignObject width="{}" height="{}" transform="matrix({})"><div data-canvas-placeholder="canvas{}"></div></foreignObject></svg>
					"#,
					1920, 1080, matrix, frame.surface_id.0
				);
				responses.add(FrontendMessage::UpdateDocumentNodeRender { svg });
			}
			TaggedValue::Bool(render_object) => Self::render(render_object, transform, responses),
			TaggedValue::String(render_object) => Self::render(render_object, transform, responses),
			TaggedValue::F32(render_object) => Self::render(render_object, transform, responses),
			TaggedValue::F64(render_object) => Self::render(render_object, transform, responses),
			TaggedValue::OptionalColor(render_object) => Self::render(render_object, transform, responses),
			TaggedValue::VectorData(render_object) => Self::render(render_object, transform, responses),
			TaggedValue::ImageFrame(render_object) => Self::render(render_object, transform, responses),
			_ => {
				return Err(format!("Invalid node graph output type: {node_graph_output:#?}"));
			}
		};
		Ok(())
	}

	/// When a blob url for a thumbnail is loaded, update the state and the UI.
	pub fn insert_thumbnail_blob_url(&mut self, blob_url: String, node_id: NodeId, responses: &mut VecDeque<Message>) {
		for segment_list in self.thumbnails.values_mut() {
			if let Some(segment) = segment_list.iter_mut().find(|segment| **segment == SvgSegment::BlobUrl(node_id)) {
				*segment = SvgSegment::String(blob_url);
				responses.add(FrontendMessage::UpdateNodeThumbnail {
					id: node_id,
					value: segment_list.to_string(),
				});
				return;
			}
		}
		warn!("Received blob url for invalid segment")
	}
}
