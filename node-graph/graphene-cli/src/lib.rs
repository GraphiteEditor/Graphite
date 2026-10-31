//! Opens, compiles, and renders Graphite documents without the editor. A `.gdd` opens through the format steps and node migrations
//! this crate links, so every binary built on it reads documents the way the current build writes them.

pub mod dump;
pub mod export;

use document_container::AnyContainer;
use document_container::backends::memory::MemoryBackend;
use document_format::{GddV1, GddV1Layout};
use graph_craft::application_io::{PlatformApplicationIo, PlatformEditorApi};
use graph_craft::document::{NetworkLocation, NodeId, NodeNetwork, OwnedNetworkLocation};
use graph_craft::graphene_compiler::Compiler;
use graph_craft::proto::ProtoNetwork;
use graph_craft::util::load_network;
use graphene_std::application_io::ApplicationIo;
use graphene_std::runtime::{AsyncRuntime, Spawn, Task};
use interpreted_executor::dynamic_executor::DynamicExecutor;
use interpreted_executor::util::wrap_network_in_scope;
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

// Referenced so the linker keeps their self-registering migrations
use migration_formats as _;
use migration_nodes as _;

/// A document as the CLI loads it: the runtime network, plus the `.gdd` it came from, if any.
pub struct OpenedDocument {
	pub gdd: Option<GddV1>,
	pub network: NodeNetwork,
	/// What `dump` shows for each node and network, when asked for.
	pub names: Option<HashMap<Vec<NodeId>, String>>,
}

/// Open a `.gdd` from its archive bytes into memory, running the migrations this crate links.
pub async fn open_gdd(archive: &[u8]) -> Result<GddV1, Box<dyn Error>> {
	let container = AnyContainer::Memory(MemoryBackend::new());
	let gdd = GddV1::open_from_archive(archive, container, GddV1Layout)
		.await
		.map_err(|error| format!("Failed to open document: {error}"))?;
	Ok(gdd)
}

/// Open `path` by extension: `.gdd` as an archive, anything else as a legacy `.graphite` document. `with_names` also collects its display names.
pub async fn open_document(path: &Path, with_names: bool) -> Result<OpenedDocument, Box<dyn Error>> {
	let is_gdd = path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("gdd"));

	if !is_gdd {
		let document_string = std::fs::read_to_string(path).map_err(|error| format!("Failed to read document {}: {error}", path.display()))?;
		return Ok(OpenedDocument {
			gdd: None,
			network: load_network(&document_string),
			names: with_names.then(|| dump::legacy_names(&document_string)),
		});
	}

	let archive = std::fs::read(path).map_err(|error| format!("Failed to read document {}: {error}", path.display()))?;
	let gdd = open_gdd(&archive).await?;
	let declarations = gdd.declarations(&gdd).await;
	let (network, nodes, networks) = gdd.registry().to_runtime_with_full_metadata(&declarations)?;
	let names = with_names.then(|| {
		let references: HashMap<_, _> = networks.into_iter().filter_map(|network| Some((network.network, network.reference?))).collect();
		let display_names: HashMap<_, _> = nodes.into_iter().filter_map(|node| Some(((node.network, node.local_id), node.display_name?))).collect();
		instance_names(&network, &references, &display_names)
	});
	Ok(OpenedDocument { gdd: Some(gdd), network, names })
}

/// The name of each node, keyed by the path the dump visits it at.
/// This is its display name, or else the reference of the network it implements.
fn instance_names(network: &NodeNetwork, references: &HashMap<OwnedNetworkLocation, String>, display_names: &HashMap<(OwnedNetworkLocation, NodeId), String>) -> HashMap<Vec<NodeId>, String> {
	network
		.recursive_nodes()
		.filter_map(|(id, _, path)| {
			let containing = network.locate(&path)?.to_owned_location();
			let node_path = [path.as_slice(), &[*id]].concat();
			let implemented = network.locate(&node_path).map(NetworkLocation::to_owned_location);

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

/// Compile `document` against a fresh GPU context, resolving its resources from its `.gdd`.
pub async fn compile_document(document: OpenedDocument) -> Result<CompiledDocument, Box<dyn Error>> {
	log::info!("Creating GPU context");
	let mut application_io = PlatformApplicationIo::new().await;
	if let Some(gdd) = &document.gdd {
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
	let spawner = Arc::new(TokioSpawner::default());
	let runtime = AsyncRuntime::new(spawner.clone(), Arc::new(std::time::Instant::now()));

	let mut network = wrap_network_in_scope(document.network, editor_api, runtime.handle());

	// A `.gdd` resolves resource hashes from its registry, while a legacy `.graphite` has no resource store
	let preprocessor = preprocessor::Preprocessor::new();
	let preprocessed = match &document.gdd {
		Some(gdd) => preprocessor.preprocess(&mut network, &|resource_id| gdd.registry().resources.get(&resource_id).and_then(|resource| resource.hash)),
		None => preprocessor.preprocess(&mut network, &|_| None),
	};
	preprocessed.map_err(|error| format!("Failed to expand network: {error:?}"))?;

	let compiled_network = Compiler {}.compile_single(network)?;
	for warning in &compiled_network.warnings {
		eprintln!("{warning:?}");
	}

	Ok(CompiledDocument {
		proto_network: compiled_network.network,
		application_io,
		runtime,
		spawner,
	})
}

impl CompiledDocument {
	pub fn into_renderer(self) -> Result<Renderer, Box<dyn Error>> {
		let executor = DynamicExecutor::new(self.proto_network).map_err(|errors| {
			let messages: Vec<String> = errors.iter().map(|error| format!("{error:?}")).collect();
			messages.join("\n")
		})?;
		self.runtime.retain_sources(executor.sources());

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
