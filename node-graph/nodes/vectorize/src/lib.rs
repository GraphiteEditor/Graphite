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
use slices::Budget;
use std::hash::{DefaultHasher, Hash, Hasher};

/// What Graphite's renderer draws of ollev's model; the rest is lowered into it by ollev.
pub const CAPABILITIES: ollev::Capabilities = ollev::Capabilities {
	even_odd: false,
	path_gradients: false,
	blur: false,
	radial_focus: false,
	clips: true,
	dashes: true,
	transparent_stops: true,
};

/// A quarter second of tracing per evaluation, which ollev's checkpoints overrun to about a second. Lowering a scene for Graphite
/// takes 0.5 to 5 s, so a partial scene is shown after 2 s of tracing once the last lowering took at most a tenth of the tracing since.
const BUDGET: Budget = Budget {
	slice: 0.25,
	partial_every: 2.,
	lowering_share: 0.1,
};

/// Traces an image into a stack of filled and stroked shapes, drawn bottom to top as an artist would layer them.
///
/// The trace runs in slices, one per evaluation, showing the shapes traced so far until it is done.
#[node_macro::node(category("Raster"), memoize)]
fn vectorize(
	_: impl Ctx,
	/// The image to trace.
	image: Item<Raster<CPU>>,
	/// How much detail to keep, as a multiple of the level estimated from how busy the image is. Higher values spend more segments on smaller features.
	#[default(1.)]
	#[hard(0.01..)]
	detail: Item<f64>,
	/// How far the shapes may deviate from the image, as a multiple of the image's own noise. Higher values give fewer, simpler shapes.
	#[default(1.)]
	#[hard(0..)]
	tolerance: Item<f64>,
	/// Finds translucent layers and traces what they cover beneath them. Off, every region is drawn opaque.
	#[default(true)]
	translucency: Item<bool>,
	/// Draws lines as strokes along their centerlines, dashed and dotted ones included. Off, lines are filled shapes.
	#[default(true)]
	strokes: Item<bool>,
	/// Refits outlines as circles, ellipses and rectangles. Off, every outline stays a path.
	#[default(true)]
	primitives: Item<bool>,
	/// Merges shaded bands into gradients and draws glows. Off, edges stay hard.
	#[default(true)]
	effects: Item<bool>,
	/// Draws repeated shapes as instances of one shape. Off, every copy is a shape of its own.
	#[default(true)]
	instancing: Item<bool>,
	#[scope("runtime")] runtime: Item<RuntimeHandle>,
	#[source_id] _source: Item<SourceId>,
	#[data] trace: TraceSlot,
) -> Poll<List<Graphic>> {
	let stages = ollev::Stages {
		translucency: *translucency.element(),
		strokes: *strokes.element(),
		primitives: *primitives.element(),
		effects: *effects.element(),
		instancing: *instancing.element(),
	};
	let state = trace_slice(&image, *detail.element(), *tolerance.element(), stages, trace, BUDGET);
	if let Poll::Partial(_) = state {
		// The task's end bumps this node's generation, so the editor evaluates the graph again and the next slice runs
		runtime.element().spawn(*_source.element(), Box::pin(async {}));
	}
	state
}

/// One slice of the trace of `image` held in `slot`, which starts it when the slot holds no trace of these inputs.
fn trace_slice(image: &Item<Raster<CPU>>, detail: f64, tolerance: f64, stages: ollev::Stages, slot: &TraceSlot, budget: Budget) -> Poll<List<Graphic>> {
	let image_transform: DAffine2 = image.attribute_cloned_or_default(ATTR_TRANSFORM);
	let raster = image.element();
	let (width, height) = (raster.width as usize, raster.height as usize);
	if width == 0 || height == 0 {
		return Poll::Final(List::new());
	}

	let mut hasher = DefaultHasher::new();
	image.cache_hash(&mut hasher);
	(detail.to_bits(), tolerance.to_bits()).hash(&mut hasher);
	[stages.translucency, stages.strokes, stages.primitives, stages.effects, stages.instancing].hash(&mut hasher);

	let start = || {
		// Graphite's colors are linear light with straight alpha; ollev composites transparency over white
		let pixels: Vec<f32> = raster.data.iter().flat_map(|color| [color.r(), color.g(), color.b(), color.a()]).collect();
		let source = ollev::Image {
			pixels: ollev::Pixels::RgbaF32 {
				data: &pixels,
				linear: true,
				premultiplied: false,
			},
			width,
			height,
		};
		let options = ollev::TraceOptions {
			tolerance,
			detail: ollev::Detail::Scaled(detail),
			host: CAPABILITIES,
			stages,
			..Default::default()
		};
		ollev::Tracing::new(&source, &options)
	};

	// The traced coordinates are in pixels, while the image's transform maps the unit square onto the document
	let pixels_to_document = image_transform * DAffine2::from_scale(DVec2::new(1. / width as f64, 1. / height as f64));
	slot.poll(hasher.finish(), budget, start, |model| convert::model_to_graphic(model, pixels_to_document))
}

#[cfg(test)]
mod test {
	use super::*;
	use core_types::Color;
	use core_types::runtime::{AsyncRuntime, Spawn, Task};
	use raster_types::Image;
	use std::sync::{Arc, Mutex};
	use std::task::{Context, Waker};

	/// Slices short enough that every image takes several, each showing the scene where one is offered.
	const SHORT: Budget = Budget {
		slice: 0.01,
		partial_every: 0.,
		lowering_share: 1e9,
	};

	/// An image of `width` by `height` pixels colored by `color` at each pixel's column and row.
	fn raster(width: u32, height: u32, color: impl Fn(u32, u32) -> Color) -> Item<Raster<CPU>> {
		let data = (0..width * height).map(|index| color(index % width, index / width)).collect();
		Item::new_from_element(Raster::new_cpu(Image {
			width,
			height,
			data,
			base64_string: None,
		}))
	}

	/// One slice of `image` traced at default settings by the node holding `slot`.
	fn slice(image: &Item<Raster<CPU>>, slot: &TraceSlot) -> Poll<List<Graphic>> {
		trace_slice(image, 1., 1., ollev::Stages::ALL, slot, SHORT)
	}

	/// The shapes traced from `image` at `detail` and `tolerance` by a node of its own, slice by slice.
	fn trace(image: &Item<Raster<CPU>>, detail: f64, tolerance: f64) -> List<Graphic> {
		let slot = TraceSlot::default();
		loop {
			match trace_slice(image, detail, tolerance, ollev::Stages::ALL, &slot, SHORT) {
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

	/// Rasters with too few pixels, too little contrast, or too little room for a shape, which panicked in debug builds (glam's
	/// asserts on vectors without a direction), each traced at a few detail and tolerance settings.
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
			let ring = (point - center).length() % 24.;
			match () {
				_ if (point - center * 0.6).length() < size as f64 / 8. => Color::from_rgbaf32_unchecked(0.8, 0.3, 0.1, 1.),
				_ if ring < 6. => DARK,
				_ => Color::from_rgbaf32_unchecked(0.9, 0.9, x as f32 / size as f32, 1.),
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
		assert_eq!(slices::held(), 1);

		let image = ring(64, 12.8, 12.8);
		let traced = loop {
			if let Poll::Final(traced) = slice(&image, &slot) {
				break traced;
			}
			assert_eq!(slices::held(), 1, "the trace of the former image is still held");
		};
		assert_eq!(traced, trace(&image, 1., 1.));

		assert!(matches!(slice(&busy(96), &slot), Poll::Partial(_)));
		drop(slot);
		assert_eq!(slices::held(), 0, "the dropped node's trace is still held");
	}

	#[derive(Default)]
	struct CollectSpawner(Mutex<Vec<Task>>);

	impl Spawn for CollectSpawner {
		fn spawn(&self, task: Task) {
			self.0.lock().unwrap().push(task);
		}
	}

	/// Each partial result spawns a task whose end bumps the node's generation, so the editor evaluates the node again.
	#[test]
	fn a_partial_result_asks_for_the_next_slice() {
		let spawner = Arc::new(CollectSpawner::default());
		let runtime = AsyncRuntime::new(spawner.clone());
		let source = SourceId(7);
		runtime.retain_sources(&[source]);

		let (image, slot) = (busy(96), TraceSlot::default());
		let mut generation = 0;
		loop {
			let state = vectorize(
				(),
				&slot,
				image.clone(),
				Item::new_from_element(1.),
				Item::new_from_element(1.),
				Item::new_from_element(true),
				Item::new_from_element(true),
				Item::new_from_element(true),
				Item::new_from_element(true),
				Item::new_from_element(true),
				Item::new_from_element(runtime.handle()),
				Item::new_from_element(source),
			);
			for mut task in std::mem::take(&mut *spawner.0.lock().unwrap()) {
				assert!(task.as_mut().poll(&mut Context::from_waker(Waker::noop())).is_ready());
			}
			let partial = matches!(state, Poll::Partial(_));
			generation += u64::from(partial);
			assert_eq!(&*runtime.snapshot(), &[(source, generation)]);
			if !partial {
				assert!(matches!(state, Poll::Final(_)));
				break;
			}
		}
		assert!(generation > 0, "the trace took a single slice");
	}
}
