use crate::adjustments::{CellularDistanceFunction, CellularReturnType, DomainWarpType, FractalType, NoiseType};
use core_types::ATTR_TRANSFORM;
use core_types::color::Color;
use core_types::color::{Alpha, AlphaMut, LinearChannel, Luminance, RGBMut};
use core_types::context::Ctx;
use core_types::list::Item;
use core_types::math::bbox::Bbox;
use core_types::transform::Transform;
use dyn_any::DynAny;
use glam::{DAffine2, DVec2, UVec2};
use graphene_resource::Resource;
use graphic_types::generator::{Generator, NoiseSettings};
use raster_kernels::evaluate_raster_effects;
use raster_types::parallel_rows::fill_rows;
use raster_types::{Bitmap, BitmapMut};
use raster_types::{CPU, Raster};
use raster_types::{Image, ImageError};
use std::fmt::Debug;
use std::hash::Hash;

#[derive(Debug, DynAny)]
pub enum Error {
	IO(std::io::Error),
	Image(::image::ImageError),
}

impl From<std::io::Error> for Error {
	fn from(e: std::io::Error) -> Self {
		Error::IO(e)
	}
}

/// Two rasters that must line up pixel for pixel but differ in size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("{first} and {second} must be the same size, but {first} is {}×{} pixels and {second} is {}×{}", .first_size.x, .first_size.y, .second_size.x, .second_size.y)]
pub struct SizeMismatchError {
	pub first: &'static str,
	pub first_size: UVec2,
	pub second: &'static str,
	pub second_size: UVec2,
}

#[node_macro::node(category("Raster"), icon("NodeChannelsToImage"), name("Channels to Image"), memoize)]
fn channels_to_image(
	_: impl Ctx,
	_primary: (),
	#[expose] red: Item<Raster<CPU>>,
	#[expose] green: Item<Raster<CPU>>,
	#[expose] blue: Item<Raster<CPU>>,
	#[expose] alpha: Item<Raster<CPU>>,
) -> Result<Item<Raster<CPU>>, SizeMismatchError> {
	// An unconnected channel arrives as the default zero-sized raster, which counts as absent
	let present = |channel: Item<Raster<CPU>>| (channel.element().width > 0 && channel.element().height > 0).then_some(channel);
	let channels = [red, green, blue, alpha].map(present);

	// All present channels must share the same dimensions
	let dimensions = |channel: &Item<Raster<CPU>>| UVec2::new(channel.element().width, channel.element().height);
	let mut present_channels = ["Red", "Green", "Blue", "Alpha"]
		.into_iter()
		.zip(&channels)
		.filter_map(|(name, channel)| Some((name, channel.as_ref()?)));
	let Some((first, first_channel)) = present_channels.next() else {
		return Ok(Item::default());
	};
	let (first_size, transform) = (dimensions(first_channel), first_channel.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM));
	if let Some((second, second_channel)) = present_channels.find(|(_, channel)| dimensions(channel) != first_size) {
		return Err(SizeMismatchError {
			first,
			first_size,
			second,
			second_size: dimensions(second_channel),
		});
	}
	let (width, height) = (first_size.x, first_size.y);

	// A channel's effects grow it by the margin they cast past its edges, so every channel is placed within the widest margin
	let channels = channels.map(|channel| channel.map(evaluate_raster_effects));
	let margin = |channel: &Item<Raster<CPU>>| ((channel.element().width - width) / 2, (channel.element().height - height) / 2);
	let (margin_x, margin_y) = channels
		.iter()
		.flatten()
		.map(margin)
		.fold((0, 0), |(x, y), (channel_x, channel_y)| (x.max(channel_x), y.max(channel_y)));

	// Take this item's blending attributes from the first present channel, whose effects are carried out by now
	let Some(mut attributes) = channels.iter().flatten().next().map(|channel| channel.attributes().clone()) else {
		return Ok(Item::default());
	};
	let size = DVec2::new(width as f64, height as f64);
	let unit_margin = DVec2::new(margin_x as f64, margin_y as f64) / size;
	attributes.insert(
		ATTR_TRANSFORM,
		transform * DAffine2::from_translation(-unit_margin) * DAffine2::from_scale(DVec2::ONE + 2. * unit_margin),
	);

	// A channel's value at a pixel of the output, with nothing past its own edges, or `None` if the channel is absent
	let value = |channel: &Option<Item<Raster<CPU>>>, x: u32, y: u32| {
		channel.as_ref().map(|channel| {
			let (channel_margin_x, channel_margin_y) = margin(channel);
			let (x, y) = ((x + channel_margin_x).checked_sub(margin_x), (y + channel_margin_y).checked_sub(margin_y));
			x.zip(y).and_then(|(x, y)| channel.element().get_pixel(x, y)).map_or(0., |pixel| pixel.l().cast_linear_channel())
		})
	};

	// Set each output pixel's channels from the present inputs, defaulting absent color channels to 0 and absent alpha to 1
	let [red, green, blue, alpha] = &channels;
	let mut image = Image::new(width + 2 * margin_x, height + 2 * margin_y, Color::TRANSPARENT);
	for y in 0..image.height() {
		for x in 0..image.width() {
			let image_pixel = image.get_pixel_mut(x, y).unwrap();

			image_pixel.set_red(value(red, x, y).unwrap_or(0.));
			image_pixel.set_green(value(green, x, y).unwrap_or(0.));
			image_pixel.set_blue(value(blue, x, y).unwrap_or(0.));
			image_pixel.set_alpha(value(alpha, x, y).unwrap_or(1.));
		}
	}

	Ok(Item::from_parts(Raster::new_cpu(image), attributes))
}

#[node_macro::node(category("Raster"), icon("NodeMask"), memoize)]
fn mask(
	_: impl Ctx,
	/// The image to be masked.
	image: Item<Raster<CPU>>,
	/// The stencil to be used for masking.
	#[expose]
	stencil: Item<Raster<CPU>>,
) -> Item<Raster<CPU>> {
	let image = evaluate_raster_effects(image);
	let stencil = evaluate_raster_effects(stencil);

	// An absent stencil arrives as the default empty raster, leaving the image unmasked
	if stencil.element().width == 0 || stencil.element().height == 0 {
		return image;
	}
	let stencil_size = DVec2::new(stencil.element().width as f64, stencil.element().height as f64);

	let mut row = image;
	let image_size = DVec2::new(row.element().width as f64, row.element().height as f64);
	let stencil_transform: DAffine2 = stencil.attribute_cloned_or_default(ATTR_TRANSFORM);

	if stencil_transform.matrix2.determinant() == 0. {
		return row;
	}

	// Transforms a point from the background image to the foreground image
	let transform_attribute: DAffine2 = row.attribute_cloned_or_default(ATTR_TRANSFORM);
	let bg_to_fg = transform_attribute * DAffine2::from_scale(1. / image_size);
	let stencil_transform_inverse = stencil_transform.inverse();

	let width = row.element().width as usize;
	let stencil_image = stencil.element();
	fill_rows(&mut row.element_mut().data_mut().data, width, |y, pixels| {
		for (x, image_pixel) in pixels.iter_mut().enumerate() {
			let image_point = DVec2::new(x as f64, y as f64);
			let mask_point = bg_to_fg.transform_point2(image_point);
			let local_mask_point = stencil_transform_inverse.transform_point2(mask_point);
			let mask_point = stencil_transform.transform_point2(local_mask_point.clamp(DVec2::ZERO, DVec2::ONE));
			let mask_point = (DAffine2::from_scale(stencil_size) * stencil_transform.inverse()).transform_point2(mask_point);

			let mask_pixel = stencil_image.sample(mask_point);
			*image_pixel = image_pixel.multiplied_alpha(mask_pixel.l().cast_linear_channel());
		}
	});

	row
}

#[node_macro::node(category(""))]
fn extend_image_to_bounds(_: impl Ctx, image: Item<Raster<CPU>>, bounds: Item<DAffine2>) -> Result<Item<Raster<CPU>>, (Item<Raster<CPU>>, ImageError)> {
	let image = evaluate_raster_effects(image);
	let bounds = *bounds.element();

	let image_transform: DAffine2 = image.attribute_cloned_or_default(ATTR_TRANSFORM);
	let image_aabb = Bbox::unit().affine_transform(image_transform).to_axis_aligned_bbox();
	let bounds_aabb = Bbox::unit().affine_transform(bounds.transform()).to_axis_aligned_bbox();
	if image_aabb.contains(bounds_aabb.start) && image_aabb.contains(bounds_aabb.end) {
		return Ok(image);
	}

	let (image_width, image_height) = (image.element().width, image.element().height);
	if image_width == 0 || image_height == 0 {
		return empty_image(bounds, Color::TRANSPARENT).map_err(|error| (image, error));
	}
	if image_transform.matrix2.determinant() == 0. {
		return Ok(image);
	}

	let orig_image_scale = DVec2::new(image_width as f64, image_height as f64);
	let layer_to_image_space = DAffine2::from_scale(orig_image_scale) * image_transform.inverse();
	let bounds_in_image_space = Bbox::unit().affine_transform(layer_to_image_space * bounds).to_axis_aligned_bbox();

	let new_start = bounds_in_image_space.start.floor().min(DVec2::ZERO);
	let new_end = bounds_in_image_space.end.ceil().max(orig_image_scale);
	let new_scale = new_end - new_start;

	// Copy over original image into enlarged image.
	let mut new_image = match Image::try_new(new_scale.x as u32, new_scale.y as u32, Color::TRANSPARENT) {
		Ok(new_image) => new_image,
		Err(error) => return Err((image, error)),
	};
	let (image, mut attributes) = image.into_parts();
	let offset_in_new_image = (-new_start).as_uvec2();
	for y in 0..image_height {
		let old_start = y * image_width;
		let new_start = (y + offset_in_new_image.y) * new_image.width + offset_in_new_image.x;
		let old_row = &image.data[old_start as usize..(old_start + image_width) as usize];
		let new_row = &mut new_image.data[new_start as usize..(new_start + image_width) as usize];
		new_row.copy_from_slice(old_row);
	}

	// Compute new transform.
	// let layer_to_new_texture_space = (DAffine2::from_scale(1. / new_scale) * DAffine2::from_translation(new_start) * layer_to_image_space).inverse();
	let new_texture_to_layer_space = image_transform * DAffine2::from_scale(1. / orig_image_scale) * DAffine2::from_translation(new_start) * DAffine2::from_scale(new_scale);

	attributes.insert(ATTR_TRANSFORM, new_texture_to_layer_space);
	Ok(Item::from_parts(Raster::new_cpu(new_image), attributes))
}

/// Creates an image filled with one color, sized in pixels by the transform's scale and placed by the transform.
fn empty_image(transform: DAffine2, color: Color) -> Result<Item<Raster<CPU>>, ImageError> {
	let width = transform.transform_vector2(DVec2::new(1., 0.)).length() as u32;
	let height = transform.transform_vector2(DVec2::new(0., 1.)).length() as u32;

	let image = Image::try_new(width, height, color)?;

	Ok(Item::new_from_element(Raster::new_cpu(image)).with_attribute(ATTR_TRANSFORM, transform))
}

/// Displays an image from a file.
///
/// Reads PNG, JPG, GIF, WEBP, TIFF, BMP, TGA, ICO, HDR, and EXR files. Light brighter than white, as HDR and EXR files can hold, is clipped to white.
#[node_macro::node(category("Raster"), icon("NodeImage"), memoize)]
fn image(
	_: impl Ctx,
	_primary: (),
	/// The image file to display.
	#[widget(ParsedWidgetOverride::Custom = "image_file")]
	resource: Item<Resource>,
) -> Result<Item<Raster<CPU>>, ImageError> {
	let resource = resource.into_element();
	if resource.is_empty() {
		return Ok(Item::default());
	}
	let image = Image::from_encoded(resource.as_ref())?;

	Ok(Item::new_from_element(Raster::new_cpu(image)))
}

/// Generates customizable procedural noise patterns.
#[node_macro::node(category("Raster"), icon("NodeNoisePattern"))]
#[allow(clippy::too_many_arguments)]
fn noise_pattern(
	_: impl Ctx,
	_primary: (),
	#[hard(0..)] seed: Item<i64>,
	#[widget(ParsedWidgetOverride::Custom = "noise_inspector_scale")]
	#[default(10.)]
	scale: Item<f64>,
	#[widget(ParsedWidgetOverride::Custom = "noise_inspector_noise_type")] noise_type: Item<NoiseType>,
	#[widget(ParsedWidgetOverride::Custom = "noise_inspector_domain_warp_type")] domain_warp_type: Item<DomainWarpType>,
	#[widget(ParsedWidgetOverride::Custom = "noise_inspector_domain_warp_amplitude")]
	#[default(100.)]
	domain_warp_amplitude: Item<f64>,
	#[widget(ParsedWidgetOverride::Custom = "noise_inspector_fractal_type")] fractal_type: Item<FractalType>,
	#[widget(ParsedWidgetOverride::Custom = "noise_inspector_fractal_octaves")]
	#[default(3)]
	#[hard(1..10)]
	fractal_octaves: Item<i64>,
	#[widget(ParsedWidgetOverride::Custom = "noise_inspector_fractal_lacunarity")]
	#[default(2.)]
	fractal_lacunarity: Item<f64>,
	#[widget(ParsedWidgetOverride::Custom = "noise_inspector_fractal_gain")]
	#[default(0.5)]
	fractal_gain: Item<f64>,
	#[widget(ParsedWidgetOverride::Custom = "noise_inspector_fractal_weighted_strength")] fractal_weighted_strength: Item<f64>,
	#[widget(ParsedWidgetOverride::Custom = "noise_inspector_ping_pong_strength")]
	#[default(2.)]
	fractal_ping_pong_strength: Item<f64>,
	#[widget(ParsedWidgetOverride::Custom = "noise_inspector_cellular_distance_function")] cellular_distance_function: Item<CellularDistanceFunction>,
	#[widget(ParsedWidgetOverride::Custom = "noise_inspector_cellular_return_type")] cellular_return_type: Item<CellularReturnType>,
	#[widget(ParsedWidgetOverride::Custom = "noise_inspector_cellular_jitter")]
	#[default(1.)]
	cellular_jitter: Item<f64>,
) -> Item<Generator> {
	let settings = NoiseSettings {
		seed: seed.into_element(),
		scale: scale.into_element(),
		noise_type: noise_type.into_element(),
		domain_warp_type: domain_warp_type.into_element(),
		domain_warp_amplitude: domain_warp_amplitude.into_element(),
		fractal_type: fractal_type.into_element(),
		fractal_octaves: fractal_octaves.into_element(),
		fractal_lacunarity: fractal_lacunarity.into_element(),
		fractal_gain: fractal_gain.into_element(),
		fractal_weighted_strength: fractal_weighted_strength.into_element(),
		fractal_ping_pong_strength: fractal_ping_pong_strength.into_element(),
		cellular_distance_function: cellular_distance_function.into_element(),
		cellular_return_type: cellular_return_type.into_element(),
		cellular_jitter: cellular_jitter.into_element(),
	};

	let mut generator = Item::new_from_element(Generator::Noise);
	settings.stamp(&mut generator);
	generator
}

#[node_macro::node(category("Raster"), icon("NodeMandelbrot"))]
fn mandelbrot(_: impl Ctx, _primary: ()) -> Item<Generator> {
	Item::new_from_element(Generator::Mandelbrot)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn channels_stay_aligned_when_effects_grow_only_one() {
		use graphic_types::{Effect, EffectOperation, stamp_effect};

		let gray = |value: f32| {
			let image = Image::new(8, 8, Color::from_rgbaf32_unchecked(value, value, value, 1.));
			Item::new_from_element(Raster::new_cpu(image)).with_attribute(ATTR_TRANSFORM, DAffine2::from_scale(DVec2::splat(8.)))
		};

		// Only the red channel is blurred, which grows it by the blur's margin
		let mut red = gray(1.);
		stamp_effect(
			&mut red,
			Effect::new(&EffectOperation::Blur {
				radius: 1.,
				box_blur: false,
				gamma: false,
			}),
		);
		let image = channels_to_image((), (), red, gray(0.5), gray(0.25), gray(1.)).unwrap();

		// The other channels are placed within the same margin, and the whole image keeps its place in the document
		let raster = image.element();
		assert!(raster.width > 8 && raster.height > 8, "the image grows to hold the blur, rather than coming out empty");
		let transform: DAffine2 = image.attribute_cloned_or_default(ATTR_TRANSFORM);
		let to_pixels = DAffine2::from_scale(DVec2::new(raster.width as f64, raster.height as f64)) * transform.inverse();
		let center = to_pixels.transform_point2(DVec2::splat(4.)).as_uvec2();
		let pixel = raster.get_pixel(center.x, center.y).unwrap();
		assert!((pixel.g() - 0.5).abs() < 1e-6 && (pixel.b() - 0.25).abs() < 1e-6, "the unblurred channels line up: {pixel:?}");
	}
}
