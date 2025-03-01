use crate::consts::FILE_SAVE_SUFFIX;
use crate::messages::frontend::utility_types::{ExportBounds, FileType};
use crate::messages::prelude::*;

use graph_craft::document::value::{RenderOutput, TaggedValue};
use graph_craft::document::{generate_uuid, DocumentNodeImplementation, NodeId, NodeNetwork};
use graph_craft::proto::GraphErrors;
use graph_craft::wasm_application_io::EditorPreferences;
use graphene_core::application_io::{NodeGraphUpdateMessage, RenderConfig};
use graphene_core::renderer::RenderSvgSegmentList;
use graphene_core::renderer::{GraphicElementRendered, ImageRenderMode, RenderParams, SvgRender};
use graphene_core::text::FontCache;
use graphene_core::transform::Footprint;
use graphene_core::vector::style::ViewMode;
use graphene_core::Context;
use graphene_std::renderer::{format_transform_matrix, RenderMetadata};
use graphene_std::vector::VectorData;
use interpreted_executor::dynamic_executor::{IntrospectError, ResolvedDocumentNodeTypesDelta};

use glam::{DAffine2, DVec2, UVec2};
use std::sync::Arc;

mod runtime_io;
pub use runtime_io::NodeRuntimeIO;

mod runtime;
pub use runtime::*;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ExecutionRequest {
	execution_id: u64,
	render_config: RenderConfig,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ExecutionResponse {
	execution_id: u64,
	result: Result<TaggedValue, String>,
	responses: VecDeque<FrontendMessage>,
	transform: DAffine2,
	vector_modify: HashMap<NodeId, VectorData>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct CompilationResponse {
	result: Result<ResolvedDocumentNodeTypesDelta, String>,
	node_graph_errors: GraphErrors,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub enum NodeGraphUpdate {
	ExecutionResponse(ExecutionResponse),
	CompilationResponse(CompilationResponse),
	NodeGraphUpdateMessage(NodeGraphUpdateMessage),
}

#[derive(Debug)]
pub struct NodeGraphExecutor {
	runtime_io: NodeRuntimeIO,
	futures: HashMap<u64, ExecutionContext>,
	node_graph_hash: u64,
}

#[derive(Debug, Clone)]
struct ExecutionContext {
	export_config: Option<ExportConfig>,
}

impl Default for NodeGraphExecutor {
	fn default() -> Self {
		Self {
			futures: Default::default(),
			runtime_io: NodeRuntimeIO::new(),
			node_graph_hash: 0,
		}
	}
}

impl NodeGraphExecutor {
	/// Execute the network by flattening it and creating a borrow stack.
	fn queue_execution(&self, render_config: RenderConfig) -> u64 {
		let execution_id = generate_uuid();
		let request = ExecutionRequest { execution_id, render_config };
		self.runtime_io.send(NodeRuntimeMessage::ExecutionRequest(request)).expect("Failed to send generation request");

		execution_id
	}

	pub async fn introspect_node(&self, path: &[NodeId]) -> Result<Arc<dyn std::any::Any>, IntrospectError> {
		introspect_node(path).await
	}

	pub fn update_font_cache(&self, font_cache: FontCache) {
		self.runtime_io.send(NodeRuntimeMessage::FontCacheUpdate(font_cache)).expect("Failed to send font cache update");
	}

	pub fn update_editor_preferences(&self, editor_preferences: EditorPreferences) {
		self.runtime_io
			.send(NodeRuntimeMessage::EditorPreferencesUpdate(editor_preferences))
			.expect("Failed to send editor preferences");
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
		let introspection = futures::executor::block_on(self.introspect_node(&[node_path, &[introspection_node]].concat())).ok()?;
		let Some(downcasted): Option<&T> = <dyn std::any::Any>::downcast_ref(introspection.as_ref()) else {
			log::warn!("Failed to downcast type for introspection");
			return None;
		};
		Some(extract_data(downcasted))
	}

	/// Evaluates a node graph, computing the entire graph
	pub fn submit_node_graph_evaluation(&mut self, document: &mut DocumentMessageHandler, viewport_resolution: UVec2, ignore_hash: bool) -> Result<(), String> {
		// Get the node graph layer
		let network_hash = document.network_interface.network(&[]).unwrap().current_hash();
		if network_hash != self.node_graph_hash || ignore_hash {
			self.node_graph_hash = network_hash;
			self.runtime_io
				.send(NodeRuntimeMessage::GraphUpdate(document.network_interface.network(&[]).unwrap().clone()))
				.map_err(|e| e.to_string())?;
		}

		let render_config = RenderConfig {
			viewport: Footprint {
				transform: document.metadata().document_to_viewport,
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
		let execution_id = self.queue_execution(render_config);

		self.futures.insert(execution_id, ExecutionContext { export_config: None });

		Ok(())
	}

	/// Evaluates a node graph for export
	pub fn submit_document_export(&mut self, document: &mut DocumentMessageHandler, mut export_config: ExportConfig) -> Result<(), String> {
		let network = document.network_interface.network(&[]).unwrap().clone();

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
			export_format: graphene_core::application_io::ExportFormat::Svg,
			view_mode: document.view_mode,
			hide_artboards: export_config.transparent_background,
			for_export: true,
		};
		export_config.size = size;

		// Execute the node graph
		self.runtime_io.send(NodeRuntimeMessage::GraphUpdate(network)).map_err(|e| e.to_string())?;
		let execution_id = self.queue_execution(render_config);
		let execution_context = ExecutionContext { export_config: Some(export_config) };
		self.futures.insert(execution_id, execution_context);

		Ok(())
	}

	fn export(&self, node_graph_output: TaggedValue, export_config: ExportConfig, responses: &mut VecDeque<Message>) -> Result<(), String> {
		let TaggedValue::RenderOutput(RenderOutput {
			data: graphene_std::wasm_application_io::RenderOutputType::Svg(svg),
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
				NodeGraphUpdate::NodeGraphUpdateMessage(NodeGraphUpdateMessage::ImaginateStatusUpdate) => {
					responses.add(DocumentMessage::PropertiesPanel(PropertiesPanelMessage::Refresh));
				}
			}
		}
		Ok(())
	}

	fn debug_render(render_object: impl GraphicElementRendered, transform: DAffine2, responses: &mut VecDeque<Message>) {
		// Setup rendering
		let mut render = SvgRender::new();
		let render_params = RenderParams::new(ViewMode::Normal, ImageRenderMode::Base64, None, false, false, false);

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
					graphene_std::wasm_application_io::RenderOutputType::Svg(svg) => {
						// Send to frontend
						responses.add(FrontendMessage::UpdateDocumentArtwork { svg });
					}
					graphene_std::wasm_application_io::RenderOutputType::CanvasFrame(frame) => {
						let matrix = format_transform_matrix(frame.transform);
						let transform = if matrix.is_empty() { String::new() } else { format!(" transform=\"{}\"", matrix) };
						let svg = format!(
							r#"<svg><foreignObject width="{}" height="{}"{transform}><div data-canvas-placeholder="canvas{}"></div></foreignObject></svg>"#,
							frame.resolution.x, frame.resolution.y, frame.surface_id.0
						);
						responses.add(FrontendMessage::UpdateDocumentArtwork { svg });
					}
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
			TaggedValue::VectorData(render_object) => Self::debug_render(render_object, transform, responses),
			TaggedValue::GraphicGroup(render_object) => Self::debug_render(render_object, transform, responses),
			TaggedValue::ImageFrame(render_object) => Self::debug_render(render_object, transform, responses),
			TaggedValue::Palette(render_object) => Self::debug_render(render_object, transform, responses),
			_ => {
				return Err(format!("Invalid node graph output type: {node_graph_output:#?}"));
			}
		};
		responses.add(Message::EndBuffer(render_output_metadata));
		responses.add(DocumentMessage::RenderScrollbars);
		responses.add(DocumentMessage::RenderRulers);
		responses.add(OverlaysMessage::Draw);
		Ok(())
	}
}
