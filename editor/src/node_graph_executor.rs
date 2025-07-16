use std::sync::Arc;

use crate::consts::FILE_SAVE_SUFFIX;
use crate::messages::frontend::utility_types::{ExportBounds, FileType};
use crate::messages::prelude::*;
use dyn_any::DynAny;
use glam::DAffine2;
use graph_craft::document::value::{NetworkOutput, TaggedValue};
use graph_craft::document::{
	AbsoluteInputConnector, AbsoluteOutputConnector, CompilationMetadata, CompiledNodeMetadata, DocumentNode, DocumentNodeImplementation, NodeId, NodeInput, NodeNetwork, generate_uuid,
};
use graph_craft::proto::GraphErrors;
use graph_craft::wasm_application_io::{EditorCompilationMetadata, EditorEvaluationMetadata, EditorMetadata};
use graphene_std::application_io::{CompilationMetadata, TimingInformation};
use graphene_std::application_io::{EditorEvaluationMetadata, NodeGraphUpdateMessage};
use graphene_std::memo::IntrospectMode;
use graphene_std::renderer::{EvaluationMetadata, format_transform_matrix};
use graphene_std::renderer::{RenderMetadata, RenderSvgSegmentList};
use graphene_std::renderer::{RenderParams, SvgRender};
use graphene_std::text::FontCache;
use graphene_std::transform::{Footprint, RenderQuality};
use graphene_std::uuid::{CompiledProtonodeInput, ProtonodePath, SNI};
use graphene_std::vector::VectorData;
use graphene_std::vector::style::ViewMode;
use graphene_std::wasm_application_io::NetworkOutput;
use graphene_std::{CompiledProtonodeInput, OwnedContextImpl, SNI};

mod runtime_io;
use interpreted_executor::dynamic_executor::{EditorContext, ResolvedDocumentNodeMetadata};
pub use runtime_io::NodeRuntimeIO;

mod runtime;
pub use runtime::*;

#[derive(Clone, Debug, Default, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct CompilationRequest {
	pub network: NodeNetwork,
	// Data which is avaialable from scope inputs (currently WasmEditorApi, but will be split)
	pub font_cache: Arc<FontCache>,
	pub editor_metadata: EditorMetadata,
}

pub struct CompilationResponse {
	result: Result<CompilationMetadata, String>,
	node_graph_errors: GraphErrors,
}

// Metadata the editor sends when evaluating the network
#[derive(Debug, Default, DynAny)]
pub struct EvaluationRequest {
	pub evaluation_id: u64,
	pub inputs_to_monitor: Vec<(CompiledProtonodeInput, IntrospectMode)>,
	pub context: EditorContext,
	// pub custom_node_to_evaluate: Option<SNI>,
}

// #[cfg_attr(feature = "decouple-execution", derive(serde::Serialize, serde::Deserialize))]
pub struct EvaluationResponse {
	evaluation_id: u64,
	result: Result<TaggedValue, String>,
	introspected_inputs: Vec<(CompiledProtonodeInput, IntrospectMode, Box<dyn std::any::Any + Send + Sync>)>,
	// TODO: Handle transforming node graph output in the node graph itself
	transform: DAffine2,
}

// #[cfg_attr(feature = "decouple-execution", derive(serde::Serialize, serde::Deserialize))]
pub enum NodeGraphUpdate {
	CompilationResponse(CompilationResponse),
	EvaluationResponse(EvaluationResponse),
}

#[derive(Debug, Default)]
pub struct NodeGraphExecutor {
	runtime_io: NodeRuntimeIO,
	futures: HashMap<u64, EvaluationContext>,
}

#[derive(Debug, Clone)]
struct EvaluationContext {
	export_config: Option<ExportConfig>,
}

impl Default for NodeGraphExecutor {
	fn default() -> Self {
		Self {
			futures: Default::default(),
			runtime_io: NodeRuntimeIO::new(),
		}
	}
}

impl NodeGraphExecutor {
	/// A local runtime is useful on threads since having global state causes flakes
	#[cfg(test)]
	pub(crate) fn new_with_local_runtime() -> (NodeRuntime, Self) {
		let (request_sender, request_receiver) = std::sync::mpsc::channel();
		let (response_sender, response_receiver) = std::sync::mpsc::channel();
		let node_runtime = NodeRuntime::new(request_receiver, response_sender);

		let node_executor = Self {
			futures: HashMap::new(),
			runtime_io: NodeRuntimeIO::with_channels(request_sender, response_receiver),
		};
		(node_runtime, node_executor)
	}

	/// Updates the network to monitor all inputs. Useful for the testing.
	#[cfg(test)]
	pub(crate) fn update_node_graph_instrumented(&mut self, document: &mut DocumentMessageHandler) -> Result<Instrumented, String> {
		let mut network = document.network_interface.document_network().clone();
		let instrumented = Instrumented::new(&mut network);

		self.runtime_io
			.send(GraphRuntimeRequest::CompilationRequest(CompilationRequest { network, ..Default::default() }))
			.map_err(|e| e.to_string())?;
		Ok(instrumented)
	}

	/// Compile the network
	pub fn submit_node_graph_compilation(&mut self, compilation_request: CompilationRequest) {
		self.runtime_io.send(GraphRuntimeRequest::CompilationRequest(compilation_request)).map_err(|e| e.to_string());
	}

	/// Adds an evaluate request for whatever current network is cached.
	pub fn submit_node_graph_evaluation(
		&mut self,
		context: EditorContext,
		inputs_to_monitor: Vec<(CompiledProtonodeInput, IntrospectMode)>,
		custom_node_to_evaluate: Option<SNI>,
		export_config: Option<ExportConfig>,
	) {
		let evaluation_id = generate_uuid();
		self.runtime_io.send(GraphRuntimeRequest::EvaluationRequest(editor_evaluation_request)).map_err(|e| e.to_string());
		let evaluation_context = EvaluationContext { export_config };
		self.futures.insert(evaluation_id, evaluation_context);
	}

	// Continuously poll the executor (called by request animation frame)
	pub fn poll_node_graph_evaluation(&mut self, document: &mut DocumentMessageHandler, responses: &mut VecDeque<Message>) -> Result<(), String> {
		// Moved into portfolio message handler, since this is where the introspected inputs are saved
		for response in self.runtime_io.receive() {
			match response {
				NodeGraphUpdate::EvaluationResponse(EvaluationResponse {
					evaluation_id,
					result,
					transform,
					introspected_inputs,
				}) => {
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
					let render_output = match node_graph_output {
						TaggedValue::RenderOutput(render_output) => render_output,
						value => {
							return Err("Incorrect render type for exporting (expected NetworkOutput)".to_string());
						}
					};

					let evaluation_context = self.futures.remove(&evaluation_id).ok_or_else(|| "Invalid generation ID".to_string())?;
					if let Some(export_config) = evaluation_context.export_config {
						// Export
						let TaggedValue::RenderOutput(RenderOutput {
							data: graphene_std::wasm_application_io::RenderOutputType::Svg(svg),
							..
						}) = node_graph_output
						else {
							return Err("Incorrect render type for exporting (expected RenderOutput::Svg)".to_string());
						};

						match export_config.file_type {
							FileType::Svg => {
								responses.add(FrontendMessage::TriggerDownloadTextFile {
									document: svg,
									name: export_config.file_name,
								});
							}
							_ => {
								responses.add(FrontendMessage::TriggerDownloadImage {
									svg,
									name: export_config.file_name,
									mime: export_config.file_type.to_mime().to_string(),
									size: export_config.size.into(),
								});
							}
						}
					} else {
						// Update artwork
						self.process_node_graph_output(render_output, introspected_inputs, transform, responses);
					}
				}
				NodeGraphUpdate::CompilationResponse(compilation_response) => {
					let CompilationResponse { node_graph_errors, result } = compilation_response;
					let compilation_metadata = match result {
						Err(e) => {
							// Clear the click targets while the graph is in an un-renderable state
							document.network_interface.update_click_targets(HashMap::new());
							document.network_interface.update_vector_modify(HashMap::new());

							document.node_graph_handler.node_graph_errors = node_graph_errors;
							responses.add(NodeGraphMessage::SendGraph);

							log::trace!("{e}");
							return Err(format!("Node graph evaluation failed:\n{e}"));
						}
						Ok(result) => result,
					};
					responses.add(PortfolioMessage::ProcessCompilationResponse { compilation_metadata });
					responses.add(NodeGraphMessage::SendGraph);
				}
			}
		}
		Ok(())
	}

	fn process_node_graph_output(
		&mut self,
		node_graph_output: TaggedValue,
		introspected_inputs: Vec<(CompiledProtonodeInput, IntrospectMode, Box<dyn std::any::Any + Send + Sync>)>,
		transform: DAffine2,
		responses: &mut VecDeque<Message>,
	) -> Result<(), String> {
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
			// TaggedValue::Bool(render_object) => Self::debug_render(render_object, transform, responses),
			// TaggedValue::String(render_object) => Self::debug_render(render_object, transform, responses),
			// TaggedValue::F64(render_object) => Self::debug_render(render_object, transform, responses),
			// TaggedValue::DVec2(render_object) => Self::debug_render(render_object, transform, responses),
			// TaggedValue::OptionalColor(render_object) => Self::debug_render(render_object, transform, responses),
			// TaggedValue::VectorData(render_object) => Self::debug_render(render_object, transform, responses),
			// TaggedValue::GraphicGroup(render_object) => Self::debug_render(render_object, transform, responses),
			// TaggedValue::RasterData(render_object) => Self::debug_render(render_object, transform, responses),
			// TaggedValue::Palette(render_object) => Self::debug_render(render_object, transform, responses),
			_ => {
				return Err(format!("Invalid node graph output type: {node_graph_output:#?}"));
			}
		};
		responses.add(Message::ProcessQueue((render_output_metadata, introspected_inputs)));
		Ok(())
	}
}

// pub enum AnimationState {
// 	#[default]
// 	Stopped,
// 	Playing {
// 		start: f64,
// 	},
// 	Paused {
// 		start: f64,
// 		pause_time: f64,
// 	},
// }

// Re-export for usage by tests in other modules
// #[cfg(test)]
// pub use test::Instrumented;

// #[cfg(test)]
// mod test {
// 	use std::sync::Arc;

// 	use super::*;
// 	use crate::messages::portfolio::document::utility_types::network_interface::NodeNetworkInterface;
// 	use crate::test_utils::test_prelude::{self, NodeGraphLayer};
// 	use graph_craft::ProtoNodeIdentifier;
// 	use graph_craft::document::NodeNetwork;
// 	use graphene_std::Context;
// 	use graphene_std::NodeInputDecleration;
// 	use graphene_std::memo::IORecord;
// 	use test_prelude::LayerNodeIdentifier;

// 	/// Stores all of the monitor nodes that have been attached to a graph
// 	#[derive(Default)]
// 	pub struct Instrumented {
// 		protonodes_by_name: HashMap<ProtoNodeIdentifier, Vec<Vec<Vec<NodeId>>>>,
// 		protonodes_by_path: HashMap<Vec<NodeId>, Vec<Vec<NodeId>>>,
// 	}

// 	impl Instrumented {
// 		/// Adds montior nodes to the network
// 		fn add(&mut self, network: &mut NodeNetwork, path: &mut Vec<NodeId>) {
// 			// Required to do seperately to satiate the borrow checker.
// 			let mut monitor_nodes = Vec::new();
// 			for (id, node) in network.nodes.iter_mut() {
// 				// Recursively instrument
// 				if let DocumentNodeImplementation::Network(nested) = &mut node.implementation {
// 					path.push(*id);
// 					self.add(nested, path);
// 					path.pop();
// 				}
// 				let mut monitor_node_ids = Vec::with_capacity(node.inputs.len());
// 				for input in &mut node.inputs {
// 					let node_id = NodeId::new();
// 					let old_input = std::mem::replace(input, NodeInput::node(node_id, 0));
// 					monitor_nodes.push((old_input, node_id));
// 					path.push(node_id);
// 					monitor_node_ids.push(path.clone());
// 					path.pop();
// 				}
// 				if let DocumentNodeImplementation::ProtoNode(identifier) = &mut node.implementation {
// 					path.push(*id);
// 					self.protonodes_by_name.entry(identifier.clone()).or_default().push(monitor_node_ids.clone());
// 					self.protonodes_by_path.insert(path.clone(), monitor_node_ids);
// 					path.pop();
// 				}
// 			}
// 			for (input, monitor_id) in monitor_nodes {
// 				let monitor_node = DocumentNode {
// 					inputs: vec![input],
// 					implementation: DocumentNodeImplementation::ProtoNode(graphene_std::memo::monitor::IDENTIFIER),
// 					manual_composition: Some(graph_craft::generic!(T)),
// 					skip_deduplication: true,
// 					..Default::default()
// 				};
// 				network.nodes.insert(monitor_id, monitor_node);
// 			}
// 		}

// 		/// Instrument a graph and return a new [Instrumented] state.
// 		pub fn new(network: &mut NodeNetwork) -> Self {
// 			let mut instrumented = Self::default();
// 			instrumented.add(network, &mut Vec::new());
// 			instrumented
// 		}

// 		fn downcast<Input: NodeInputDecleration>(dynamic: Arc<dyn std::any::Any + Send + Sync>) -> Option<Input::Result>
// 		where
// 			Input::Result: Send + Sync + Clone + 'static,
// 		{
// 			// This is quite inflexible since it only allows the footprint as inputs.
// 			if let Some(x) = dynamic.downcast_ref::<IORecord<(), Input::Result>>() {
// 				Some(x.output.clone())
// 			} else if let Some(x) = dynamic.downcast_ref::<IORecord<Footprint, Input::Result>>() {
// 				Some(x.output.clone())
// 			} else if let Some(x) = dynamic.downcast_ref::<IORecord<Context, Input::Result>>() {
// 				Some(x.output.clone())
// 			} else {
// 				panic!("cannot downcast type for introspection");
// 			}
// 		}

// 		/// Grab all of the values of the input every time it occurs in the graph.
// 		pub fn grab_all_input<'a, Input: NodeInputDecleration + 'a>(&'a self, runtime: &'a NodeRuntime) -> impl Iterator<Item = Input::Result> + 'a
// 		where
// 			Input::Result: Send + Sync + Clone + 'static,
// 		{
// 			self.protonodes_by_name
// 				.get(&Input::identifier())
// 				.map_or([].as_slice(), |x| x.as_slice())
// 				.iter()
// 				.filter_map(|inputs| inputs.get(Input::INDEX))
// 				.filter_map(|input_monitor_node| runtime.executor.introspect(input_monitor_node).ok())
// 				.filter_map(Instrumented::downcast::<Input>)
// 		}

// 		pub fn grab_protonode_input<Input: NodeInputDecleration>(&self, path: &Vec<NodeId>, runtime: &NodeRuntime) -> Option<Input::Result>
// 		where
// 			Input::Result: Send + Sync + Clone + 'static,
// 		{
// 			let input_monitor_node = self.protonodes_by_path.get(path)?.get(Input::INDEX)?;

// 			let dynamic = runtime.executor.introspect(input_monitor_node).ok()?;

// 			Self::downcast::<Input>(dynamic)
// 		}

// 		pub fn grab_input_from_layer<Input: NodeInputDecleration>(&self, layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface, runtime: &NodeRuntime) -> Option<Input::Result>
// 		where
// 			Input::Result: Send + Sync + Clone + 'static,
// 		{
// 			let node_graph_layer = NodeGraphLayer::new(layer, network_interface);
// 			let node = node_graph_layer.upstream_node_id_from_protonode(Input::identifier())?;
// 			self.grab_protonode_input::<Input>(&vec![node], runtime)
// 		}
// 	}
// }
