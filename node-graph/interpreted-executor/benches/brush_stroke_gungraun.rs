//! Counts the instructions a document update costs when one stroke is added to its brush layer, the way the Brush tool appends strokes:
//! wrapping and compiling the changed graph, updating the executor and rendering the viewport, as the editor's runtime does for each change.
//! Rendering the same document with an empty brush cache is measured alongside, showing how much of the earlier render the cache saves.
//!
//! The graph renders on a software Vulkan device (lavapipe), so CI runners without a GPU can measure it and the counts include the "GPU" work.

use core_types::transform::{Footprint, RenderQuality};
use glam::{DAffine2, DVec2, UVec2};
use graph_craft::application_io::{PlatformApplicationIo, PlatformEditorApi};
use graph_craft::concrete;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeInput, NodeNetwork};
use graph_craft::graphene_compiler::Compiler;
use graphene_std::application_io::RenderConfig;
use graphene_std::brush::basic_brush::{basic_brush, basic_brush_pipeline};
use graphene_std::brush::{BrushCache, Channel, Stroke, brush_strokes};
use graphene_std::graphic;
use graphene_std::runtime::{AsyncRuntime, Spawn, Task};
use graphene_std::uuid::NodeId;
use graphene_std::{Color, Context};
use gungraun::prelude::*;
use gungraun::{Callgrind, EntryPoint};
use interpreted_executor::dynamic_executor::DynamicExecutor;
use interpreted_executor::util::wrap_network_in_scope;
use std::hint::black_box;
use std::sync::Arc;
use wgpu_executor::{WgpuContextBuilder, WgpuExecutor};

/// Small enough to keep Valgrind runs short, since the full-frame passes scale with it.
const VIEWPORT: UVec2 = UVec2::new(960, 540);
/// Samples in the added stroke.
const STROKE_SAMPLES: usize = 128;
/// Samples in each stroke already in the document.
const DOCUMENT_STROKE_SAMPLES: usize = 48;
/// Distance between consecutive pointer samples, about what a quick drag moves between pointer events.
const SAMPLE_SPACING: f64 = 4.;
/// Strokes the Brush tool draws before its settings change.
const STROKES_PER_GROUP: usize = 8;

pub struct Session {
	gpu: WgpuExecutor,
	editor_api: Arc<PlatformEditorApi>,
	runtime: AsyncRuntime,
	preprocessor: preprocessor::Preprocessor,
	executor: DynamicExecutor,
	/// The document graph for the measured update.
	document: NodeNetwork,
}

impl Session {
	/// Opens lavapipe and renders the finished document once through a brush cache that is then discarded,
	/// so that the device's shaders are compiled and its textures allocated before anything is measured.
	fn open(after: &[Group]) -> Self {
		let gpu = lavapipe();
		let mut session = Self {
			editor_api: Arc::new(PlatformEditorApi {
				application_io: Some(Arc::new(PlatformApplicationIo::new_with_executor(gpu.clone()))),
			}),
			gpu,
			runtime: AsyncRuntime::new(Arc::new(BlockingSpawner)),
			preprocessor: preprocessor::Preprocessor::new(),
			executor: DynamicExecutor::default(),
			document: NodeNetwork::default(),
		};
		session.update(brush_layer(&BrushCache::default(), after));
		session
	}

	/// Recompiles the document graph into the executor and renders the viewport, as the runtime does after an edit.
	fn update(&mut self, document: NodeNetwork) {
		let mut network = wrap_network_in_scope(document, self.editor_api.clone(), self.runtime.handle());
		self.preprocessor.preprocess(&mut network, &|_| None).expect("Failed to preprocess the document");
		let proto_network = Compiler {}.compile_single(network).expect("Failed to compile the document").network;
		self.executor.update(proto_network).expect("Failed to update the executor");

		let render_config = RenderConfig {
			viewport: Footprint {
				transform: DAffine2::IDENTITY,
				resolution: VIEWPORT,
				quality: RenderQuality::Full,
			},
			scale: 1.,
			..Default::default()
		};
		let output = self.executor.tree().eval_tagged_value(self.executor.output(), &render_config.into_context());
		assert!(matches!(output, Ok(graphene_std::Poll::Final(_))), "Failed to render the document: {output:?}");
		// Waiting for the device puts its rasterization inside the measurement, rather than only the encoding of its commands
		let _ = self.gpu.context().device.poll(wgpu::PollType::wait_indefinitely());
		let _ = black_box(output);
	}
}

/// A document of `existing` strokes rendered once, so the added stroke meets the caches the editor would have.
fn setup_add_stroke(existing: usize) -> Session {
	let (before, after) = document(existing);
	let mut session = Session::open(&after);

	// The brush layer's cache lives in the graph, so it carries over between compilations like in the editor
	let cache = BrushCache::default();
	session.update(brush_layer(&cache, &before));
	session.document = brush_layer(&cache, &after);
	session
}

/// The same document with the stroke already added, meeting an empty brush cache, as after reopening the document.
fn setup_render_layer(existing: usize) -> Session {
	let (_, after) = document(existing);
	let mut session = Session::open(&after);
	session.document = brush_layer(&BrushCache::default(), &after);
	session
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

/// The `Brush Strokes` settings of a group, as the Brush tool starts a new group whenever its settings change.
#[derive(Clone, Copy)]
struct Style {
	color: Color,
	diameter: f64,
	hardness: f64,
	flow: f64,
}

/// Strokes drawn with the same settings, which the Brush tool keeps in one `Brush Strokes` group.
#[derive(Clone)]
struct Group {
	style: Style,
	strokes: Vec<Stroke>,
}

/// The groups of a document of `existing` strokes before and after one more stroke is drawn on top of it, the same on every run.
/// The added stroke continues the top group, as drawing on with the same settings does.
/// The groups' colors go once around the color wheel, and their other settings vary a little around the Brush tool's defaults.
fn document(existing: usize) -> (Vec<Group>, Vec<Group>) {
	let mut random = Random(existing as u64);
	let count = existing.div_ceil(STROKES_PER_GROUP).max(1);
	let mut before: Vec<_> = (0..count)
		.map(|index| Group {
			style: random.style(index as f32 / count as f32),
			strokes: Vec::new(),
		})
		.collect();
	for index in 0..existing {
		before[index / STROKES_PER_GROUP].strokes.push(random.stroke(DOCUMENT_STROKE_SAMPLES));
	}

	let mut after = before.clone();
	after.last_mut().expect("A document has at least one group").strokes.push(random.stroke(STROKE_SAMPLES));
	(before, after)
}

/// The `Basic Brush` node and the layer conversion after it, whose IDs stay put as groups are added.
const BRUSH_NODE: NodeId = NodeId(0);
const LAYER_NODE: NodeId = NodeId(1);

/// A document holding just a brush layer with its `Brush Strokes` groups,
/// stacked with the nodes the `Merge` layer uses and ending in the conversion the brush layer applies to its content.
/// Each group keeps its node IDs as strokes are added, as in the editor.
fn brush_layer(cache: &BrushCache, groups: &[Group]) -> NodeNetwork {
	let node = |implementation, inputs| DocumentNode {
		inputs,
		call_argument: concrete!(Context),
		implementation: DocumentNodeImplementation::ProtoNode(implementation),
		..Default::default()
	};
	// Each group's `Brush Strokes`, `Into Group` and `Extend` nodes, numbered after the layer's own
	let id = |index: usize, part: u64| NodeId(10 + 3 * index as u64 + part);

	let mut nodes = Vec::new();
	let mut stack = None;
	for (index, group) in groups.iter().enumerate() {
		let strokes = node(
			brush_strokes::IDENTIFIER,
			vec![
				NodeInput::value(TaggedValue::Strokes(group.strokes.clone()), false),
				NodeInput::value(TaggedValue::Color(group.style.color), false),
				NodeInput::value(TaggedValue::Number(group.style.diameter), false),
				NodeInput::value(TaggedValue::Number(group.style.hardness), false),
				NodeInput::value(TaggedValue::Number(group.style.flow), false),
			],
		);
		nodes.push((id(index, 0), strokes));
		nodes.push((id(index, 1), node(graphic::into_group::IDENTIFIER, vec![NodeInput::node(id(index, 0), 0)])));
		stack = Some(match stack {
			None => id(index, 1),
			Some(below) => {
				nodes.push((id(index, 2), node(graphic::extend::IDENTIFIER, vec![NodeInput::node(below, 0), NodeInput::node(id(index, 1), 0)])));
				id(index, 2)
			}
		});
	}

	let stack = stack.expect("A brush layer has at least one group");
	let brush = node(
		basic_brush::IDENTIFIER,
		vec![
			NodeInput::node(stack, 0),
			NodeInput::value(TaggedValue::BrushCache(cache.clone()), false),
			NodeInput::scope(basic_brush_pipeline::IDENTIFIER),
		],
	);
	nodes.push((BRUSH_NODE, brush));
	// The layer node takes its content through `Into Group` before merging it into the stack
	nodes.push((LAYER_NODE, node(graphic::into_group::IDENTIFIER, vec![NodeInput::node(BRUSH_NODE, 0)])));

	NodeNetwork {
		exports: vec![NodeInput::node(LAYER_NODE, 0)],
		nodes: nodes.into_iter().collect(),
		..Default::default()
	}
}

/// Wandering pen strokes and their settings from a fixed seed.
struct Random(u64);

impl Random {
	fn next(&mut self) -> f64 {
		// SplitMix64
		self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
		let mut z = self.0;
		z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
		z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
		((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
	}

	/// A value in `center ± spread`.
	fn around(&mut self, center: f64, spread: f64) -> f64 {
		center + (self.next() * 2. - 1.) * spread
	}

	fn style(&mut self, hue: f32) -> Style {
		Style {
			color: Color::from_hsva(hue, 0.7, 0.9, 1.),
			diameter: self.around(24., 4.),
			hardness: self.around(50., 10.),
			flow: self.around(80., 10.),
		}
	}

	fn stroke(&mut self, samples: usize) -> Stroke {
		let viewport = VIEWPORT.as_dvec2();
		let mut position = DVec2::new(self.next(), self.next()) * viewport;
		let mut heading = self.next() * std::f64::consts::TAU;
		let mut positions = Vec::with_capacity(samples);
		let mut pressures = Vec::with_capacity(samples);
		for index in 0..samples {
			positions.push(position);
			pressures.push((0.4 + 0.6 * (index as f64 / samples as f64 * std::f64::consts::PI).sin()) as f32);
			heading += (self.next() - 0.5) * 0.6;
			position = (position + DVec2::from_angle(heading) * SAMPLE_SPACING).clamp(DVec2::ZERO, viewport);
		}
		Stroke {
			position: positions,
			pressure: Channel::Samples(pressures),
			seed: (self.next() * u32::MAX as f64) as u64,
			..Default::default()
		}
	}
}

/// Callgrind's client requests for switching instrumentation, which unlike collection applies to every thread.
/// Outside Valgrind these are a few no-op rotations.
mod callgrind {
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
		panic!("This benchmark only knows how to reach Valgrind on x86-64");
	}
}

#[library_benchmark]
#[benches::existing_strokes(args = [0, 16, 256], setup = setup_add_stroke)]
pub fn add_stroke(session: Session) -> Session {
	measure(session)
}

#[library_benchmark]
#[benches::existing_strokes(args = [0, 16, 256], setup = setup_render_layer)]
pub fn render_layer(session: Session) -> Session {
	measure(session)
}

fn measure(mut session: Session) -> Session {
	let document = std::mem::take(&mut session.document);
	callgrind::start();
	session.update(document);
	callgrind::stop();

	// Return the session so its teardown happens outside the measured section
	session
}

library_benchmark_group!(name = brush_stroke_group; benchmarks = add_stroke, render_layer);

main!(
	// Only what runs between `callgrind::start` and `callgrind::stop` is measured, on every thread,
	// since the device works on its own queue thread, which the default entry point (collecting on the benchmark's thread alone) would leave out
	config = LibraryBenchmarkConfig::default()
		.pass_through_env("VK_DRIVER_FILES")
		// Rasterizing on the queue thread alone keeps the counts independent of the runner's core count
		.env("LP_NUM_THREADS", "0")
		.tool(Callgrind::with_args(["--instr-atstart=no"]).entry_point(EntryPoint::None));
	library_benchmark_groups = brush_stroke_group
);
