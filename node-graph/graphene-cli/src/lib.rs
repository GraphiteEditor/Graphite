//! Opens, compiles, and renders Graphite documents without the editor. Documents open through `document-loader`, so every binary
//! built on it reads documents the way the current build writes them.

pub mod dump;
pub mod export;

use document_format::GddV1;
use document_graph_storage::NodeMetadataEntry;
use document_loader::{DocumentOrigin, LoadedDocument};
use graph_craft::application_io::{PlatformApplicationIo, PlatformEditorApi};
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NetworkLocation, NodeId, NodeNetwork};
use graph_craft::graphene_compiler::Compiler;
use graph_craft::proto::ProtoNetwork;
use graphene_std::application_io::ApplicationIo;
use graphene_std::runtime::{AsyncRuntime, RuntimeHandle, Spawn, Task};
use interpreted_executor::dynamic_executor::DynamicExecutor;
use interpreted_executor::layer_paths::write_layer_paths;
use interpreted_executor::util::wrap_network_in_scope;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// A document as the CLI loads it: the runtime network, plus the `.gdd` it came from, if any.
pub struct OpenedDocument {
	pub gdd: Option<GddV1>,
	pub network: NodeNetwork,
	/// The path of every layer, which compiling writes onto what the layer renders so the artwork can be measured.
	pub layers: Vec<Vec<NodeId>>,
	/// What `dump` shows for each node and network, when asked for.
	pub names: Option<HashMap<Vec<NodeId>, String>>,
}

/// Open `path` with [`document_loader::open_document`]. `with_names` also collects its display names.
pub async fn open_document(path: &Path, with_names: bool) -> Result<OpenedDocument, Box<dyn Error>> {
	let LoadedDocument { network, origin } = document_loader::open_document(path).await?;

	let (gdd, layers, names) = match origin {
		DocumentOrigin::Legacy(document_string) => {
			let layers = legacy_layer_paths(&network, &document_string);
			(None, layers, with_names.then(|| dump::legacy_names(&document_string)))
		}
		DocumentOrigin::Gdd { gdd, nodes, networks } => {
			let layers = layer_paths(&network, &nodes);
			let names = with_names.then(|| {
				let references: HashMap<_, _> = networks.into_iter().filter_map(|network| Some((network.network, network.reference?))).collect();
				let display_names: HashMap<_, _> = nodes.into_iter().filter_map(|node| Some(((node.network, node.local_id), node.display_name?))).collect();
				instance_names(&network, &references, &display_names)
			});
			(Some(*gdd), layers, names)
		}
	};
	Ok(OpenedDocument { gdd, network, layers, names })
}

/// The path of every node its metadata marks as a layer, once for each instance of the networks holding it.
pub fn layer_paths(network: &NodeNetwork, node_metadata: &[NodeMetadataEntry]) -> Vec<Vec<NodeId>> {
	let layers: HashSet<(NetworkLocation, NodeId)> = node_metadata.iter().filter(|node| node.is_layer).map(|node| (node.network, node.local_id)).collect();

	instance_layer_paths(
		network,
		None,
		|_, _, node| match node.implementation {
			DocumentNodeImplementation::Definition(id) => Some(Some(id)),
			_ => None,
		},
		|location, node_id| layers.contains(&(*location, node_id)),
	)
}

/// The path of every layer in a legacy document, read from its JSON text.
/// That describes the networks nodes hold inside their own metadata, and the ones they name in a table beside the document network's.
fn legacy_layer_paths(network: &NodeNetwork, document: &str) -> Vec<Vec<NodeId>> {
	let Ok(document) = serde_json::from_str::<serde_json::Value>(document) else { return Vec::new() };
	let document_network = &document["network_interface"]["network_metadata"];
	let definitions = &document_network["persistent_metadata"]["definitions"];

	instance_layer_paths(
		network,
		legacy_node_metadata(document_network),
		|node_metadata, node_id, node| match node.implementation {
			DocumentNodeImplementation::Network(_) => Some(legacy_node_metadata(&node_metadata.get(&node_id).copied()?["network_metadata"])),
			DocumentNodeImplementation::Definition(id) => Some(legacy_node_metadata(&definitions[id.0.to_string()])),
			_ => None,
		},
		|node_metadata, node_id| node_metadata.get(&node_id).is_some_and(|metadata| metadata["node_type_metadata"]["Layer"].is_object()),
	)
}

/// The metadata of each node in a legacy document's network, by ID.
fn legacy_node_metadata(network_metadata: &serde_json::Value) -> HashMap<NodeId, &serde_json::Value> {
	let entries = network_metadata["persistent_metadata"]["node_metadata"].as_array().into_iter().flatten();
	entries.filter_map(|entry| Some((NodeId(entry[0].as_u64()?), &entry[1]["persistent_metadata"]))).collect()
}

/// The path of every layer, once for each instance of the networks holding it, with a network's layers before those of the networks inside it as the editor orders them.
/// The walk carries the metadata of each network it enters, which `nested` finds for the network a node holds or names.
fn instance_layer_paths<Metadata>(
	network: &NodeNetwork,
	document_network_metadata: Metadata,
	nested: impl Fn(&Metadata, NodeId, &DocumentNode) -> Option<Metadata>,
	is_layer: impl Fn(&Metadata, NodeId) -> bool,
) -> Vec<Vec<NodeId>> {
	let mut layer_paths = Vec::new();

	// Each network is walked with the definitions enclosing it, so one that holds itself isn't entered again
	let mut stack = vec![(Vec::new(), document_network_metadata, Vec::new())];
	while let Some((network_path, metadata, enclosing_definitions)) = stack.pop() {
		let Some(walked) = network.nested_network(&network_path) else { continue };

		for (node_id, node) in &walked.nodes {
			let node_path = [network_path.as_slice(), &[*node_id]].concat();
			if is_layer(&metadata, *node_id) {
				layer_paths.push(node_path.clone());
			}

			let definition = match node.implementation {
				DocumentNodeImplementation::Definition(id) if enclosing_definitions.contains(&id) => continue,
				DocumentNodeImplementation::Definition(id) => Some(id),
				_ => None,
			};
			if let Some(nested_metadata) = nested(&metadata, *node_id, node) {
				let enclosing_definitions = enclosing_definitions.iter().copied().chain(definition).collect();
				stack.push((node_path, nested_metadata, enclosing_definitions));
			}
		}
	}

	layer_paths
}

/// The name of each node, keyed by the path the dump visits it at.
/// This is its display name, or else the reference of the network it implements.
fn instance_names(network: &NodeNetwork, references: &HashMap<NetworkLocation, String>, display_names: &HashMap<(NetworkLocation, NodeId), String>) -> HashMap<Vec<NodeId>, String> {
	network
		.recursive_nodes()
		.filter_map(|(id, _, path)| {
			let containing = network.locate(&path)?;
			let node_path = [path.as_slice(), &[*id]].concat();
			let implemented = network.locate(&node_path);

			let display_name = display_names.get(&(containing, *id)).filter(|name| !name.is_empty());
			let reference = implemented.and_then(|location| references.get(&location)).filter(|name| !name.is_empty());
			Some((node_path, display_name.or(reference)?.clone()))
		})
		.collect()
}

/// A document compiled against a GPU context, ready to turn into a [`Renderer`].
pub struct CompiledDocument {
	pub proto_network: ProtoNetwork,
	application_io: Arc<PlatformApplicationIo>,
	runtime: AsyncRuntime,
	spawner: Arc<TokioSpawner>,
}

pub async fn editor_api(gdd: Option<&GddV1>) -> (Arc<PlatformApplicationIo>, Arc<PlatformEditorApi>) {
	log::info!("Creating GPU context");
	let mut application_io = PlatformApplicationIo::new().await;
	if let Some(gdd) = gdd {
		application_io.inject_resource_proxy(Box::new(gdd.resource_proxy()));
	}
	let application_io = Arc::new(application_io);
	if let Some(executor) = application_io.gpu_executor() {
		let info = executor.context().adapter.get_info();
		log::info!("Rendering on {} ({:?}, {:?}, driver {})", info.name, info.backend, info.device_type, info.driver);
	}

	let editor_api = Arc::new(PlatformEditorApi {
		application_io: Some(application_io.clone()),
	});
	(application_io, editor_api)
}

/// Compile `network`, writing the path of each of its `layers` onto what that layer renders.
pub fn compile_graph(mut network: NodeNetwork, layers: &[Vec<NodeId>], editor_api: Arc<PlatformEditorApi>, gdd: Option<&GddV1>, runtime: RuntimeHandle) -> Result<ProtoNetwork, Box<dyn Error>> {
	write_layer_paths(&mut network, layers);
	let mut network = wrap_network_in_scope(network, editor_api, runtime);

	let preprocessor = preprocessor::Preprocessor::new();
	let preprocessed = match gdd {
		Some(gdd) => preprocessor.preprocess(&mut network, &|resource_id| gdd.registry().resources.get(&resource_id).and_then(|resource| resource.hash)),
		None => preprocessor.preprocess(&mut network, &|_| None),
	};
	preprocessed.map_err(|error| format!("Failed to expand network: {error:?}"))?;

	let compiled_network = Compiler {}.compile_single(network)?;
	for warning in &compiled_network.warnings {
		eprintln!("{warning:?}");
	}
	Ok(compiled_network.network)
}

pub fn create_executor(proto_network: ProtoNetwork) -> Result<DynamicExecutor, Box<dyn Error>> {
	DynamicExecutor::new(proto_network).map_err(|errors| {
		let messages: Vec<String> = errors.iter().map(|error| format!("{error:?}")).collect();
		messages.join("\n").into()
	})
}

pub async fn compile_document(document: OpenedDocument) -> Result<CompiledDocument, Box<dyn Error>> {
	let (application_io, editor_api) = editor_api(document.gdd.as_ref()).await;
	let spawner = Arc::new(TokioSpawner::default());
	let runtime = AsyncRuntime::new(spawner.clone());
	let proto_network = compile_graph(document.network, &document.layers, editor_api, document.gdd.as_ref(), runtime.handle())?;

	Ok(CompiledDocument {
		proto_network,
		application_io,
		runtime,
		spawner,
	})
}

impl CompiledDocument {
	pub fn into_renderer(self) -> Result<Renderer, Box<dyn Error>> {
		let executor = create_executor(self.proto_network)?;
		self.runtime.update_sources(executor.sources());

		let device = self.application_io.gpu_executor().ok_or("No GPU executor is available")?.context().device.clone();
		let polling = Arc::new(AtomicBool::new(true));
		let still_polling = polling.clone();
		std::thread::spawn(move || {
			while still_polling.load(Ordering::Relaxed) {
				std::thread::sleep(std::time::Duration::from_nanos(10));
				if let Err(error) = device.poll(wgpu::PollType::Poll) {
					log::error!("Polling the GPU device failed: {error}");
				}
			}
		});

		Ok(Renderer {
			executor,
			application_io: self.application_io,
			runtime: self.runtime,
			spawner: self.spawner,
			polling,
		})
	}
}

/// Executes a compiled document, polling its GPU device until dropped.
pub struct Renderer {
	pub executor: DynamicExecutor,
	pub runtime: AsyncRuntime,
	pub spawner: Arc<TokioSpawner>,
	application_io: Arc<PlatformApplicationIo>,
	polling: Arc<AtomicBool>,
}

impl Renderer {
	pub fn wgpu_executor(&self) -> &wgpu_executor::WgpuExecutor {
		self.application_io.gpu_executor().expect("a renderer is only built with a GPU executor")
	}

	/// Fit `settings` to the artwork, unless they give a size.
	pub async fn frame(&self, settings: export::ExportSettings) -> Result<export::ExportSettings, Box<dyn Error>> {
		settings.framed(&self.executor, &self.runtime, &self.spawner).await
	}

	/// Render one frame at `animation_time`.
	pub async fn render(&self, settings: export::ExportSettings, animation_time: std::time::Duration) -> Result<export::RenderedFrame, Box<dyn Error>> {
		export::render_frame(&self.executor, self.wgpu_executor(), settings, animation_time, &self.runtime, &self.spawner).await
	}
}

impl Drop for Renderer {
	fn drop(&mut self) {
		self.polling.store(false, Ordering::Relaxed);
	}
}

/// Runs the tasks nodes spawn on tokio, so an export can wait for them to settle.
#[derive(Default)]
pub struct TokioSpawner(Mutex<Vec<tokio::task::JoinHandle<()>>>);

impl Spawn for TokioSpawner {
	fn spawn(&self, task: Task) {
		self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(tokio::spawn(task));
	}
}

impl TokioSpawner {
	/// Wait for every spawned task. Returns whether there were any.
	pub(crate) async fn join(&self) -> bool {
		let tasks = std::mem::take(&mut *self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner));
		let pending = !tasks.is_empty();
		for task in tasks {
			let _ = task.await;
		}
		pending
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use graph_craft::document::NetworkId;

	const DEFINITION: NetworkId = NetworkId(9);

	fn node(implementation: DocumentNodeImplementation) -> DocumentNode {
		DocumentNode { implementation, ..Default::default() }
	}

	/// A network holding the nodes 3 and 4.
	fn inner_network() -> NodeNetwork {
		let nodes = [3, 4].map(|id| (NodeId(id), DocumentNode::default()));
		NodeNetwork {
			nodes: nodes.into_iter().collect(),
			..Default::default()
		}
	}

	fn node_metadata(network: NetworkLocation, local_id: u64, is_layer: bool) -> NodeMetadataEntry {
		NodeMetadataEntry {
			network,
			local_id: NodeId(local_id),
			position: None,
			is_layer,
			display_name: None,
			locked: false,
			pinned: false,
			input_metadata: Vec::new(),
			output_names: Vec::new(),
			output_descriptions: Vec::new(),
		}
	}

	/// The paths found, checking each layer comes after the one holding it.
	fn checked_and_sorted(mut layer_paths: Vec<Vec<NodeId>>) -> Vec<Vec<NodeId>> {
		for (index, layer_path) in layer_paths.iter().enumerate() {
			let holder = &layer_path[..layer_path.len() - 1];
			let holder_index = layer_paths.iter().position(|other| other == holder);
			assert!(
				holder_index.is_none_or(|holder_index| holder_index < index),
				"Layer {layer_path:?} should come after the layer holding it"
			);
		}

		layer_paths.sort();
		layer_paths
	}

	/// The nodes 1 and 2, which both name the definition holding the nodes 3 and 4.
	fn two_instances_of_a_definition() -> NodeNetwork {
		NodeNetwork {
			nodes: [1, 2].map(|id| (NodeId(id), node(DocumentNodeImplementation::Definition(DEFINITION)))).into_iter().collect(),
			definitions: [(DEFINITION, inner_network())].into_iter().collect(),
			..Default::default()
		}
	}

	/// The layer paths when 1 and 3 are layers.
	fn expected_instance_paths() -> Vec<Vec<NodeId>> {
		vec![vec![NodeId(1)], vec![NodeId(1), NodeId(3)], vec![NodeId(2), NodeId(3)]]
	}

	#[test]
	fn a_layer_inside_a_definition_is_found_through_each_instance() {
		let network = two_instances_of_a_definition();
		let metadata = [(None, 1, true), (None, 2, false), (Some(DEFINITION), 3, true), (Some(DEFINITION), 4, false)];
		let metadata: Vec<_> = metadata.into_iter().map(|(location, local_id, is_layer)| node_metadata(location, local_id, is_layer)).collect();

		assert_eq!(checked_and_sorted(layer_paths(&network, &metadata)), expected_instance_paths());
	}

	// Current legacy JSON describes a definition's nodes in a table beside the document network's
	#[test]
	fn a_legacy_layer_inside_a_definition_is_found_through_each_instance() {
		let network = two_instances_of_a_definition();
		let document = r#"{ "network_interface": { "network_metadata": { "persistent_metadata": {
			"node_metadata": [[1, { "persistent_metadata": { "node_type_metadata": { "Layer": {} } } }], [2, { "persistent_metadata": { "node_type_metadata": { "Node": {} } } }]],
			"definitions": { "9": { "persistent_metadata": {
				"node_metadata": [[3, { "persistent_metadata": { "node_type_metadata": { "Layer": {} } } }], [4, { "persistent_metadata": { "node_type_metadata": { "Node": {} } } }]]
			} } }
		} } } }"#;

		assert_eq!(checked_and_sorted(legacy_layer_paths(&network, document)), expected_instance_paths());
	}

	// Older legacy JSON describes a network a node holds inside that node's own metadata
	#[test]
	fn a_legacy_layer_inside_a_held_network_is_found() {
		let network = NodeNetwork {
			nodes: [(NodeId(1), node(DocumentNodeImplementation::Network(inner_network())))].into_iter().collect(),
			..Default::default()
		};
		let document = r#"{ "network_interface": { "network_metadata": { "persistent_metadata": {
			"node_metadata": [[1, { "persistent_metadata": {
				"node_type_metadata": { "Layer": {} },
				"network_metadata": { "persistent_metadata": {
					"node_metadata": [[3, { "persistent_metadata": { "node_type_metadata": { "Layer": {} } } }], [4, { "persistent_metadata": { "node_type_metadata": { "Node": {} } } }]]
				} }
			} }]]
		} } } }"#;

		assert_eq!(checked_and_sorted(legacy_layer_paths(&network, document)), vec![vec![NodeId(1)], vec![NodeId(1), NodeId(3)]]);
	}
}
