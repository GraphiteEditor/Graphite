use core::cmp::Ordering;
use core_types::list::Item;
use glam::{DAffine2, DVec2};
use no_std_types::Ctx;
use no_std_types::blending::BlendMode;
use no_std_types::color::{Color, Pixel};
use raster_types::parallel_rows::fill_rows;
use raster_types::{CPU, Image, Raster};
use vector_types::{Gradient, GradientStop};

pub trait Blend<P: Pixel>: Sized {
	/// Blends `self` over `under`, each placed by its transform where placement matters.
	fn blend(&self, transform: DAffine2, under: &Self, under_transform: DAffine2, blend_fn: impl Fn(P, P) -> P + Sync) -> Self;

	/// The item with any effects it carries carried out into its content, so blending sees what it looks like.
	fn with_effects_applied(item: Item<Self>) -> Item<Self>;
}
impl Blend<Color> for Color {
	fn with_effects_applied(item: Item<Self>) -> Item<Self> {
		graphic_types::evaluate_color_effects(item)
	}

	fn blend(&self, _transform: DAffine2, under: &Self, _under_transform: DAffine2, blend_fn: impl Fn(Color, Color) -> Color + Sync) -> Self {
		blend_fn(*self, *under)
	}
}

impl Blend<Color> for Raster<CPU> {
	fn with_effects_applied(item: Item<Self>) -> Item<Self> {
		graphic_types::evaluate_raster_effects(item)
	}

	/// Each pixel blends with what lies beneath its center in `under`, so the two images needn't share a size or placement. O(width × height).
	fn blend(&self, transform: DAffine2, under: &Self, under_transform: DAffine2, blend_fn: impl Fn(Color, Color) -> Color + Sync) -> Self {
		// Maps this image's pixel coordinates onto the under image's, through the unit square each transform places
		let pixels_to_unit = |image: &Image<Color>| DAffine2::from_scale(1. / DVec2::new(image.width as f64, image.height as f64));
		let under_placement = (under_transform.matrix2.determinant() != 0.).then(|| pixels_to_unit(under).inverse() * under_transform.inverse() * transform * pixels_to_unit(self));

		let mut data = vec![Color::TRANSPARENT; self.data.len()];
		fill_rows(&mut data, self.width as usize, |y, row| {
			for (x, pixel) in row.iter_mut().enumerate() {
				let over = self.data[y * self.width as usize + x];
				let center = DVec2::new(x as f64 + 0.5, y as f64 + 0.5);
				let beneath = under_placement.map_or(Color::TRANSPARENT, |placement| under.sample_bilinear(placement.transform_point2(center)));
				*pixel = blend_fn(over, beneath);
			}
		});

		Raster::new_cpu(Image {
			data,
			width: self.width,
			height: self.height,
		})
	}
}

impl Blend<Color> for Gradient {
	fn with_effects_applied(item: Item<Self>) -> Item<Self> {
		graphic_types::evaluate_gradient_effects(item)
	}

	// TODO: This joining is unfaithful in several ways: it samples only at stop positions so midpoint curves flatten away;
	// TODO: it evaluates both sources with default whole-ramp attributes rather than their own (which this element-level impl cannot read);
	// TODO: and the output keeps over's attributes despite being sampled with defaults
	fn blend(&self, _transform: DAffine2, under: &Self, _under_transform: DAffine2, blend_fn: impl Fn(Color, Color) -> Color + Sync) -> Self {
		let mut combined_stops = self.positions(false).into_iter().chain(under.positions(false)).collect::<Vec<_>>();
		combined_stops.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
		combined_stops.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
		let over_evaluator = self.evaluator(Default::default());
		let under_evaluator = under.evaluator(Default::default());
		let stops = combined_stops.into_iter().map(|position| {
			let color = blend_fn(over_evaluator.evaluate(position), under_evaluator.evaluate(position));
			GradientStop { position, midpoint: 0.5, color }
		});

		// Positions stay explicit because eliding them needs the cyclic flag this impl can't read, and a wrong guess would relocate the stops
		Gradient::new(stops)
	}
}

pub use no_std_types::blending::{apply_blend_mode, blend_colors};

#[node_macro::node(category("Raster"), icon("NodeMix"), memoize)]
fn mix<T: Blend<Color> + Send>(
	_: impl Ctx,
	#[implementations(
		Raster<CPU>,
		Color,
		Gradient,
	)]
	over: Item<T>,
	#[expose]
	#[implementations(
		Raster<CPU>,
		Color,
		Gradient,
	)]
	under: Item<T>,
	blend_mode: Item<BlendMode>,
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(100.)]
	opacity: Item<f64>,
) -> Item<T> {
	let mut over = T::with_effects_applied(over);
	let under = T::with_effects_applied(under);
	let blend_mode = blend_mode.into_element();
	let opacity = (opacity.into_element() / 100.) as f32;

	let over_transform: DAffine2 = over.attribute_cloned_or_default(core_types::ATTR_TRANSFORM);
	let under_transform: DAffine2 = under.attribute_cloned_or_default(core_types::ATTR_TRANSFORM);
	let blended = over.element().blend(over_transform, under.element(), under_transform, |a, b| blend_colors(a, b, blend_mode, opacity));
	*over.element_mut() = blended;
	over
}

#[cfg(test)]
mod test {
	use core_types::color::Color;
	use raster_types::Image;
	use raster_types::Raster;

	#[test]
	fn mix_places_each_image_by_its_transform() {
		use super::Blend;
		use glam::{DAffine2, DVec2};

		// A 2x2 image over a 2-unit square, and a 1x1 red image covering only its bottom right unit
		let over = Raster::new_cpu(Image::new(2, 2, Color::WHITE));
		let under = Raster::new_cpu(Image::new(1, 1, Color::RED));
		let over_transform = DAffine2::from_scale(DVec2::splat(2.));
		let under_transform = DAffine2::from_translation(DVec2::ONE);

		// Reports what lies beneath each pixel
		let beneath = over.blend(over_transform, &under, under_transform, |_, under| under);

		assert_eq!((beneath.width, beneath.height), (2, 2));
		assert_eq!(beneath.data[3], Color::RED, "the bottom right pixel's center lies within the under image");
		for pixel in &beneath.data[..3] {
			assert_eq!(*pixel, Color::TRANSPARENT, "the other pixels lie outside it");
		}
	}
}
