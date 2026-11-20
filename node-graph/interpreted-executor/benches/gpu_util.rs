//! Shared by the benchmarks that render documents the way the editor does, through Vello on a software Vulkan device (lavapipe),
//! so CI runners without a GPU can measure them and the counts include the "GPU" work.

use core_types::transform::{Footprint, RenderQuality};
use glam::{DAffine2, UVec2};
use graph_craft::application_io::{PlatformApplicationIo, PlatformEditorApi};
use graph_craft::document::NodeNetwork;
use graph_craft::graphene_compiler::Compiler;
use graphene_std::application_io::{ExportFormat, RenderConfig};
use graphene_std::runtime::{AsyncRuntime, Spawn, Task};
use gungraun::{Callgrind, EntryPoint, LibraryBenchmarkConfig};
use interpreted_executor::dynamic_executor::DynamicExecutor;
use interpreted_executor::util::wrap_network_in_scope;
use std::hint::black_box;
use std::sync::Arc;
use wgpu_executor::{WgpuContextBuilder, WgpuExecutor};

/// Small enough to keep Valgrind runs short, since the full-frame passes scale with it.
pub const VIEWPORT: UVec2 = UVec2::new(960, 540);

/// Measures only what runs between [`callgrind::start`] and [`callgrind::stop`], on every thread,
/// since the device works on its own queue thread, which the default entry point (collecting on the benchmark's thread alone) would leave out.
pub fn config() -> LibraryBenchmarkConfig {
	let mut config = LibraryBenchmarkConfig::default();
	config
		.pass_through_env("VK_DRIVER_FILES")
		// Rasterizing on the queue thread alone keeps the counts independent of the runner's core count
		.env("LP_NUM_THREADS", "0")
		// Shaders compiled by an earlier run, such as the baseline's, would otherwise load from disk and lower the counts
		.env("MESA_SHADER_CACHE_DISABLE", "true")
		.tool(Callgrind::with_args(["--instr-atstart=no"]).entry_point(EntryPoint::None));
	config
}

/// A document compiled into an executor that renders it through the editor's render chain, as the editor's runtime does.
pub struct GpuDocument {
	gpu: WgpuExecutor,
	editor_api: Arc<PlatformEditorApi>,
	runtime: AsyncRuntime,
	preprocessor: preprocessor::Preprocessor,
	executor: DynamicExecutor,
}

impl GpuDocument {
	pub fn new() -> Self {
		let gpu = lavapipe();
		Self {
			editor_api: Arc::new(PlatformEditorApi {
				application_io: Some(Arc::new(PlatformApplicationIo::new_with_executor(gpu.clone()))),
			}),
			gpu,
			runtime: AsyncRuntime::new(Arc::new(BlockingSpawner)),
			preprocessor: preprocessor::Preprocessor::new(),
			executor: DynamicExecutor::default(),
		}
	}

	/// Wraps and compiles the document graph into the executor, as the runtime does after an edit.
	pub fn compile(&mut self, document: NodeNetwork) {
		let mut network = wrap_network_in_scope(document, self.editor_api.clone(), self.runtime.handle());
		self.preprocessor.preprocess(&mut network, &|_| None).expect("Failed to preprocess the document");
		let proto_network = Compiler {}.compile_single(network).expect("Failed to compile the document").network;
		self.executor.update(proto_network).expect("Failed to update the executor");
	}

	/// Renders the viewport with the document placed by `transform`, waiting for the device to finish.
	pub fn render(&self, transform: DAffine2) {
		let render_config = RenderConfig {
			viewport: Footprint {
				transform,
				resolution: VIEWPORT,
				quality: RenderQuality::Full,
			},
			scale: 1.,
			export_format: ExportFormat::Raster,
			..Default::default()
		};
		let output = self.executor.tree().eval_tagged_value(self.executor.output(), &render_config.into_context());
		assert!(matches!(output, Ok(graphene_std::Poll::Final(_))), "Failed to render the document: {output:?}");
		// Waiting for the device puts its rasterization inside the measurement, rather than only the encoding of its commands
		let _ = self.gpu.context().device.poll(wgpu::PollType::wait_indefinitely());
		let _ = black_box(output);
	}
}

/// Opens lavapipe, refusing a hardware device, whose work Callgrind can't see.
fn lavapipe() -> WgpuExecutor {
	let context = futures::executor::block_on(WgpuContextBuilder::new().with_backends(wgpu::Backends::VULKAN).build())
		.expect("No Vulkan device; install lavapipe (mesa-vulkan-drivers) and point VK_DRIVER_FILES at its ICD");
	let info = context.adapter.get_info();
	assert_eq!(
		info.device_type,
		wgpu::DeviceType::Cpu,
		"Picked {} instead of a software device, which would make the counts meaningless; point VK_DRIVER_FILES at lavapipe's ICD",
		info.name
	);
	WgpuExecutor::with_context(context).expect("Failed to create the GPU executor")
}

/// Runs spawned work to completion on the spot, so everything the graph starts happens in the measured section.
struct BlockingSpawner;

impl Spawn for BlockingSpawner {
	fn spawn(&self, task: Task) {
		futures::executor::block_on(task);
	}
}

/// Callgrind's client requests for switching instrumentation, which unlike collection applies to every thread.
/// Outside Valgrind these are a few no-op rotations.
pub mod callgrind {
	/// `VG_USERREQ_TOOL_BASE('C', 'T')` in `callgrind.h`, followed by `DUMP_STATS`, `ZERO_STATS`, `TOGGLE_COLLECT` and `DUMP_STATS_AT`
	const START_INSTRUMENTATION: u64 = ((b'C' as u64) << 24 | (b'T' as u64) << 16) + 4;
	const STOP_INSTRUMENTATION: u64 = START_INSTRUMENTATION + 1;

	pub fn start() {
		request(START_INSTRUMENTATION);
	}

	pub fn stop() {
		request(STOP_INSTRUMENTATION);
	}

	/// The x86-64 form of `VALGRIND_DO_CLIENT_REQUEST_EXPR` in `valgrind.h`, which Valgrind recognizes by the preamble's rotations of `rdi` (adding up to a full turn).
	#[cfg(target_arch = "x86_64")]
	fn request(code: u64) {
		let args = [code, 0, 0, 0, 0, 0];
		// SAFETY: The rotations leave `rdi` as it was and `xchg rbx, rbx` changes nothing, so natively this only reads `args`; under Valgrind, the request writes `rdx`
		unsafe {
			std::arch::asm!(
				"rol rdi, 3",
				"rol rdi, 13",
				"rol rdi, 61",
				"rol rdi, 51",
				"xchg rbx, rbx",
				in("rax") args.as_ptr(),
				inout("rdx") 0_u64 => _,
				out("rdi") _,
				options(nostack),
			);
		}
	}

	#[cfg(not(target_arch = "x86_64"))]
	fn request(_code: u64) {
		panic!("The GPU benchmarks only know how to reach Valgrind on x86-64");
	}
}
