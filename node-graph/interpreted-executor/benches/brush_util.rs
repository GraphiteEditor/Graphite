//! Shared by the brush benchmarks, which render on a software Vulkan device (lavapipe) so that CI runners without a GPU can measure them,
//! with the counts including the "GPU" work.

use glam::{DVec2, UVec2};
use graphene_std::Color;
use graphene_std::brush::{Channel, Stroke};
use gungraun::{Callgrind, EntryPoint, LibraryBenchmarkConfig};
use wgpu_executor::{WgpuContextBuilder, WgpuExecutor};

/// Small enough to keep Valgrind runs short, since the full-frame passes scale with it.
pub const VIEWPORT: UVec2 = UVec2::new(960, 540);
/// Samples in the added stroke.
pub const STROKE_SAMPLES: usize = 128;
/// Samples in each stroke already in the document.
pub const DOCUMENT_STROKE_SAMPLES: usize = 48;
/// Distance between consecutive pointer samples, about what a quick drag moves between pointer events.
const SAMPLE_SPACING: f64 = 4.;

/// Measures only what runs between [`callgrind::start`] and [`callgrind::stop`], on every thread.
/// The device works on its own queue thread, which the default entry point (collecting on the benchmark's thread alone) would leave out.
pub fn config() -> LibraryBenchmarkConfig {
	let mut config = LibraryBenchmarkConfig::default();
	config
		.pass_through_env("VK_DRIVER_FILES")
		// Rasterizing on the queue thread alone keeps the counts independent of the runner's core count
		.env("LP_NUM_THREADS", "0")
		.tool(Callgrind::with_args(["--instr-atstart=no"]).entry_point(EntryPoint::None));
	config
}

/// Opens lavapipe, refusing a hardware device, whose work Callgrind can't see.
pub fn lavapipe() -> WgpuExecutor {
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

/// Blocks until the device finishes everything submitted so far, so its rasterization counts rather than only the encoding of its commands.
pub fn wait(executor: &WgpuExecutor) {
	let _ = executor.context().device.poll(wgpu::PollType::wait_indefinitely());
}

/// The `Brush Strokes` settings of a group, as the Brush tool starts a new group whenever its settings change.
#[derive(Clone, Copy)]
pub struct Style {
	pub color: Color,
	pub diameter: f64,
	pub hardness: f64,
	pub flow: f64,
}

/// Strokes drawn with the same settings, which the Brush tool keeps in one `Brush Strokes` group.
#[derive(Clone)]
pub struct Group {
	pub style: Style,
	pub strokes: Vec<Stroke>,
}

/// Strokes the Brush tool draws before its settings change.
const STROKES_PER_GROUP: usize = 8;

/// The groups of a document of `existing` strokes before and after one more stroke is drawn on top of it, the same on every run.
/// The added stroke continues the top group, as drawing on with the same settings does.
/// The groups' colors go once around the color wheel, and their other settings vary a little around the Brush tool's defaults.
pub fn document(existing: usize) -> (Vec<Group>, Vec<Group>) {
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
		panic!("The brush benchmarks only know how to reach Valgrind on x86-64");
	}
}
