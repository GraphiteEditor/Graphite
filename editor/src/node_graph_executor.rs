use crate::consts::FILE_SAVE_SUFFIX;
use crate::messages::frontend::utility_types::FrontendImageData;
use crate::messages::frontend::utility_types::{ExportBounds, FileType};
use crate::messages::portfolio::document::node_graph::wrap_network_in_scope;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::layer_panel::LayerClassification;
use crate::messages::portfolio::document::utility_types::misc::LayerPanelEntry;
use crate::messages::prelude::*;

use graph_craft::concrete;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{generate_uuid, DocumentNodeImplementation, NodeId, NodeNetwork};
use graph_craft::graphene_compiler::Compiler;
use graph_craft::imaginate_input::ImaginatePreferences;
use graph_craft::proto::{GraphErrors, ProtoNetwork};
use graphene_core::application_io::{NodeGraphUpdateMessage, NodeGraphUpdateSender, RenderConfig};
use graphene_core::memo::IORecord;
use graphene_core::raster::{Image, ImageFrame};
use graphene_core::renderer::{ClickTarget, GraphicElementRendered, SvgSegmentList};
use graphene_core::text::FontCache;
use graphene_core::transform::{Footprint, Transform};
use graphene_core::vector::style::ViewMode;
use graphene_core::vector::VectorData;
use graphene_core::{Color, GraphicElement, SurfaceFrame};
use graphene_std::wasm_application_io::{WasmApplicationIo, WasmEditorApi};
use interpreted_executor::dynamic_executor::{DynamicExecutor, ResolvedDocumentNodeTypes};

use glam::{DAffine2, DVec2, UVec2};
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
	pub(crate) thumbnails: HashMap<NodeId, SvgSegmentList>,
	pub(crate) click_targets: HashMap<NodeId, Vec<ClickTarget>>,
	pub(crate) upstream_transforms: HashMap<NodeId, (Footprint, DAffine2)>,
	pub(crate) resolved_types: ResolvedDocumentNodeTypes,
	pub(crate) node_graph_errors: GraphErrors,
	graph_hash: Option<u64>,
	monitor_nodes: Vec<Vec<NodeId>>,
}

enum NodeRuntimeMessage {
	GenerationRequest(GenerationRequest),
	FontCacheUpdate(FontCache),
	ImaginatePreferencesUpdate(ImaginatePreferences),
}

#[derive(Default, Debug, Clone)]
pub struct ExportConfig {
	pub file_name: String,
	pub file_type: FileType,
	pub scale_factor: f64,
	pub bounds: ExportBounds,
	pub transparent_background: bool,
	pub size: DVec2,
}

pub(crate) struct GenerationRequest {
	generation_id: u64,
	graph: NodeNetwork,
	render_config: RenderConfig,
}

pub(crate) struct GenerationResponse {
	generation_id: u64,
	result: Result<TaggedValue, String>,
	updates: VecDeque<Message>,
	new_thumbnails: HashMap<NodeId, SvgSegmentList>,
	new_click_targets: HashMap<LayerNodeIdentifier, Vec<ClickTarget>>,
	new_upstream_transforms: HashMap<NodeId, (Footprint, DAffine2)>,
	resolved_types: ResolvedDocumentNodeTypes,
	node_graph_errors: GraphErrors,
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
			click_targets: HashMap::new(),
			graph_hash: None,
			upstream_transforms: HashMap::new(),
			resolved_types: ResolvedDocumentNodeTypes::default(),
			node_graph_errors: Vec::new(),
			monitor_nodes: Vec::new(),
		}
	}
	pub async fn run(&mut self) {
		let mut requests = self.receiver.try_iter().collect::<Vec<_>>();
		// TODO: Currently we still render the document after we submit the node graph execution request.
		// This should be avoided in the future.
		requests.reverse();
		requests.dedup_by_key(|x| match x {
			NodeRuntimeMessage::GenerationRequest(x) => Some(x.graph.current_hash()),
			_ => None,
		});
		requests.reverse();
		for request in requests {
			match request {
				NodeRuntimeMessage::FontCacheUpdate(font_cache) => self.font_cache = font_cache,
				NodeRuntimeMessage::ImaginatePreferencesUpdate(preferences) => self.imaginate_preferences = preferences,
				NodeRuntimeMessage::GenerationRequest(GenerationRequest {
					generation_id, graph, render_config, ..
				}) => {
					let transform = render_config.viewport.transform;
					let result = self.execute_network(graph, render_config).await;
					let mut responses = VecDeque::new();

					self.update_thumbnails(&mut responses);
					self.update_upstream_transforms();

					let response = GenerationResponse {
						generation_id,
						result,
						updates: responses,
						new_thumbnails: self.thumbnails.clone(),
						new_click_targets: self.click_targets.clone().into_iter().map(|(id, targets)| (LayerNodeIdentifier::new_unchecked(id), targets)).collect(),
						new_upstream_transforms: self.upstream_transforms.clone(),
						resolved_types: self.resolved_types.clone(),
						node_graph_errors: core::mem::take(&mut self.node_graph_errors),
						transform,
					};
					self.sender.send_generation_response(response);
				}
			}
		}
	}

	async fn execute_network<'a>(&'a mut self, graph: NodeNetwork, render_config: RenderConfig) -> Result<TaggedValue, String> {
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

		let mut proto_network = ProtoNetwork::default();
		let mut scoped_network = NodeNetwork::default();
		let typing_context_serded = serde_json::to_string(&self.executor.typing_context.inferred).unwrap();
		if self.graph_hash.is_none() {
			scoped_network = wrap_network_in_scope(graph.clone(), font_hash_code);

			self.monitor_nodes = scoped_network
				.recursive_nodes()
				.filter(|(_, node)| node.implementation == DocumentNodeImplementation::proto("graphene_core::memo::MonitorNode<_, _, _>"))
				.map(|(_, node)| node.original_location.path.clone().unwrap_or_default())
				.collect::<Vec<_>>();

			// We assume only one output
			assert_eq!(scoped_network.outputs.len(), 1, "Graph with multiple outputs not yet handled");
			let c = Compiler {};
			proto_network = c.compile_single(scoped_network.clone())?;

			assert_ne!(proto_network.nodes.len(), 0, "No protonodes exist?");
			if let Err(e) = self.executor.update(proto_network.clone()).await {
				self.node_graph_errors = e;
			} else {
				self.graph_hash = Some(hash_code);
			}
			self.resolved_types = self.executor.document_node_types();
		}

		use graph_craft::graphene_compiler::Executor;

		let hook = std::panic::take_hook();
		std::panic::set_hook(Box::new({
			let proto_network = proto_network.clone();
			let graph = graph.clone();
			move |info| {
				error!("Panic whilst executing {proto_network:#?} \n\ndocument: {graph:#?}\n\n{info:?}");
			}
		}));
		let result = match self.executor.input_type() {
			Some(t) if t == concrete!(WasmEditorApi) => (&self.executor).execute(editor_api).await.map_err(|e| e.to_string()),
			Some(t) if t == concrete!(()) => (&self.executor).execute(()).await.map_err(|e| e.to_string()),
			Some(t) => Err(format!("Invalid input type {t:?}")),
			_ => {
				let scoped_network_serded = serde_json::to_string(&scoped_network).unwrap();
				let proto_network_serded = serde_json::to_string(&proto_network).unwrap();
				Err(format!(
					"No input type \n\ntypes: {typing_context_serded}\n\nproto: {proto_network_serded}\n\ndocument: {scoped_network_serded}"
				))
			}
		};
		std::panic::set_hook(hook);
		let result = match result {
			Ok(value) => value,
			Err(e) => return Err(e),
		};

		// if let TaggedValue::SurfaceFrame(SurfaceFrame { surface_id, transform: _ }) = result {
		// 	let old_id = self.canvas_cache.insert(path.to_vec(), surface_id);
		// 	if let Some(old_id) = old_id {
		// 		if old_id != surface_id {
		// 			if let Some(io) = self.wasm_io.as_ref() {
		// 				io.destroy_surface(old_id)
		// 			}
		// 		}
		// 	}
		// }
		Ok(result)
	}

	/// Recomputes the thumbnails for the layers in the graph, modifying the state and updating the UI.
	pub fn update_thumbnails(&mut self, responses: &mut VecDeque<Message>) {
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
			let render_params = RenderParams::new(ViewMode::Normal, ImageRenderMode::BlobUrl, bounds, true, false, false);
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
			image_data.extend(render.image_data.into_iter().filter_map(|(_, image)| NodeGraphExecutor::to_frontend_image_data(image, resize).ok()))
		}
		if !image_data.is_empty() {
			responses.add(FrontendMessage::UpdateImageData {
				document_id: DocumentId(0),
				image_data,
			});
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
	futures: HashMap<u64, ExecutionContext>,
}

#[derive(Debug, Clone)]
struct ExecutionContext {
	export_config: Option<ExportConfig>,
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
		}
	}
}

impl NodeGraphExecutor {
	/// Execute the network by flattening it and creating a borrow stack.
	fn queue_execution(&self, network: NodeNetwork, render_config: RenderConfig) -> u64 {
		let generation_id = generate_uuid();
		let request = GenerationRequest {
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
		let Some(downcasted): Option<&T> = <dyn std::any::Any>::downcast_ref(introspection.as_ref()) else {
			log::warn!("Failed to downcast type for introspection");
			return None;
		};
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
	fn to_frontend_image_data(image: Image<Color>, resize: Option<DVec2>) -> Result<FrontendImageData, String> {
		let (image_data, _size) = Self::encode_img(image, resize, image::ImageOutputFormat::Bmp)?;

		let mime = "image/bmp".to_string();
		let image_data = std::sync::Arc::new(image_data);

		Ok(FrontendImageData { image_data, mime })
	}

	/// Evaluates a node graph, computing the entire graph
	pub fn submit_node_graph_evaluation(&mut self, document: &mut DocumentMessageHandler, viewport_resolution: UVec2) -> Result<(), String> {
		// Get the node graph layer
		let network = document.network().clone();

		let render_config = RenderConfig {
			viewport: Footprint {
				transform: document.metadata.document_to_viewport,
				resolution: viewport_resolution,
				..Default::default()
			},
			#[cfg(any(feature = "resvg", feature = "vello"))]
			export_format: graphene_core::application_io::ExportFormat::Canvas,
			#[cfg(not(any(feature = "resvg", feature = "vello")))]
			export_format: graphene_core::application_io::ExportFormat::Svg,
			view_mode: document.view_mode,
			hide_artboards: false,
			for_export: false,
		};

		// Execute the node graph
		let generation_id = self.queue_execution(network, render_config);

		self.futures.insert(generation_id, ExecutionContext { export_config: None });

		Ok(())
	}

	/// Evaluates a node graph for export
	pub fn submit_document_export(&mut self, document: &mut DocumentMessageHandler, mut export_config: ExportConfig) -> Result<(), String> {
		let network = document.network().clone();

		// Calculate the bounding box of the region to be exported
		let bounds = match export_config.bounds {
			ExportBounds::AllArtwork => document.metadata().document_bounds_document_space(!export_config.transparent_background),
			ExportBounds::Selection => document.metadata().selected_bounds_document_space(!export_config.transparent_background),
			ExportBounds::Artboard(id) => document.metadata().bounding_box_document(id),
		}
		.ok_or_else(|| "No bounding box".to_string())?;
		let size = bounds[1] - bounds[0];
		let transform = DAffine2::from_translation(bounds[0]).inverse();

		let render_config = RenderConfig {
			viewport: Footprint {
				transform,
				resolution: (size * export_config.scale_factor).as_uvec2(),
				..Default::default()
			},
			export_format: graphene_core::application_io::ExportFormat::Svg,
			view_mode: document.view_mode,
			hide_artboards: export_config.transparent_background,
			for_export: true,
		};
		export_config.size = size;

		// Execute the node graph
		let generation_id = self.queue_execution(network, render_config);
		let execution_context = ExecutionContext { export_config: Some(export_config) };
		self.futures.insert(generation_id, execution_context);

		Ok(())
	}

	fn export(&self, node_graph_output: TaggedValue, export_config: ExportConfig, responses: &mut VecDeque<Message>) -> Result<(), String> {
		let TaggedValue::RenderOutput(graphene_std::wasm_application_io::RenderOutput::Svg(svg)) = node_graph_output else {
			return Err("Incorrect render type for exportign (expected RenderOutput::Svg)".to_string());
		};

		let ExportConfig {
			file_type,
			file_name,
			size,
			scale_factor,
			..
		} = export_config;

		let file_suffix = &format!(".{file_type:?}").to_lowercase();
		let name = match file_name.ends_with(FILE_SAVE_SUFFIX) {
			true => file_name.replace(FILE_SAVE_SUFFIX, file_suffix),
			false => file_name + file_suffix,
		};

		if file_type == FileType::Svg {
			responses.add(FrontendMessage::TriggerDownloadTextFile { document: svg, name });
		} else {
			let mime = file_type.to_mime().to_string();
			let size = (size * scale_factor).into();
			responses.add(FrontendMessage::TriggerDownloadImage { svg, name, mime, size });
		}
		Ok(())
	}

	pub fn poll_node_graph_evaluation(&mut self, document: &mut DocumentMessageHandler, responses: &mut VecDeque<Message>) -> Result<(), String> {
		let DocumentMessageHandler {
			network: document_network,
			metadata: document_metadata,
			collapsed,
			..
		} = document;

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
					resolved_types,
					node_graph_errors,
					transform,
				}) => {
					responses.add(NodeGraphMessage::UpdateTypes { resolved_types, node_graph_errors });
					let node_graph_output = result.map_err(|e| format!("Node graph evaluation failed: {e}"))?;
					let execution_context = self.futures.remove(&generation_id).ok_or_else(|| "Invalid generation ID".to_string())?;

					if let Some(export_config) = execution_context.export_config {
						return self.export(node_graph_output, export_config, responses);
					}

					for (&node_id, svg) in &new_thumbnails {
						if !document_network.nodes.contains_key(&node_id) {
							warn!("Missing node");
							continue;
						}
						let layer = LayerNodeIdentifier::new(node_id, document_network);
						responses.add(FrontendMessage::UpdateDocumentLayerDetails {
							data: LayerPanelEntry {
								name: document_network.nodes.get(&node_id).map(|node| node.alias.clone()).unwrap_or_default(),
								tooltip: if cfg!(debug_assertions) { format!("Layer ID: {node_id}") } else { "".into() },
								layer_classification: if document_metadata.is_artboard(layer) {
									LayerClassification::Artboard
								} else if document_metadata.is_folder(layer) {
									LayerClassification::Folder
								} else {
									LayerClassification::Layer
								},
								expanded: layer.has_children(document_metadata) && !collapsed.contains(&layer),
								selected: document_metadata.selected_layers_contains(layer),
								parent_id: layer.parent(document_metadata).map(|parent| parent.to_node()),
								id: node_id,
								depth: layer.ancestors(document_metadata).count() - 1,
								thumbnail: svg.to_string(),
							},
						});
					}
					document_metadata.update_transforms(new_upstream_transforms);
					document_metadata.update_click_targets(new_click_targets);
					responses.extend(updates);
					self.process_node_graph_output(node_graph_output, transform, responses)?;
					responses.add(DocumentMessage::RenderDocument);
					responses.add(DocumentMessage::DocumentStructureChanged);
					responses.add(BroadcastEvent::DocumentIsDirty);
					responses.add(OverlaysMessage::Draw);
				}
				NodeGraphUpdate::NodeGraphUpdateMessage(NodeGraphUpdateMessage::ImaginateStatusUpdate) => responses.add(DocumentMessage::PropertiesPanel(PropertiesPanelMessage::Refresh)),
			}
		}
		Ok(())
	}

	fn render(render_object: impl GraphicElementRendered, transform: DAffine2, responses: &mut VecDeque<Message>) {
		use graphene_core::renderer::{ImageRenderMode, RenderParams, SvgRender};

		// Setup rendering
		let mut render = SvgRender::new();
		let render_params = RenderParams::new(ViewMode::Normal, ImageRenderMode::BlobUrl, None, false, false, false);

		// Render SVG
		render_object.render_svg(&mut render, &render_params);

		// Concatenate the defs and the SVG into one string
		render.wrap_with_transform(transform, None);
		let svg = render.svg.to_string();

		// Send to frontend
		responses.add(FrontendMessage::UpdateDocumentArtwork { svg });
	}

	fn process_node_graph_output(&mut self, node_graph_output: TaggedValue, transform: DAffine2, responses: &mut VecDeque<Message>) -> Result<(), String> {
		match node_graph_output {
			TaggedValue::SurfaceFrame(SurfaceFrame { surface_id: _, transform: _ }) => {
				// TODO: Reimplement this now that document-legacy is gone
			}
			TaggedValue::RenderOutput(graphene_std::wasm_application_io::RenderOutput::Svg(svg)) => {
				// Send to frontend
				responses.add(FrontendMessage::UpdateDocumentArtwork { svg });
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
				responses.add(FrontendMessage::UpdateDocumentArtwork { svg });
			}
			TaggedValue::Bool(render_object) => Self::render(render_object, transform, responses),
			TaggedValue::String(render_object) => Self::render(render_object, transform, responses),
			TaggedValue::F32(render_object) => Self::render(render_object, transform, responses),
			TaggedValue::F64(render_object) => Self::render(render_object, transform, responses),
			TaggedValue::OptionalColor(render_object) => Self::render(render_object, transform, responses),
			TaggedValue::VectorData(render_object) => Self::render(render_object, transform, responses),
			TaggedValue::GraphicGroup(render_object) => Self::render(render_object, transform, responses),
			TaggedValue::Artboard(render_object) => Self::render(render_object, transform, responses),
			TaggedValue::ImageFrame(render_object) => Self::render(render_object, transform, responses),
			TaggedValue::Palette(render_object) => Self::render(render_object, transform, responses),
			_ => {
				return Err(format!("Invalid node graph output type: {node_graph_output:#?}"));
			}
		};
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	#[tokio::test]
	async fn intermittent_crash() {
		let val = include_str!("test_net.txt");
		let typing_context = val.split_once("types: ").unwrap().1.split_once("\n").unwrap().0;
		let document = val.split_once("document: ").unwrap().1.split_once("\n").unwrap().0;
		println!("{val:?}");
		let inferred = serde_json::from_str(typing_context).unwrap();
		let scoped_network: NodeNetwork = serde_json::from_str(document).unwrap();

		let mut executor = DynamicExecutor::default();
		executor.typing_context.inferred = inferred;
		let c = Compiler {};
		let proto_network = c.compile_single(scoped_network).unwrap();

		assert_ne!(proto_network.nodes.len(), 0, "No protonodes exist?");
		executor.update(proto_network.clone()).await.unwrap();
		println!("{:?}", executor.input_type());
	}
}
