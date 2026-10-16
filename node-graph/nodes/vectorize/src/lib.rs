mod convert;

pub use convert::{ATTR_VTRACE_REPEAT, RepeatStructure};

use core_types::ATTR_TRANSFORM;
use core_types::context::Ctx;
use core_types::list::{Item, List};
use glam::{DAffine2, DVec2};
use graphic_types::Graphic;
use raster_types::{CPU, Raster};
use std::ops::ControlFlow;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// What Graphite's renderer draws of vtrace's model; the rest is lowered into it by vtrace.
pub const CAPABILITIES: vtrace::Capabilities = vtrace::Capabilities {
	even_odd: false,
	path_gradients: false,
	blur: false,
	radial_focus: false,
	clips: true,
	dashes: true,
	transparent_stops: true,
};

/// Counts the traces one node has started, kept as the node's own data, so a running trace stops once the same node starts a newer one
/// while other vectorize nodes tracing at the same time carry on.
#[derive(Debug, Clone, Default)]
pub struct TraceGeneration(Arc<AtomicU64>);

/// Traces an image into a stack of filled and stroked shapes, drawn bottom to top as an artist would layer them.
#[node_macro::node(category("Raster"), memoize)]
async fn vectorize(
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
	#[data] generation: TraceGeneration,
) -> List<Graphic> {
	let image_transform: DAffine2 = image.attribute_cloned_or_default(ATTR_TRANSFORM);
	let image = image.element();
	let (width, height) = (image.width as usize, image.height as usize);
	if width == 0 || height == 0 {
		return List::new();
	}

	// Graphite's colors are linear light with straight alpha; vtrace composites transparency over white
	let pixels: Vec<f32> = image.data.iter().flat_map(|color| [color.r(), color.g(), color.b(), color.a()]).collect();
	let source = vtrace::Image {
		pixels: vtrace::Pixels::RgbaF32 {
			data: &pixels,
			linear: true,
			premultiplied: false,
		},
		width,
		height,
	};
	let options = vtrace::TraceOptions {
		tolerance: *tolerance.element(),
		detail: vtrace::Detail::Scaled(*detail.element()),
		host: CAPABILITIES,
		..Default::default()
	};
	let traces = &generation.0;
	let this = traces.fetch_add(1, Ordering::Relaxed) + 1;
	let mut stage = "";
	let progress = |progress: &vtrace::Progress| {
		if progress.stage != stage {
			stage = progress.stage;
			log::debug!("Vectorize: {stage}");
		}
		if traces.load(Ordering::Relaxed) == this {
			ControlFlow::Continue(())
		} else {
			ControlFlow::Break(())
		}
	};
	let model = match vtrace::trace_with(&source, &options, progress) {
		Ok(model) => model,
		Err(vtrace::TraceError::Cancelled) => return List::new(),
		Err(error) => {
			log::error!("Vectorize could not trace the image: {error}");
			return List::new();
		}
	};

	// The traced coordinates are in pixels, while the image's transform maps the unit square onto the document
	let pixels_to_document = image_transform * DAffine2::from_scale(DVec2::new(1. / width as f64, 1. / height as f64));
	convert::model_to_graphic(&model, pixels_to_document)
}

#[cfg(test)]
mod test {
	use super::*;
	use core_types::Color;
	use raster_types::Image;

	/// An image of `width` by `height` pixels colored by `color` at each pixel's column and row.
	fn raster(width: u32, height: u32, color: impl Fn(u32, u32) -> Color) -> Raster<CPU> {
		let data = (0..width * height).map(|index| color(index % width, index / width)).collect();
		Raster::new_cpu(Image {
			width,
			height,
			data,
			base64_string: None,
		})
	}

	/// The shapes traced from `image` at `detail` and `tolerance`, by a node of its own.
	fn trace(image: Raster<CPU>, detail: f64, tolerance: f64) -> List<Graphic> {
		let (image, detail, tolerance) = (Item::new_from_element(image), Item::new_from_element(detail), Item::new_from_element(tolerance));
		futures::executor::block_on(vectorize((), &TraceGeneration::default(), image, detail, tolerance))
	}

	const DARK: Color = Color::from_rgbaf32_unchecked(0.05, 0.1, 0.4, 1.);

	/// A dark ring from `inner` to `inner + width` pixels out from the middle of a light square of `size` pixels.
	fn ring(size: u32, inner: f64, width: f64) -> Raster<CPU> {
		let center = size as f64 / 2.;
		raster(size, size, |x, y| {
			let distance = DVec2::new(x as f64 + 0.5, y as f64 + 0.5).distance(DVec2::splat(center));
			if (inner..inner + width).contains(&distance) { DARK } else { Color::WHITE }
		})
	}

	/// A dark ring on a light background, whose hole only an even-odd fill or a correctly wound nonzero fill leaves empty.
	#[test]
	fn traces_a_ring_into_shapes() {
		let traced = trace(ring(64, 12.8, 12.8), 1., 1.);
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
				let traced = trace(image.clone(), detail, tolerance);
				assert!(name != "flat" || !traced.is_empty(), "the flat raster traced into no shapes at detail {detail}, tolerance {tolerance}");
			}
		}
	}

	/// Two nodes tracing at once each finish their own trace: a trace stops only for a newer one of the same node.
	#[test]
	fn concurrent_nodes_do_not_cancel_each_other() {
		let traces: Vec<_> = (0..4).map(|_| std::thread::spawn(|| trace(ring(64, 12.8, 12.8), 1., 1.))).collect();
		for traced in traces {
			assert!(!traced.join().unwrap().is_empty(), "a trace was cancelled by another node's");
		}
	}
}
