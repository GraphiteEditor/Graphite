mod bands;
mod convert;

pub use convert::{ATTR_VTRACE_BLUR, ATTR_VTRACE_PATH_GRADIENT, ATTR_VTRACE_RADIAL_FOCUS, ATTR_VTRACE_REPEAT, PathGradient, RepeatStructure};

use core_types::ATTR_TRANSFORM;
use core_types::context::Ctx;
use core_types::list::{Item, List};
use glam::{DAffine2, DVec2};
use graphic_types::Graphic;
use raster_types::{CPU, Raster};

/// Traces an image into a stack of filled and stroked shapes, drawn bottom to top as an artist would layer them.
#[node_macro::node(category("Raster"))]
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
) -> List<Graphic> {
	let image_transform: DAffine2 = image.attribute_cloned_or_default(ATTR_TRANSFORM);
	let image = image.element();
	let (width, height) = (image.width as usize, image.height as usize);
	if width == 0 || height == 0 {
		return List::new();
	}

	// vtrace reads opaque gamma-encoded RGB, so transparent areas are composited over white
	let rgb: Vec<u8> = image
		.data
		.iter()
		.flat_map(|pixel| {
			let [red, green, blue, alpha] = pixel.to_gamma_srgb_channels();
			let alpha = alpha.clamp(0., 1.);
			[red, green, blue].map(|channel| ((channel * alpha + 1. - alpha).clamp(0., 1.) * 255.).round() as u8)
		})
		.collect();

	let detail = vtrace::auto_detail(&rgb, width, height) * *detail.element();
	let options = vtrace::variants::plan::Options {
		tolerance: Some(*tolerance.element()),
		..Default::default()
	};
	let scene = vtrace::variants::plan::trace(&rgb, width, height, detail, options, |_| {});

	// The traced coordinates are in pixels, while the image's transform maps the unit square onto the document
	let pixels_to_document = image_transform * DAffine2::from_scale(DVec2::new(1. / width as f64, 1. / height as f64));
	convert::model_to_graphic(&scene.model(), pixels_to_document)
}

#[cfg(test)]
mod test {
	use super::*;
	use core_types::Color;
	use raster_types::Image;

	/// A dark ring on a light background, whose hole only an even-odd fill or a correctly wound nonzero fill leaves empty.
	fn ring(size: u32) -> Raster<CPU> {
		let center = size as f64 / 2.;
		let data = (0..size * size)
			.map(|index| {
				let position = DVec2::new((index % size) as f64 + 0.5, (index / size) as f64 + 0.5);
				let distance = position.distance(DVec2::splat(center));
				let in_ring = (size as f64 * 0.2..size as f64 * 0.4).contains(&distance);
				if in_ring { Color::from_rgbaf32_unchecked(0.05, 0.1, 0.4, 1.) } else { Color::WHITE }
			})
			.collect();
		Raster::new_cpu(Image {
			width: size,
			height: size,
			data,
			base64_string: None,
		})
	}

	#[test]
	fn traces_a_ring_into_shapes() {
		let image = Item::new_from_element(ring(64));
		let traced = futures::executor::block_on(vectorize((), image, Item::new_from_element(1.), Item::new_from_element(1.)));
		assert!(!traced.is_empty(), "the ring traced into no shapes");
	}
}
