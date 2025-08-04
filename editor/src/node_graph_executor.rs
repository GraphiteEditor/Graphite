use crate::consts::FILE_SAVE_SUFFIX;
use crate::messages::frontend::utility_types::{ExportBounds, FileType};
use crate::messages::prelude::*;
use glam::{DAffine2, DVec2, UVec2};
use graph_craft::document::value::{RenderOutput, TaggedValue};
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeId, NodeInput};
use graph_craft::proto::GraphErrors;
use graph_craft::wasm_application_io::EditorPreferences;
use graphene_std::application_io::TimingInformation;
use graphene_std::application_io::{NodeGraphUpdateMessage, RenderConfig};
use graphene_std::renderer::RenderSvgSegmentList;
use graphene_std::renderer::{Render, RenderParams, SvgRender};
use graphene_std::renderer::{RenderMetadata, format_transform_matrix};
use graphene_std::text::FontCache;
use graphene_std::transform::Footprint;
use graphene_std::vector::Vector;
use graphene_std::vector::style::ViewMode;
use interpreted_executor::dynamic_executor::ResolvedDocumentNodeTypesDelta;

mod runtime_io;
pub use runtime_io::NodeRuntimeIO;

mod runtime;
pub use runtime::*;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ExecutionRequest {
	execution_id: u64,
	render_config: RenderConfig,
}

pub struct ExecutionResponse {
	execution_id: u64,
	result: Result<TaggedValue, String>,
	responses: VecDeque<FrontendMessage>,
	transform: DAffine2,
	vector_modify: HashMap<NodeId, Vector>,
	/// The resulting value from the temporary inspected during execution
	inspect_result: Option<InspectResult>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct CompilationResponse {
	result: Result<ResolvedDocumentNodeTypesDelta, String>,
	node_graph_errors: GraphErrors,
}

pub enum NodeGraphUpdate {
	ExecutionResponse(ExecutionResponse),
	CompilationResponse(CompilationResponse),
	NodeGraphUpdateMessage(NodeGraphUpdateMessage),
}

#[derive(Debug, Default)]
pub struct NodeGraphExecutor {
	runtime_io: NodeRuntimeIO,
	current_execution_id: u64,
	futures: HashMap<u64, ExecutionContext>,
	node_graph_hash: u64,
	old_inspect_node: Option<NodeId>,
}

#[derive(Debug, Clone)]
struct ExecutionContext {
	export_config: Option<ExportConfig>,
}

impl NodeGraphExecutor {
	/// A local runtime is useful on threads since having global state causes flakes
	#[cfg(test)]
	pub(crate) fn new_with_local_runtime() -> (NodeRuntime, Self) {
		let (request_sender, request_receiver) = std::sync::mpsc::channel();
		let (response_sender, response_receiver) = std::sync::mpsc::channel();
		let node_runtime = NodeRuntime::new(request_receiver, response_sender);

		let node_executor = Self {
			futures: Default::default(),
			runtime_io: NodeRuntimeIO::with_channels(request_sender, response_receiver),
			node_graph_hash: 0,
			current_execution_id: 0,
			old_inspect_node: None,
		};
		(node_runtime, node_executor)
	}
	/// Execute the network by flattening it and creating a borrow stack.
	fn queue_execution(&mut self, render_config: RenderConfig) -> u64 {
		let execution_id = self.current_execution_id;
		self.current_execution_id += 1;
		let request = ExecutionRequest { execution_id, render_config };
		self.runtime_io.send(GraphRuntimeRequest::ExecutionRequest(request)).expect("Failed to send generation request");

		execution_id
	}

	pub fn update_font_cache(&self, font_cache: FontCache) {
		self.runtime_io.send(GraphRuntimeRequest::FontCacheUpdate(font_cache)).expect("Failed to send font cache update");
	}

	pub fn update_editor_preferences(&self, editor_preferences: EditorPreferences) {
		self.runtime_io
			.send(GraphRuntimeRequest::EditorPreferencesUpdate(editor_preferences))
			.expect("Failed to send editor preferences");
	}

	/// Updates the network to monitor all inputs. Useful for the testing.
	#[cfg(test)]
	pub(crate) fn update_node_graph_instrumented(&mut self, document: &mut DocumentMessageHandler) -> Result<Instrumented, String> {
		// We should always invalidate the cache.
		self.node_graph_hash = crate::application::generate_uuid();
		let mut network = document.network_interface.document_network().clone();
		let instrumented = Instrumented::new(&mut network);

		self.runtime_io
			.send(GraphRuntimeRequest::GraphUpdate(GraphUpdate { network, inspect_node: None }))
			.map_err(|e| e.to_string())?;
		Ok(instrumented)
	}

	/// Update the cached network if necessary.
	fn update_node_graph(&mut self, document: &mut DocumentMessageHandler, inspect_node: Option<NodeId>, ignore_hash: bool) -> Result<(), String> {
		let network_hash = document.network_interface.document_network().current_hash();
		// Refresh the graph when it changes or the inspect node changes
		if network_hash != self.node_graph_hash || self.old_inspect_node != inspect_node || ignore_hash {
			let network = document.network_interface.document_network().clone();
			self.old_inspect_node = inspect_node;
			self.node_graph_hash = network_hash;

			self.runtime_io
				.send(GraphRuntimeRequest::GraphUpdate(GraphUpdate { network, inspect_node }))
				.map_err(|e| e.to_string())?;
		}
		Ok(())
	}

	/// Adds an evaluate request for whatever current network is cached.
	pub(crate) fn submit_current_node_graph_evaluation(&mut self, document: &mut DocumentMessageHandler, viewport_resolution: UVec2, time: TimingInformation) -> Result<Message, String> {
		let render_config = RenderConfig {
			viewport: Footprint {
				transform: document.metadata().document_to_viewport,
				resolution: viewport_resolution,
				..Default::default()
			},
			time,
			#[cfg(any(feature = "resvg", feature = "vello"))]
			export_format: graphene_std::application_io::ExportFormat::Canvas,
			#[cfg(not(any(feature = "resvg", feature = "vello")))]
			export_format: graphene_std::application_io::ExportFormat::Svg,
			view_mode: document.view_mode,
			hide_artboards: false,
			for_export: false,
		};

		// Execute the node graph
		let execution_id = self.queue_execution(render_config);

		self.futures.insert(execution_id, ExecutionContext { export_config: None });

		Ok(DeferMessage::SetGraphSubmissionIndex(execution_id).into())
	}

	/// Evaluates a node graph, computing the entire graph
	pub fn submit_node_graph_evaluation(
		&mut self,
		document: &mut DocumentMessageHandler,
		viewport_resolution: UVec2,
		time: TimingInformation,
		inspect_node: Option<NodeId>,
		ignore_hash: bool,
	) -> Result<Message, String> {
		self.update_node_graph(document, inspect_node, ignore_hash)?;
		self.submit_current_node_graph_evaluation(document, viewport_resolution, time)
	}

	/// Evaluates a node graph for export
	pub fn submit_document_export(&mut self, document: &mut DocumentMessageHandler, mut export_config: ExportConfig) -> Result<(), String> {
		let network = document.network_interface.document_network().clone();

		// Calculate the bounding box of the region to be exported
		let bounds = match export_config.bounds {
			ExportBounds::AllArtwork => document.network_interface.document_bounds_document_space(!export_config.transparent_background),
			ExportBounds::Selection => document.network_interface.selected_bounds_document_space(!export_config.transparent_background, &[]),
			ExportBounds::Artboard(id) => document.metadata().bounding_box_document(id),
		}
		.ok_or_else(|| "No bounding box".to_string())?;
		let size = bounds[1] - bounds[0];
		let transform = DAffine2::from_translation(bounds[0]).inverse();

		let render_config = RenderConfig {
			viewport: Footprint {
				transform: DAffine2::from_scale(DVec2::splat(export_config.scale_factor)) * transform,
				resolution: (size * export_config.scale_factor).as_uvec2(),
				..Default::default()
			},
			time: Default::default(),
			export_format: graphene_std::application_io::ExportFormat::Svg,
			view_mode: document.view_mode,
			hide_artboards: export_config.transparent_background,
			for_export: true,
		};
		export_config.size = size;

		// Execute the node graph
		self.runtime_io
			.send(GraphRuntimeRequest::GraphUpdate(GraphUpdate { network, inspect_node: None }))
			.map_err(|e| e.to_string())?;
		let execution_id = self.queue_execution(render_config);
		let execution_context = ExecutionContext { export_config: Some(export_config) };
		self.futures.insert(execution_id, execution_context);

		Ok(())
	}

	fn export(&self, node_graph_output: TaggedValue, export_config: ExportConfig, responses: &mut VecDeque<Message>) -> Result<(), String> {
		let TaggedValue::RenderOutput(RenderOutput {
			data: graphene_std::wasm_application_io::RenderOutputType::Svg { svg, .. },
			..
		}) = node_graph_output
		else {
			return Err("Incorrect render type for exporting (expected RenderOutput::Svg)".to_string());
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
		let results = self.runtime_io.receive().collect::<Vec<_>>();
		for response in results {
			match response {
				NodeGraphUpdate::ExecutionResponse(execution_response) => {
					let ExecutionResponse {
						execution_id,
						result,
						responses: existing_responses,
						transform,
						vector_modify,
						inspect_result,
					} = execution_response;

					responses.add(OverlaysMessage::Draw);

					let node_graph_output = match result {
						Ok(output) => output,
						Err(e) => {
							// Clear the click targets while the graph is in an un-renderable state
							document.network_interface.update_click_targets(HashMap::new());
							document.network_interface.update_vector_modify(HashMap::new());
							return Err(format!("Node graph evaluation failed:\n{e}"));
						}
					};

					responses.extend(existing_responses.into_iter().map(Into::into));
					document.network_interface.update_vector_modify(vector_modify);

					let execution_context = self.futures.remove(&execution_id).ok_or_else(|| "Invalid generation ID".to_string())?;
					if let Some(export_config) = execution_context.export_config {
						// Special handling for exporting the artwork
						self.export(node_graph_output, export_config, responses)?
					} else {
						self.process_node_graph_output(node_graph_output, transform, responses)?
					}
					responses.add(DeferMessage::TriggerGraphRun(execution_id));

					// Update the spreadsheet on the frontend using the value of the inspect result.
					if self.old_inspect_node.is_some() {
						if let Some(inspect_result) = inspect_result {
							responses.add(SpreadsheetMessage::UpdateLayout { inspect_result });
						}
					}
				}
				NodeGraphUpdate::CompilationResponse(execution_response) => {
					let CompilationResponse { node_graph_errors, result } = execution_response;
					let type_delta = match result {
						Err(e) => {
							// Clear the click targets while the graph is in an un-renderable state

							document.network_interface.update_click_targets(HashMap::new());
							document.network_interface.update_vector_modify(HashMap::new());

							log::trace!("{e}");

							responses.add(NodeGraphMessage::UpdateTypes {
								resolved_types: Default::default(),
								node_graph_errors,
							});
							responses.add(NodeGraphMessage::SendGraph);

							return Err(format!("Node graph evaluation failed:\n{e}"));
						}
						Ok(result) => result,
					};

					responses.add(NodeGraphMessage::UpdateTypes {
						resolved_types: type_delta,
						node_graph_errors,
					});
					responses.add(NodeGraphMessage::SendGraph);
				}
			}
		}
		Ok(())
	}

	fn debug_render(render_object: impl Render, transform: DAffine2, responses: &mut VecDeque<Message>) {
		// Setup rendering
		let mut render = SvgRender::new();
		let render_params = RenderParams {
			view_mode: ViewMode::Normal,
			culling_bounds: None,
			thumbnail: false,
			hide_artboards: false,
			for_export: false,
			for_mask: false,
			alignment_parent_transform: None,
		};

		// Render SVG
		render_object.render_svg(&mut render, &render_params);

		// Concatenate the defs and the SVG into one string
		render.wrap_with_transform(transform, None);
		let svg = render.svg.to_svg_string();

		// Send to frontend
		responses.add(FrontendMessage::UpdateDocumentArtwork { svg });
	}

	fn process_node_graph_output(&mut self, node_graph_output: TaggedValue, transform: DAffine2, responses: &mut VecDeque<Message>) -> Result<(), String> {
		let mut render_output_metadata = RenderMetadata::default();
		match node_graph_output {
			TaggedValue::RenderOutput(render_output) => {
				match render_output.data {
					graphene_std::wasm_application_io::RenderOutputType::Svg { svg, image_data } => {
						// Send to frontend
						responses.add(FrontendMessage::UpdateImageData { image_data });
						responses.add(FrontendMessage::UpdateDocumentArtwork { svg });
					}
					graphene_std::wasm_application_io::RenderOutputType::CanvasFrame(frame) => {
						let matrix = format_transform_matrix(frame.transform);
						let transform = if matrix.is_empty() { String::new() } else { format!(" transform=\"{matrix}\"") };
						let svg = format!(
							r#"<svg><foreignObject width="{}" height="{}"{transform}><div data-canvas-placeholder="{}"></div></foreignObject></svg>"#,
							frame.resolution.x, frame.resolution.y, frame.surface_id.0
						);
						responses.add(FrontendMessage::UpdateDocumentArtwork { svg });
					}
					graphene_std::wasm_application_io::RenderOutputType::Texture { .. } => {}
					_ => {
						return Err(format!("Invalid node graph output type: {:#?}", render_output.data));
					}
				}

				render_output_metadata = render_output.metadata;
			}
			TaggedValue::Bool(render_object) => Self::debug_render(render_object, transform, responses),
			TaggedValue::String(render_object) => Self::debug_render(render_object, transform, responses),
			TaggedValue::F64(render_object) => Self::debug_render(render_object, transform, responses),
			TaggedValue::DVec2(render_object) => Self::debug_render(render_object, transform, responses),
			TaggedValue::OptionalColor(render_object) => Self::debug_render(render_object, transform, responses),
			TaggedValue::Vector(render_object) => Self::debug_render(render_object, transform, responses),
			TaggedValue::Group(render_object) => Self::debug_render(render_object, transform, responses),
			TaggedValue::Raster(render_object) => Self::debug_render(render_object, transform, responses),
			TaggedValue::Palette(render_object) => Self::debug_render(render_object, transform, responses),
			_ => {
				return Err(format!("Invalid node graph output type: {node_graph_output:#?}"));
			}
		};
		let graphene_std::renderer::RenderMetadata {
			upstream_footprints: footprints,
			local_transforms,
			first_element_source_id,
			click_targets,
			clip_targets,
		} = render_output_metadata;

		// Run these update state messages immediately
		responses.add(DocumentMessage::UpdateUpstreamTransforms {
			upstream_footprints: footprints,
			local_transforms,
			first_element_source_id,
		});
		responses.add(DocumentMessage::UpdateClickTargets { click_targets });
		responses.add(DocumentMessage::UpdateClipTargets { clip_targets });
		responses.add(DocumentMessage::RenderScrollbars);
		responses.add(DocumentMessage::RenderRulers);
		responses.add(OverlaysMessage::Draw);
		Ok(())
	}
}

// Re-export for usage by tests in other modules
#[cfg(test)]
pub use test::Instrumented;

#[cfg(test)]
mod test {
	use std::sync::Arc;

	use super::*;
	use crate::messages::portfolio::document::utility_types::network_interface::NodeNetworkInterface;
	use crate::test_utils::test_prelude::{self, NodeGraphLayer};
	use graph_craft::ProtoNodeIdentifier;
	use graph_craft::document::NodeNetwork;
	use graphene_std::Context;
	use graphene_std::NodeInputDecleration;
	use graphene_std::memo::IORecord;
	use test_prelude::LayerNodeIdentifier;

	/// Stores all of the monitor nodes that have been attached to a graph
	#[derive(Default)]
	pub struct Instrumented {
		protonodes_by_name: HashMap<ProtoNodeIdentifier, Vec<Vec<Vec<NodeId>>>>,
		protonodes_by_path: HashMap<Vec<NodeId>, Vec<Vec<NodeId>>>,
	}

	impl Instrumented {
		/// Adds montior nodes to the network
		fn add(&mut self, network: &mut NodeNetwork, path: &mut Vec<NodeId>) {
			// Required to do seperately to satiate the borrow checker.
			let mut monitor_nodes = Vec::new();
			for (id, node) in network.nodes.iter_mut() {
				// Recursively instrument
				if let DocumentNodeImplementation::Network(nested) = &mut node.implementation {
					path.push(*id);
					self.add(nested, path);
					path.pop();
				}
				let mut monitor_node_ids = Vec::with_capacity(node.inputs.len());
				for input in &mut node.inputs {
					let node_id = NodeId::new();
					let old_input = std::mem::replace(input, NodeInput::node(node_id, 0));
					monitor_nodes.push((old_input, node_id));
					path.push(node_id);
					monitor_node_ids.push(path.clone());
					path.pop();
				}
				if let DocumentNodeImplementation::ProtoNode(identifier) = &mut node.implementation {
					path.push(*id);
					self.protonodes_by_name.entry(identifier.clone()).or_default().push(monitor_node_ids.clone());
					self.protonodes_by_path.insert(path.clone(), monitor_node_ids);
					path.pop();
				}
			}
			for (input, monitor_id) in monitor_nodes {
				let monitor_node = DocumentNode {
					inputs: vec![input],
					implementation: DocumentNodeImplementation::ProtoNode(graphene_std::memo::monitor::IDENTIFIER),
					manual_composition: Some(graph_craft::generic!(T)),
					skip_deduplication: true,
					..Default::default()
				};
				network.nodes.insert(monitor_id, monitor_node);
			}
		}

		/// Instrument a graph and return a new [Instrumented] state.
		pub fn new(network: &mut NodeNetwork) -> Self {
			let mut instrumented = Self::default();
			instrumented.add(network, &mut Vec::new());
			instrumented
		}

		fn downcast<Input: NodeInputDecleration>(dynamic: Arc<dyn std::any::Any + Send + Sync>) -> Option<Input::Result>
		where
			Input::Result: Send + Sync + Clone + 'static,
		{
			// This is quite inflexible since it only allows the footprint as inputs.
			if let Some(x) = dynamic.downcast_ref::<IORecord<(), Input::Result>>() {
				Some(x.output.clone())
			} else if let Some(x) = dynamic.downcast_ref::<IORecord<Footprint, Input::Result>>() {
				Some(x.output.clone())
			} else if let Some(x) = dynamic.downcast_ref::<IORecord<Context, Input::Result>>() {
				Some(x.output.clone())
			} else {
				panic!("cannot downcast type for introspection");
			}
		}

		/// Grab all of the values of the input every time it occurs in the graph.
		pub fn grab_all_input<'a, Input: NodeInputDecleration + 'a>(&'a self, runtime: &'a NodeRuntime) -> impl Iterator<Item = Input::Result> + 'a
		where
			Input::Result: Send + Sync + Clone + 'static,
		{
			self.protonodes_by_name
				.get(&Input::identifier())
				.map_or([].as_slice(), |x| x.as_slice())
				.iter()
				.filter_map(|inputs| inputs.get(Input::INDEX))
				.filter_map(|input_monitor_node| runtime.executor.introspect(input_monitor_node).ok())
				.filter_map(Instrumented::downcast::<Input>)
		}

		pub fn grab_protonode_input<Input: NodeInputDecleration>(&self, path: &Vec<NodeId>, runtime: &NodeRuntime) -> Option<Input::Result>
		where
			Input::Result: Send + Sync + Clone + 'static,
		{
			let input_monitor_node = self.protonodes_by_path.get(path)?.get(Input::INDEX)?;

			let dynamic = runtime.executor.introspect(input_monitor_node).ok()?;

			Self::downcast::<Input>(dynamic)
		}

		pub fn grab_input_from_layer<Input: NodeInputDecleration>(&self, layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface, runtime: &NodeRuntime) -> Option<Input::Result>
		where
			Input::Result: Send + Sync + Clone + 'static,
		{
			let node_graph_layer = NodeGraphLayer::new(layer, network_interface);
			let node = node_graph_layer.upstream_node_id_from_protonode(Input::identifier())?;
			self.grab_protonode_input::<Input>(&vec![node], runtime)
		}
	}
}
