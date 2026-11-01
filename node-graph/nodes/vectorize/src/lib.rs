mod convert;
mod slices;

pub use convert::{ATTR_OLLEV_REPEAT, RepeatStructure};
pub use slices::TraceSlot;

use core_types::list::{Item, List};
use core_types::runtime::Runtime;
use core_types::{ATTR_TRANSFORM, CacheHash, Ctx, Poll, RuntimeHandle, SourceId};
use glam::{DAffine2, DVec2};
use graphic_types::Graphic;
use raster_types::{CPU, Raster};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::Duration;

/// The parts of ollev's model Graphite renders natively. ollev lowers the rest into these.
pub const CAPABILITIES: ollev::Capabilities = ollev::Capabilities {
	even_odd: false,
	path_gradients: false,
	blur: false,
	radial_focus: false,
	clips: true,
	dashes: true,
	transparent_stops: true,
};

/// Tracing time per evaluation, leaving room in the frame for the render. ollev only checks it at checkpoints, which can
/// overrun it by about half a second on a large image.
const SLICE: Duration = Duration::from_millis(10);

/// A trace's peak memory per traced pixel, measured on a busy image.
const BYTES_PER_PIXEL: f64 = 800.;

/// The most memory a trace may take in the browser without the override, which is also the default limit everywhere.
const BROWSER_MAX_MEGABYTES: f64 = 1000.;

/// Traces an image into a stack of filled and stroked shapes, drawn bottom to top as an artist would layer them.
///
/// The trace runs in slices, one per evaluation, showing the shapes traced so far until it is done.
#[node_macro::node(category("Raster"), memoize)]
fn vectorize(
	_: impl Ctx,
	/// The image to trace.
	image: Item<Raster<CPU>>,
	/// How much detail to keep, as a multiple of the default level read from how busy the image is. Higher values spend more segments on smaller features.
	#[default(1.)]
	#[hard(0.01..)]
	detail: Item<f64>,
	/// How far the shapes may deviate from the image, as a multiple of the image's own noise. Higher values give fewer, simpler shapes.
	#[default(1.)]
	#[hard(0..)]
	tolerance: Item<f64>,
	/// Finds translucent layers and traces what they cover beneath them. Off, every region is drawn opaque.
	#[default(false)]
	translucency: Item<bool>,
	/// Draws lines as strokes along their centerlines, dashed and dotted ones included. Off, lines are filled shapes.
	#[default(false)]
	strokes: Item<bool>,
	/// Refits outlines as circles, ellipses and rectangles. Off, every outline stays a path.
	#[default(true)]
	primitives: Item<bool>,
	/// Merges shaded bands into gradients and draws glows. Off, edges stay hard.
	#[default(false)]
	effects: Item<bool>,
	/// Draws repeated shapes as instances of one shape. Off, every copy is a shape of its own.
	#[default(true)]
	instancing: Item<bool>,
	/// Fits gradients along a curve of a shape's boundary to shaded bands. Off, bands take linear or radial gradients.
	#[default(false)]
	path_gradients: Item<bool>,
	/// The most memory the trace may take: a larger image is traced at a size that fits and its shapes scaled back up. 0 traces every pixel.
	#[default(1000.)]
	#[hard(0..)]
	#[unit(" MB")]
	max_memory: Item<f64>,
	/// Allows a memory limit above 1000 MB, or none, in the browser, where the trace can run out of memory and crash the editor. The desktop app has no such limit.
	#[default(false)]
	exceed_browser_limit: Item<bool>,
	#[scope("runtime")] runtime: Item<RuntimeHandle>,
	#[source_id] source: Item<SourceId>,
	#[data] trace: TraceSlot,
) -> Poll<List<Graphic>> {
	let stages = ollev::Stages {
		translucency: *translucency.element(),
		strokes: *strokes.element(),
		primitives: *primitives.element(),
		effects: *effects.element(),
		instancing: *instancing.element(),
		path_gradients: *path_gradients.element(),
	};
	let max_megabytes = Some(*max_memory.element()).filter(|&megabytes| megabytes > 0.);
	if cfg!(target_family = "wasm") && !*exceed_browser_limit.element() && max_megabytes.is_none_or(|megabytes| megabytes > BROWSER_MAX_MEGABYTES) {
		trace.clear();
		return Poll::error(format!(
			"A memory limit above {BROWSER_MAX_MEGABYTES} MB, or none, can crash the editor in the browser. Lower the limit or turn on \"Exceed Browser Limit\"."
		));
	}
	let max_pixels = max_megabytes.map(|megabytes| (megabytes * 1e6 / BYTES_PER_PIXEL) as usize);
	let runtime = runtime.element();
	let state = trace_slice(&image, *detail.element(), *tolerance.element(), stages, max_pixels, trace, SLICE);
	if let Poll::Partial(_) = state {
		// Finishing an empty task bumps the node's generation, which re-evaluates it for the next slice
		runtime.spawn(*source.element(), Box::pin(async {}));
	}
	state
}

/// Runs one slice of the trace held in `slot`, starting a new trace if `slot` holds none for these inputs.
fn trace_slice(image: &Item<Raster<CPU>>, detail: f64, tolerance: f64, stages: ollev::Stages, max_pixels: Option<usize>, slot: &TraceSlot, slice: Duration) -> Poll<List<Graphic>> {
	let image_transform: DAffine2 = image.attribute_cloned_or_default(ATTR_TRANSFORM);
	let raster = image.element();
	let (width, height) = (raster.width as usize, raster.height as usize);
	if width == 0 || height == 0 {
		slot.clear();
		return Poll::Final(List::new());
	}

	let mut hasher = DefaultHasher::new();
	image.cache_hash(&mut hasher);
	(detail.to_bits(), tolerance.to_bits(), max_pixels).hash(&mut hasher);
	[stages.translucency, stages.strokes, stages.primitives, stages.effects, stages.instancing, stages.path_gradients].hash(&mut hasher);

	let start = || {
		let source = ollev::Image {
			pixels: ollev::Pixels::RgbaF32 {
				data: bytemuck::cast_slice(&raster.data),
				linear: true,
				premultiplied: false,
			},
			width,
			height,
		};
		let options = ollev::TraceOptions {
			tolerance,
			detail: ollev::Detail::Scaled(detail),
			max_pixels,
			host: CAPABILITIES,
			stages,
			..Default::default()
		};
		ollev::Tracing::new(&source, &options)
	};

	let pixels_to_document = image_transform * DAffine2::from_scale(DVec2::new(1. / width as f64, 1. / height as f64));
	slot.poll(hasher.finish(), slice, start, |model| convert::model_to_graphic(model, pixels_to_document))
}

#[cfg(test)]
mod test {
	use super::*;
	use core_types::Color;
	use core_types::runtime::{AsyncRuntime, Spawn, Task};
	use raster_types::Image;
	use std::sync::{Arc, Mutex};
	use std::task::{Context, Waker};

	/// An image of `width` by `height` pixels colored by `color` at each pixel's column and row.
	fn raster(width: u32, height: u32, color: impl Fn(u32, u32) -> Color) -> Item<Raster<CPU>> {
		let data = (0..width * height).map(|index| color(index % width, index / width)).collect();
		Item::new_from_element(Raster::new_cpu(Image { width, height, data }))
	}

	/// One slice of `image` traced at default settings by the node holding `slot`.
	fn slice(image: &Item<Raster<CPU>>, slot: &TraceSlot) -> Poll<List<Graphic>> {
		trace_slice(image, 1., 1., ollev::Stages::ALL, None, slot, SLICE)
	}

	/// The shapes traced from `image` at `detail` and `tolerance` by a node of its own, slice by slice.
	fn trace(image: &Item<Raster<CPU>>, detail: f64, tolerance: f64) -> List<Graphic> {
		let slot = TraceSlot::default();
		loop {
			match trace_slice(image, detail, tolerance, ollev::Stages::ALL, None, &slot, SLICE) {
				Poll::Partial(_) => {}
				Poll::Final(traced) => return traced,
				state => panic!("the trace ended in {state:?}"),
			}
		}
	}

	const DARK: Color = Color::from_rgbaf32_unchecked(0.05, 0.1, 0.4, 1.);

	/// A dark ring from `inner` to `inner + width` pixels out from the middle of a light square of `size` pixels.
	fn ring(size: u32, inner: f64, width: f64) -> Item<Raster<CPU>> {
		let center = size as f64 / 2.;
		raster(size, size, |x, y| {
			let distance = DVec2::new(x as f64 + 0.5, y as f64 + 0.5).distance(DVec2::splat(center));
			if (inner..inner + width).contains(&distance) { DARK } else { Color::WHITE }
		})
	}

	/// A dark ring on a light background, whose hole only an even-odd fill or a correctly wound nonzero fill leaves empty.
	#[test]
	fn traces_a_ring_into_shapes() {
		let traced = trace(&ring(64, 12.8, 12.8), 1., 1.);
		assert!(!traced.is_empty(), "the ring traced into no shapes");
	}

	/// Rasters with too few pixels, too little contrast or too little room for a shape, which used to trip glam's zero-length
	/// vector asserts in debug builds.
	#[test]
	fn traces_degenerate_rasters() {
		let gray = |value: f32| Color::from_rgbaf32_unchecked(value, value, value, 1.);
		let rasters = [
			("1x1", raster(1, 1, |_, _| DARK)),
			("1x7", raster(1, 7, |_, y| if y % 2 == 0 { DARK } else { Color::WHITE })),
			("7x1", raster(7, 1, |x, _| gray(x as f32 / 6.))),
			("ring 1 px", ring(24, 6., 1.)),
			("ring 2 px", ring(24, 6., 2.)),
			("ring 3 px", ring(24, 6., 3.)),
			("flat", raster(16, 16, |_, _| DARK)),
			("clear", raster(16, 16, |_, _| Color::TRANSPARENT)),
			("half alpha", raster(16, 16, |x, _| if x < 8 { DARK.with_alpha(0.5) } else { Color::TRANSPARENT })),
			("checker", raster(16, 16, |x, y| if (x + y) % 2 == 0 { DARK } else { Color::WHITE })),
			("ramp", raster(24, 8, |x, _| gray(x as f32 / 23.))),
			("diagonals", raster(16, 16, |x, y| if (x + y) % 5 == 0 || x == y { DARK } else { Color::WHITE })),
			("sparse dots", raster(24, 24, |x, y| if x % 7 == 3 && y % 7 == 3 { DARK } else { Color::WHITE })),
		];
		for (name, image) in rasters {
			for (detail, tolerance) in [(0.25, 4.), (1., 1.), (4., 0.)] {
				let traced = trace(&image, detail, tolerance);
				assert!(name != "flat" || !traced.is_empty(), "the flat raster traced into no shapes at detail {detail}, tolerance {tolerance}");
			}
		}
	}

	/// Shapes partly over one another: rings, a disc and a ramp, enough to keep a trace busy for many short slices.
	fn busy(size: u32) -> Item<Raster<CPU>> {
		let center = DVec2::splat(size as f64 / 2.);
		raster(size, size, |x, y| {
			let point = DVec2::new(x as f64 + 0.5, y as f64 + 0.5);
			if (point - center * 0.6).length() < size as f64 / 8. {
				Color::from_rgbaf32_unchecked(0.8, 0.3, 0.1, 1.)
			} else if (point - center).length() % 24. < 6. {
				DARK
			} else {
				Color::from_rgbaf32_unchecked(0.9, 0.9, x as f32 / size as f32, 1.)
			}
		})
	}

	/// The shapes traced so far arrive as a partial result before the final one.
	#[test]
	fn shows_partial_scenes_before_the_final_result() {
		let (image, slot) = (busy(96), TraceSlot::default());
		let (mut slices, mut partials) = (0, 0);
		let traced = loop {
			slices += 1;
			match slice(&image, &slot) {
				Poll::Partial(shown) => partials += usize::from(!shown.is_empty()),
				Poll::Final(traced) => break traced,
				state => panic!("the trace ended in {state:?}"),
			}
		};
		assert!(slices > 2 && partials > 0, "{slices} slices, {partials} of them showing shapes");
		assert!(!traced.is_empty());
	}

	/// Interleaved slices of two nodes each finish their own trace.
	#[test]
	fn interleaved_nodes_do_not_cancel_each_other() {
		let image = ring(64, 12.8, 12.8);
		let slots = [TraceSlot::default(), TraceSlot::default()];
		let mut traced = [None, None];
		while traced.iter().any(Option::is_none) {
			for (slot, traced) in slots.iter().zip(&mut traced).filter(|(_, traced)| traced.is_none()) {
				if let Poll::Final(shapes) = slice(&image, slot) {
					*traced = Some(shapes);
				}
			}
		}
		assert_eq!(traced[0], traced[1], "two traces of one image differ");
		assert_eq!(traced[0], Some(trace(&image, 1., 1.)));
	}

	/// New inputs mid-trace drop the running trace and trace them instead, and dropping the node drops its trace.
	#[test]
	fn new_inputs_and_a_dropped_node_cancel_the_trace() {
		let slot = TraceSlot::default();
		assert!(matches!(slice(&busy(96), &slot), Poll::Partial(_)));
		assert!(slot.tracing());

		let image = ring(64, 12.8, 12.8);
		let traced = loop {
			if let Poll::Final(traced) = slice(&image, &slot) {
				break traced;
			}
		};
		assert_eq!(traced, trace(&image, 1., 1.));
		assert!(!slot.tracing(), "the trace of the former image is still running");

		assert!(matches!(slice(&busy(96), &slot), Poll::Partial(_)), "the former image's trace was resumed rather than started anew");
		let released = slot.released();
		let clone = slot.clone();
		drop(slot);
		assert!(!released(), "a clone of the node dropped the trace");
		drop(clone);
		assert!(released(), "the dropped node's trace is still held");
	}

	#[test]
	fn an_empty_image_drops_the_trace() {
		let slot = TraceSlot::default();
		assert!(matches!(slice(&busy(96), &slot), Poll::Partial(_)));
		assert!(matches!(slice(&raster(0, 0, |_, _| DARK), &slot), Poll::Final(_)));
		assert!(!slot.tracing(), "the former image's trace is still held");
	}

	/// A first slice long enough to reach ollev's first showable checkpoint already shows a first look at the image.
	#[test]
	fn the_first_evaluation_shows_shapes() {
		let slot = TraceSlot::default();
		match trace_slice(&busy(256), 1., 1., ollev::Stages::ALL, None, &slot, Duration::from_millis(200)) {
			Poll::Partial(shown) => assert!(!shown.is_empty(), "the first partial result shows no shapes"),
			state => panic!("the first slice ended in {state:?}"),
		}
	}

	#[derive(Default)]
	struct CollectSpawner(Mutex<Vec<Task>>);

	impl Spawn for CollectSpawner {
		fn spawn(&self, task: Task) {
			self.0.lock().unwrap().push(task);
		}
	}

	impl CollectSpawner {
		/// Polls each spawned task once, telling how many finished.
		fn drain(&self) -> usize {
			let tasks = std::mem::take(&mut *self.0.lock().unwrap());
			tasks
				.into_iter()
				.map(|mut task| task.as_mut().poll(&mut Context::from_waker(Waker::noop())).is_ready())
				.filter(|&ready| ready)
				.count()
		}
	}

	/// A runtime whose tasks are collected, with `SOURCE` live.
	fn collecting_runtime() -> (Arc<CollectSpawner>, AsyncRuntime) {
		let spawner = Arc::new(CollectSpawner::default());
		let runtime = AsyncRuntime::new(spawner.clone());
		runtime.retain_sources(&[SOURCE]);
		(spawner, runtime)
	}

	const SOURCE: SourceId = SourceId(7);

	/// One evaluation of the node holding `slot` on `image`, with every stage on.
	fn evaluate(slot: &TraceSlot, image: &Item<Raster<CPU>>, runtime: &AsyncRuntime) -> Poll<List<Graphic>> {
		let one = || Item::new_from_element(1.);
		let on = || Item::new_from_element(true);
		vectorize(
			(),
			slot,
			image.clone(),
			one(),
			one(),
			on(),
			on(),
			on(),
			on(),
			on(),
			on(),
			Item::new_from_element(BROWSER_MAX_MEGABYTES),
			Item::new_from_element(false),
			Item::new_from_element(runtime.handle()),
			Item::new_from_element(SOURCE),
		)
	}

	/// Each partial result spawns a task whose end bumps the node's generation, so the editor evaluates the node again.
	#[test]
	fn a_partial_result_asks_for_the_next_slice() {
		let (spawner, runtime) = collecting_runtime();
		let (image, slot) = (busy(96), TraceSlot::default());
		let mut generation = 0;
		loop {
			let state = evaluate(&slot, &image, &runtime);
			let partial = matches!(state, Poll::Partial(_));
			assert_eq!(spawner.drain(), usize::from(partial));
			generation += u64::from(partial);
			assert_eq!(&*runtime.snapshot(), &[(SOURCE, generation)]);
			if !partial {
				assert!(matches!(state, Poll::Final(_)));
				break;
			}
		}
		assert!(generation > 0, "the trace took a single slice");
	}

	/// New inputs leave the former trace's request for its next slice, which coalesces with the new one into one evaluation.
	#[test]
	fn new_inputs_ask_for_their_own_slice() {
		let (spawner, runtime) = collecting_runtime();
		let slot = TraceSlot::default();
		assert!(matches!(evaluate(&slot, &busy(96), &runtime), Poll::Partial(_)));
		assert!(matches!(evaluate(&slot, &busy(128), &runtime), Poll::Partial(_)));

		assert_eq!(spawner.drain(), 2);
	}
}
