use crate::adjustments::{CellularDistanceFunction, CellularReturnType, DomainWarpType, FractalType, NoiseType};
use crate::gpu_params;
use crate::parallel_rows::fill_rows;
use crate::raster_kernel::{KernelFrame, RasterKernel, Wgsl, raster_kernel_pipeline, raster_placement, run_raster_kernel, union_frame};
use core_types::ATTR_TRANSFORM;
use core_types::color::Color;
use core_types::color::{Alpha, AlphaMut, Channel, LinearChannel, Luminance, RGBMut};
use core_types::context::{Ctx, ExtractFootprint};
use core_types::list::Item;
use core_types::math::bbox::Bbox;
use core_types::transform::Transform;
use core_types::{Interrupt, NodeError};
use dyn_any::DynAny;
use fastnoise_lite;
use glam::{DAffine2, DVec2, UVec2, Vec2};
use graphene_resource::Resource;
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use raster_types::{Bitmap, BitmapMut};
use raster_types::{CPU, GPU, Raster};
use raster_types::{Image, ImageError};
use std::fmt::Debug;
use std::hash::Hash;
use wgpu_executor::WgpuPipelineCache;

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
fn channels_to_image<T: PlacedRaster>(
	_: impl Ctx,
	_primary: (),
	#[expose]
	#[implementations(Raster<CPU>, Raster<GPU>)]
	red: Item<T>,
	#[expose]
	#[implementations(Raster<CPU>, Raster<GPU>)]
	green: Item<T>,
	#[expose]
	#[implementations(Raster<CPU>, Raster<GPU>)]
	blue: Item<T>,
	#[expose]
	#[implementations(Raster<CPU>, Raster<GPU>)]
	alpha: Item<T>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	Ok(T::channels_to_image([red, green, blue, alpha], pipeline.element())?)
}

fn channels_to_image_cpu(red: Item<Raster<CPU>>, green: Item<Raster<CPU>>, blue: Item<Raster<CPU>>, alpha: Item<Raster<CPU>>) -> Result<Item<Raster<CPU>>, SizeMismatchError> {
	// An unconnected channel arrives as the default zero-sized raster, which counts as absent
	let present = |channel: Item<Raster<CPU>>| (channel.element().width > 0 && channel.element().height > 0).then_some(channel);
	let (red, green, blue, alpha) = (present(red), present(green), present(blue), present(alpha));

	// Take this item's transform and blending attributes from the first present channel
	let Some(attributes) = [&red, &green, &blue, &alpha].iter().find_map(|channel| channel.as_ref()).map(|channel| channel.attributes().clone()) else {
		return Ok(Item::default());
	};

	// All present channels must share the same dimensions
	let channel_dimensions = [("Red", &red), ("Green", &green), ("Blue", &blue), ("Alpha", &alpha)]
		.map(|(name, channel)| channel.as_ref().map(|channel| (name, UVec2::new(channel.element().width, channel.element().height))));
	let Some(&(first, first_size)) = channel_dimensions.iter().flatten().next() else {
		return Ok(Item::default());
	};
	if let Some(&(second, second_size)) = channel_dimensions.iter().flatten().find(|&&(_, size)| size != first_size) {
		return Err(SizeMismatchError {
			first,
			first_size,
			second,
			second_size,
		});
	}
	let (width, height) = (first_size.x, first_size.y);

	// Set each output pixel's channels from the present inputs, defaulting absent color channels to 0 and absent alpha to 1
	let mut image = Image::new(width, height, Color::TRANSPARENT);
	for y in 0..image.height() {
		for x in 0..image.width() {
			let image_pixel = image.get_pixel_mut(x, y).unwrap();

			match red.as_ref().and_then(|r| r.element().get_pixel(x, y)) {
				Some(r) => image_pixel.set_red(r.l().cast_linear_channel()),
				None => image_pixel.set_red(Channel::from_linear(0.)),
			}
			match green.as_ref().and_then(|g| g.element().get_pixel(x, y)) {
				Some(g) => image_pixel.set_green(g.l().cast_linear_channel()),
				None => image_pixel.set_green(Channel::from_linear(0.)),
			}
			match blue.as_ref().and_then(|b| b.element().get_pixel(x, y)) {
				Some(b) => image_pixel.set_blue(b.l().cast_linear_channel()),
				None => image_pixel.set_blue(Channel::from_linear(0.)),
			}
			match alpha.as_ref().and_then(|a| a.element().get_pixel(x, y)) {
				Some(a) => image_pixel.set_alpha(a.l().cast_linear_channel()),
				None => image_pixel.set_alpha(Channel::from_linear(1.)),
			}
		}
	}

	Ok(Item::from_parts(Raster::new_cpu(image), attributes))
}

#[node_macro::node(category("Raster"), icon("NodeMask"), memoize)]
fn mask<T: PlacedRaster>(
	_: impl Ctx,
	/// The image to be masked.
	#[implementations(Raster<CPU>, Raster<GPU>)]
	image: Item<T>,
	/// The stencil to be used for masking.
	#[expose]
	#[implementations(Raster<CPU>, Raster<GPU>)]
	stencil: Item<T>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	Ok(T::mask(image, stencil, pipeline.element())?)
}

fn mask_cpu(image: Item<Raster<CPU>>, stencil: Item<Raster<CPU>>) -> Item<Raster<CPU>> {
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
fn extend_image_to_bounds<T: PlacedRaster>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Raster<GPU>)] image: Item<T>,
	bounds: Item<DAffine2>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, (Item<T>, NodeError)> {
	T::extend_image_to_bounds(image, *bounds.element(), pipeline.element())
}

fn extend_image_to_bounds_cpu(image: Item<Raster<CPU>>, bounds: DAffine2) -> Result<Item<Raster<CPU>>, (Item<Raster<CPU>>, ImageError)> {
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

/// A raster the Channels to Image, Mask, and Extend Image to Bounds nodes process, on the CPU or the GPU.
pub trait PlacedRaster: Sized {
	fn channels_to_image(channels: [Item<Self>; 4], pipeline: &WgpuPipelineCache) -> Result<Item<Self>, NodeError>;
	fn mask(image: Item<Self>, stencil: Item<Self>, pipeline: &WgpuPipelineCache) -> Result<Item<Self>, NodeError>;
	fn extend_image_to_bounds(image: Item<Self>, bounds: DAffine2, pipeline: &WgpuPipelineCache) -> Result<Item<Self>, (Item<Self>, NodeError)>;
}

impl PlacedRaster for Raster<CPU> {
	fn channels_to_image([red, green, blue, alpha]: [Item<Self>; 4], _: &WgpuPipelineCache) -> Result<Item<Self>, NodeError> {
		channels_to_image_cpu(red, green, blue, alpha).map_err(NodeError::new)
	}

	fn mask(image: Item<Self>, stencil: Item<Self>, _: &WgpuPipelineCache) -> Result<Item<Self>, NodeError> {
		Ok(mask_cpu(image, stencil))
	}

	fn extend_image_to_bounds(image: Item<Self>, bounds: DAffine2, _: &WgpuPipelineCache) -> Result<Item<Self>, (Item<Self>, NodeError)> {
		extend_image_to_bounds_cpu(image, bounds).map_err(|(image, error)| (image, NodeError::new(error)))
	}
}

impl PlacedRaster for Raster<GPU> {
	/// Each channel is placed by its transform into their [`union_frame`], which takes the red channel's other attributes, and gives its luminance weighted by its alpha.
	fn channels_to_image(channels: [Item<Self>; 4], pipeline: &WgpuPipelineCache) -> Result<Item<Self>, NodeError> {
		let frame = union_frame(&channels.each_ref().map(raster_placement)).ok_or_else(|| NodeError::new("Every channel must be placed by an invertible transform"))?;
		let raster = run_raster_kernel(&CombineChannels {}, frame.kernel_inputs(&channels.each_ref().map(Item::element)), frame.size, pipeline)?;

		let [red, ..] = channels;
		let (_, mut attributes) = red.into_parts();
		attributes.insert(ATTR_TRANSFORM, frame.transform);
		Ok(Item::from_parts(raster, attributes))
	}

	/// The stencil is sampled bilinearly where each image pixel's center lies, extending its edge pixels beyond it.
	fn mask(image: Item<Self>, stencil: Item<Self>, pipeline: &WgpuPipelineCache) -> Result<Item<Self>, NodeError> {
		let (image_size, image_transform) = raster_placement(&image);
		let (stencil_size, stencil_transform) = raster_placement(&stencil);
		if image_transform.matrix2.determinant() == 0. || stencil_transform.matrix2.determinant() == 0. {
			return Ok(image);
		}

		let frame = KernelFrame::new(image_size, image_transform, &[(image_size, image_transform), (stencil_size, stencil_transform)]);
		let to_stencil = frame.output_to_texel[1];
		let kernel = MaskStencil {
			stencil_axes: to_stencil.matrix2.to_cols_array().map(|value| value as f32),
			stencil_translation: to_stencil.translation.to_array().map(|value| value as f32),
		};
		let raster = run_raster_kernel(&kernel, frame.kernel_inputs(&[image.element(), stencil.element()]), frame.size, pipeline)?;

		let (_, attributes) = image.into_parts();
		Ok(Item::from_parts(raster, attributes))
	}

	fn extend_image_to_bounds(image: Item<Self>, bounds: DAffine2, pipeline: &WgpuPipelineCache) -> Result<Item<Self>, (Item<Self>, NodeError)> {
		let (image_size, image_transform) = raster_placement(&image);
		let image_aabb = Bbox::unit().affine_transform(image_transform).to_axis_aligned_bbox();
		let bounds_aabb = Bbox::unit().affine_transform(bounds.transform()).to_axis_aligned_bbox();
		if image_transform.matrix2.determinant() == 0. || (image_aabb.contains(bounds_aabb.start) && image_aabb.contains(bounds_aabb.end)) {
			return Ok(image);
		}

		let orig_image_scale = image_size.as_dvec2();
		let layer_to_image_space = DAffine2::from_scale(orig_image_scale) * image_transform.inverse();
		let bounds_in_image_space = Bbox::unit().affine_transform(layer_to_image_space * bounds).to_axis_aligned_bbox();
		let new_start = bounds_in_image_space.start.floor().min(DVec2::ZERO);
		let new_scale = bounds_in_image_space.end.ceil().max(orig_image_scale) - new_start;
		let transform = image_transform * DAffine2::from_scale(1. / orig_image_scale) * DAffine2::from_translation(new_start) * DAffine2::from_scale(new_scale);

		let frame = KernelFrame::new(new_scale.as_uvec2(), transform, &[(image_size, image_transform)]);
		let offset = new_start.as_ivec2();
		let kernel = ExtendToBounds {
			offset_x: offset.x,
			offset_y: offset.y,
		};
		let raster = match run_raster_kernel(&kernel, frame.kernel_inputs(&[image.element()]), frame.size, pipeline) {
			Ok(raster) => raster,
			Err(error) => return Err((image, error)),
		};

		let (_, mut attributes) = image.into_parts();
		attributes.insert(ATTR_TRANSFORM, transform);
		Ok(Item::from_parts(raster, attributes))
	}
}

gpu_params! {
	#[derive(Clone)]
	struct CombineChannels {}
}

impl RasterKernel for CombineChannels {
	const INPUTS: usize = 4;
	fn wgsl() -> Wgsl {
		Wgsl::body(
			r#"
			let red = input_0_at(coord);
			let green = input_1_at(coord);
			let blue = input_2_at(coord);
			let alpha = input_3_at(coord);
			return vec4<f32>(luminance_rec_709(red) * red.a, luminance_rec_709(green) * green.a, luminance_rec_709(blue) * blue.a, luminance_rec_709(alpha) * alpha.a);
			"#,
		)
	}
}

gpu_params! {
	#[derive(Clone)]
	struct MaskStencil {
		stencil_axes: [f32; 4],
		stencil_translation: [f32; 2],
	}
}

impl RasterKernel for MaskStencil {
	const INPUTS: usize = 2;
	fn wgsl() -> Wgsl {
		Wgsl::function(
			r#"
fn stencil_luminance(texel: vec2<i32>) -> f32 {
	return luminance_rec_709(input_1_load(texel));
}

fn kernel(coord: vec2<i32>) -> vec4<f32> {
	let parameters = params();
	let axes = parameters.stencil_axes;
	let translation = parameters.stencil_translation;
	let center = vec2<f32>(coord) + 0.5;
	let texel = vec2<f32>(axes[0], axes[1]) * center.x + vec2<f32>(axes[2], axes[3]) * center.y + vec2<f32>(translation[0], translation[1]) - 0.5;
	let base = floor(texel);
	let t = texel - base;
	let corner = vec2<i32>(base);
	let top = mix(stencil_luminance(corner), stencil_luminance(corner + vec2<i32>(1, 0)), t.x);
	let bottom = mix(stencil_luminance(corner + vec2<i32>(0, 1)), stencil_luminance(corner + vec2<i32>(1, 1)), t.x);
	let color = input_0_load(coord);
	return vec4<f32>(color.rgb, color.a * mix(top, bottom, t.y));
}
"#,
		)
	}
}

gpu_params! {
	#[derive(Clone)]
	struct ExtendToBounds {
		offset_x: i32,
		offset_y: i32,
	}
}

impl RasterKernel for ExtendToBounds {
	const INPUTS: usize = 1;
	fn wgsl() -> Wgsl {
		Wgsl::body(
			r#"
			let texel = coord + vec2<i32>(offset_x, offset_y);
			if any(texel < vec2<i32>(0)) || any(texel >= input_0_size()) {
				return vec4<f32>(0.);
			}
			return input_0_load(texel);
			"#,
		)
	}
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
	ctx: impl ExtractFootprint + Ctx,
	_primary: (),
	#[default(true)] clip: Item<bool>,
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
) -> Result<Item<Raster<CPU>>, ImageError> {
	let (clip, seed, scale, domain_warp_amplitude) = (*clip.element(), *seed.element(), *scale.element(), *domain_warp_amplitude.element());
	let (fractal_octaves, fractal_lacunarity, fractal_gain) = (*fractal_octaves.element(), *fractal_lacunarity.element(), *fractal_gain.element());
	let (fractal_weighted_strength, fractal_ping_pong_strength, cellular_jitter) = (*fractal_weighted_strength.element(), *fractal_ping_pong_strength.element(), *cellular_jitter.element());
	let (noise_type, domain_warp_type, fractal_type) = (noise_type.into_element(), domain_warp_type.into_element(), fractal_type.into_element());
	let (cellular_distance_function, cellular_return_type) = (cellular_distance_function.into_element(), cellular_return_type.into_element());

	let footprint = ctx.footprint();
	let viewport_bounds = footprint.viewport_bounds_in_local_space();

	let mut size = viewport_bounds.size();
	let mut offset = viewport_bounds.start;
	if clip {
		// TODO: Remove "clip" entirely (and its arbitrary 100x100 clipping square) once we have proper resolution-aware layer clipping
		const CLIPPING_SQUARE_SIZE: f64 = 100.;
		let image_bounds = Bbox::from_transform(DAffine2::from_scale(DVec2::splat(CLIPPING_SQUARE_SIZE))).to_axis_aligned_bbox();
		let intersection = viewport_bounds.intersect(&image_bounds);

		offset = (intersection.start - image_bounds.start).max(DVec2::ZERO);
		size = intersection.size();
	}

	// If the image would not be visible, return an empty image
	if size.x <= 0. || size.y <= 0. {
		return Ok(Item::default());
	}

	let transform = DAffine2::from_translation(offset) * DAffine2::from_scale(size);

	let footprint_scale = footprint.scale();
	let width = (size.x * footprint_scale.x) as u32;
	let height = (size.y * footprint_scale.y) as u32;

	// All
	let mut image = Image::try_new(width, height, Color::from_luminance(0.5))?;
	let mut noise = fastnoise_lite::FastNoiseLite::with_seed(seed as i32);
	noise.set_frequency(Some(1. / (scale as f32).max(f32::EPSILON)));

	// Domain Warp
	let domain_warp_type = match domain_warp_type {
		DomainWarpType::None => None,
		DomainWarpType::OpenSimplex2 => Some(fastnoise_lite::DomainWarpType::OpenSimplex2),
		DomainWarpType::OpenSimplex2Reduced => Some(fastnoise_lite::DomainWarpType::OpenSimplex2Reduced),
		DomainWarpType::BasicGrid => Some(fastnoise_lite::DomainWarpType::BasicGrid),
	};
	let domain_warp_active = domain_warp_type.is_some();
	noise.set_domain_warp_type(domain_warp_type);
	noise.set_domain_warp_amp(Some(domain_warp_amplitude as f32));

	// Fractal
	let noise_type = match noise_type {
		NoiseType::Perlin => fastnoise_lite::NoiseType::Perlin,
		NoiseType::OpenSimplex2 => fastnoise_lite::NoiseType::OpenSimplex2,
		NoiseType::OpenSimplex2S => fastnoise_lite::NoiseType::OpenSimplex2S,
		NoiseType::Cellular => fastnoise_lite::NoiseType::Cellular,
		NoiseType::ValueCubic => fastnoise_lite::NoiseType::ValueCubic,
		NoiseType::Value => fastnoise_lite::NoiseType::Value,
		NoiseType::WhiteNoise => {
			// TODO: Generate in layer space, not viewport space

			let mut rng = ChaCha8Rng::seed_from_u64(seed as u64);

			for y in 0..height {
				for x in 0..width {
					let pixel = image.get_pixel_mut(x, y).unwrap();
					let luminance = rng.random_range(0.0..1.) as f32;
					*pixel = Color::from_luminance(luminance);
				}
			}

			return Ok(Item::new_from_element(Raster::new_cpu(image)).with_attribute(ATTR_TRANSFORM, transform));
		}
	};
	noise.set_noise_type(Some(noise_type));
	let fractal_type = match fractal_type {
		FractalType::None => fastnoise_lite::FractalType::None,
		FractalType::FBm => fastnoise_lite::FractalType::FBm,
		FractalType::Ridged => fastnoise_lite::FractalType::Ridged,
		FractalType::PingPong => fastnoise_lite::FractalType::PingPong,
		FractalType::DomainWarpProgressive => fastnoise_lite::FractalType::DomainWarpProgressive,
		FractalType::DomainWarpIndependent => fastnoise_lite::FractalType::DomainWarpIndependent,
	};
	noise.set_fractal_type(Some(fractal_type));
	noise.set_fractal_octaves(Some(fractal_octaves as i32));
	noise.set_fractal_lacunarity(Some(fractal_lacunarity as f32));
	noise.set_fractal_gain(Some(fractal_gain as f32));
	noise.set_fractal_weighted_strength(Some(fractal_weighted_strength as f32));
	noise.set_fractal_ping_pong_strength(Some(fractal_ping_pong_strength as f32));

	// Cellular
	let cellular_distance_function = match cellular_distance_function {
		CellularDistanceFunction::Euclidean => fastnoise_lite::CellularDistanceFunction::Euclidean,
		CellularDistanceFunction::EuclideanSq => fastnoise_lite::CellularDistanceFunction::EuclideanSq,
		CellularDistanceFunction::Manhattan => fastnoise_lite::CellularDistanceFunction::Manhattan,
		CellularDistanceFunction::Hybrid => fastnoise_lite::CellularDistanceFunction::Hybrid,
	};
	let cellular_return_type = match cellular_return_type {
		CellularReturnType::CellValue => fastnoise_lite::CellularReturnType::CellValue,
		CellularReturnType::Nearest => fastnoise_lite::CellularReturnType::Distance,
		CellularReturnType::NextNearest => fastnoise_lite::CellularReturnType::Distance2,
		CellularReturnType::Average => fastnoise_lite::CellularReturnType::Distance2Add,
		CellularReturnType::Difference => fastnoise_lite::CellularReturnType::Distance2Sub,
		CellularReturnType::Product => fastnoise_lite::CellularReturnType::Distance2Mul,
		CellularReturnType::Division => fastnoise_lite::CellularReturnType::Distance2Div,
	};
	noise.set_cellular_distance_function(Some(cellular_distance_function));
	noise.set_cellular_return_type(Some(cellular_return_type));
	noise.set_cellular_jitter(Some(cellular_jitter as f32));

	let coordinate_offset = offset.as_vec2();
	let scale = size.as_vec2() / Vec2::new(width as f32, height as f32);
	// Calculate the noise for every pixel
	for y in 0..height {
		for x in 0..width {
			let pixel = image.get_pixel_mut(x, y).unwrap();
			let pos = Vec2::new(x as f32, y as f32);
			let vec = pos * scale + coordinate_offset;

			let (mut x, mut y) = (vec.x, vec.y);
			if domain_warp_active && domain_warp_amplitude > 0. {
				(x, y) = noise.domain_warp_2d(x, y);
			}

			let luminance = (noise.get_noise_2d(x, y) + 1.) * 0.5;
			*pixel = Color::from_luminance(luminance);
		}
	}

	Ok(Item::new_from_element(Raster::new_cpu(image)).with_attribute(ATTR_TRANSFORM, transform))
}

#[node_macro::node(category("Raster"), icon("NodeMandelbrot"))]
fn mandelbrot(ctx: impl ExtractFootprint + Send) -> Result<Item<Raster<CPU>>, ImageError> {
	let footprint = ctx.footprint();
	let viewport_bounds = footprint.viewport_bounds_in_local_space();

	let image_bounds = Bbox::from_transform(DAffine2::IDENTITY).to_axis_aligned_bbox();
	let intersection = viewport_bounds.intersect(&image_bounds);
	let size = intersection.size();

	let offset = (intersection.start - image_bounds.start).max(DVec2::ZERO);

	// If the image would not be visible, return an empty image
	if size.x <= 0. || size.y <= 0. {
		return Ok(Item::default());
	}

	let scale = footprint.scale();
	let width = (size.x * scale.x) as u32;
	let height = (size.y * scale.y) as u32;

	let mut image = Image::try_new(width, height, Color::TRANSPARENT)?;
	let max_iter = 255;

	let scale = 3. * size.as_vec2() / Vec2::new(width as f32, height as f32);
	let coordinate_offset = offset.as_vec2() * 3. - Vec2::new(2., 1.5);
	for y in 0..height {
		for x in 0..width {
			let pos = Vec2::new(x as f32, y as f32);
			let c = pos * scale + coordinate_offset;

			let iter = mandelbrot_impl(c, max_iter);
			*image.get_mut(x as usize, y as usize) = map_color(iter, max_iter);
		}
	}

	Ok(Item::new_from_element(Raster::new_cpu(image)).with_attribute(ATTR_TRANSFORM, DAffine2::from_translation(offset) * DAffine2::from_scale(size)))
}

#[inline(always)]
fn mandelbrot_impl(c: Vec2, max_iter: usize) -> usize {
	let mut z = Vec2::new(0., 0.);
	for i in 0..max_iter {
		z = Vec2::new(z.x * z.x - z.y * z.y, 2. * z.x * z.y) + c;
		if z.length_squared() > 4. {
			return i;
		}
	}
	max_iter
}

fn map_color(iter: usize, max_iter: usize) -> Color {
	let v = iter as f32 / max_iter as f32;
	Color::from_rgbaf32_unchecked(v, v, v, 1.)
}
