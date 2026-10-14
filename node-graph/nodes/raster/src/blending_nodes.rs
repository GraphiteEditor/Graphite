use crate::adjust::Adjust;
use core::cmp::Ordering;
use core_types::list::Item;
use glam::{DAffine2, DVec2};
use graphic_types::Adjustment;
use no_std_types::Ctx;
use no_std_types::blending::BlendMode;
use no_std_types::color::{Color, Pixel};
use raster_types::parallel_rows::fill_rows;
use raster_types::{CPU, Image, Raster};
use vector_types::{Gradient, GradientStop};

pub trait Blend<P: Pixel> {
	/// Blends `self` over `under`, each placed by its transform where placement matters.
	fn blend(&self, transform: DAffine2, under: &Self, under_transform: DAffine2, blend_fn: impl Fn(P, P) -> P + Sync) -> Self;
}
impl Blend<Color> for Color {
	fn blend(&self, _transform: DAffine2, under: &Self, _under_transform: DAffine2, blend_fn: impl Fn(Color, Color) -> Color + Sync) -> Self {
		blend_fn(*self, *under)
	}
}

impl Blend<Color> for Raster<CPU> {
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
				let beneath = under_placement.map_or(Color::TRANSPARENT, |placement| sample_bilinear(under, placement.transform_point2(center)));
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

/// The image's color at a point in its pixel coordinates, interpolating the four nearest pixel centers by their alpha, with transparency outside it.
fn sample_bilinear(image: &Image<Color>, point: DVec2) -> Color {
	let corner = (point - 0.5).floor();
	let fraction = (point - 0.5 - corner).as_vec2();

	let premultiplied_at = |offset_x: i64, offset_y: i64| {
		let (x, y) = (corner.x as i64 + offset_x, corner.y as i64 + offset_y);
		if x < 0 || y < 0 || x >= image.width as i64 || y >= image.height as i64 {
			return [0.; 4];
		}
		let color = image.data[(y * image.width as i64 + x) as usize];
		[color.r() * color.a(), color.g() * color.a(), color.b() * color.a(), color.a()]
	};
	let lerp = |a: [f32; 4], b: [f32; 4], t: f32| std::array::from_fn::<f32, 4, _>(|channel| a[channel] + (b[channel] - a[channel]) * t);

	let top = lerp(premultiplied_at(0, 0), premultiplied_at(1, 0), fraction.x);
	let bottom = lerp(premultiplied_at(0, 1), premultiplied_at(1, 1), fraction.x);
	let [red, green, blue, alpha] = lerp(top, bottom, fraction.y);

	match alpha > 0. {
		true => Color::from_rgbaf32_unchecked(red / alpha, green / alpha, blue / alpha, alpha),
		false => Color::TRANSPARENT,
	}
}

impl Blend<Color> for Gradient {
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
	let mut over = over;
	let blend_mode = blend_mode.into_element();
	let opacity = (opacity.into_element() / 100.) as f32;

	let over_transform: DAffine2 = over.attribute_cloned_or_default(core_types::ATTR_TRANSFORM);
	let under_transform: DAffine2 = under.attribute_cloned_or_default(core_types::ATTR_TRANSFORM);
	let blended = over.element().blend(over_transform, under.element(), under_transform, |a, b| blend_colors(a, b, blend_mode, opacity));
	*over.element_mut() = blended;
	over
}

// TODO: Rename to "Paint Overlay" and take a `Graphic` paint like the `Fill` node, enabling this to serve the cases of Color Overlay, Gradient Overlay, and Pattern Overlay.
#[node_macro::node(category("Raster: Adjustment"), icon("NodeColorOverlay"))]
fn color_overlay<T: Adjust>(
	_: impl Ctx,
	#[implementations(
		Raster<CPU>,
		Color,
		Gradient,
	)]
	image: Item<T>,
	#[default(Color::BLACK)] color: Item<Color>,
	blend_mode: Item<BlendMode>,
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(100.)]
	opacity: Item<f64>,
) -> Item<T> {
	let mut image = image;
	let adjustment = Adjustment::ColorOverlay {
		color: color.into_element(),
		blend_mode: blend_mode.into_element(),
		opacity: opacity.into_element(),
	};
	image.element_mut().adjust(&adjustment);
	image
}

#[cfg(test)]
mod test {
	use core_types::blending::BlendMode;
	use core_types::color::Color;
	use core_types::list::Item;
	use raster_types::Image;
	use raster_types::Raster;

	#[test]
	fn color_overlay_multiply() {
		let image_color = Color::from_rgbaf32_unchecked(0.7, 0.6, 0.5, 0.4);
		let image = Image::new(1, 1, image_color);

		// Color { red: 0., green: 1., blue: 0., alpha: 1. }
		let overlay_color = Color::GREEN;

		// 100% of the output should come from the multiplied value
		let opacity = 100.;

		let result = super::color_overlay(
			(),
			Item::new_from_element(Raster::new_cpu(image.clone())),
			overlay_color.into(),
			BlendMode::Multiply.into(),
			opacity.into(),
		);
		let result = result.into_element();

		// The output should just be the original green and alpha channels (as we multiply them by 1 and other channels by 0)
		assert_eq!(result.data[0], Color::from_rgbaf32_unchecked(0., image_color.g(), 0., image_color.a()));
	}

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
