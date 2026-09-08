use crate::adjustments::{CellularDistanceFunction, CellularReturnType, DomainWarpType, FractalType, NoiseType};
use core_types::attribute::{Attr, Attribute, BlendMode as BlendModeAttr, ClippingMask, EditorLayerPath, Opacity, OpacityFill, Transform as TransformAttr};
use core_types::color::Color;
use core_types::color::{Alpha, AlphaMut, Channel, LinearChannel, Luminance, RGBMut};
use core_types::context::{Ctx, ExtractFootprint, ExtractIndex, InjectIndex};
use core_types::extent::{LevelIn, ListIn, ValueIn};
use core_types::gpoll::{Extent, GPoll, GraphError, Interrupt};
use core_types::math::bbox::Bbox;
use core_types::transform::Transform;
use dyn_any::DynAny;
use fastnoise_lite;
use glam::{DAffine2, DVec2, Vec2};
use graphene_resource::Resource;
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use raster_types::Image;
use raster_types::{Bitmap, BitmapMut};
use raster_types::{CPU, Raster};
use std::fmt::Debug;

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

#[node_macro::node(category("Debug"))]
pub fn sample_image(ctx: impl Ctx + ExtractFootprint, (image, lane_transform): (Raster<CPU>, Attr<TransformAttr>)) -> (Raster<CPU>, Attr<TransformAttr>) {
	let image_frame_transform: DAffine2 = *lane_transform;

	// Resize the image using the image crate
	let data = bytemuck::cast_vec(image.data.clone());

	let footprint = ctx.footprint();
	let viewport_bounds = footprint.viewport_bounds_in_local_space();
	let image_bounds = Bbox::from_transform(image_frame_transform).to_axis_aligned_bbox();
	let intersection = viewport_bounds.intersect(&image_bounds);
	let image_size = DAffine2::from_scale(DVec2::new(image.width as f64, image.height as f64));
	let size = intersection.size();
	let size_px = image_size.transform_vector2(size).as_uvec2();

	// A culled lane serves a zero-size raster, which renders as nothing.
	if size.x <= 0. || size.y <= 0. {
		return (Raster::new_cpu(Image::default()), Attr(image_frame_transform));
	}

	let image_buffer = ::image::Rgba32FImage::from_raw(image.width, image.height, data).expect("Failed to convert internal image format into image-rs data type.");

	let dynamic_image: ::image::DynamicImage = image_buffer.into();
	let offset = (intersection.start - image_bounds.start).max(DVec2::ZERO);
	let offset_px = image_size.transform_vector2(offset).as_uvec2();
	let cropped = dynamic_image.crop_imm(offset_px.x, offset_px.y, size_px.x, size_px.y);

	let viewport_resolution_x = footprint.transform.transform_vector2(DVec2::X * size.x).length();
	let viewport_resolution_y = footprint.transform.transform_vector2(DVec2::Y * size.y).length();
	let mut new_width = size_px.x;
	let mut new_height = size_px.y;

	// Only downscale the image for now
	let resized = if new_width < image.width || new_height < image.height {
		new_width = viewport_resolution_x as u32;
		new_height = viewport_resolution_y as u32;
		// TODO: choose filter based on quality requirements
		cropped.resize_exact(new_width, new_height, ::image::imageops::Triangle)
	} else {
		cropped
	};
	let buffer = resized.to_rgba32f();
	let buffer = buffer.into_raw();
	let vec = bytemuck::cast_vec(buffer);
	let image = Image {
		width: new_width,
		height: new_height,
		data: vec,
		base64_string: None,
	};
	// we need to adjust the offset if we truncate the offset calculation

	let new_transform = image_frame_transform * DAffine2::from_translation(offset) * DAffine2::from_scale(size);

	(Raster::new_cpu(image), Attr(new_transform))
}

#[node_macro::node(category("Raster: Channels"), extent(combine_channels_extent))]
pub fn combine_channels<'e>(
	ctx: impl Ctx + ExtractArena<'e> + ExtractIndex + InjectIndex + Copy,
	_primary: (),
	#[expose] red: IList<Raster<CPU>>,
	#[expose] green: IList<Raster<CPU>>,
	#[expose] blue: IList<Raster<CPU>>,
	#[expose] alpha: IList<Raster<CPU>>,
) -> Result<
	IList<(
		Raster<CPU>,
		Attr<'e, TransformAttr>,
		Attr<'e, BlendModeAttr>,
		Attr<'e, Opacity>,
		Attr<'e, OpacityFill>,
		Attr<'e, ClippingMask>,
		Attr<'e, EditorLayerPath>,
	)>,
	Interrupt,
> {
	let lane = ctx.index() as usize;
	let max_len = red.len().max(green.len()).max(blue.len()).max(alpha.len());
	if lane >= max_len {
		return Err(GraphError::past_end().into());
	}

	// Zero-size lanes and lanes past a shorter channel's end contribute nothing
	fn pick<'l>(list: &'l core_types::node::List<'_, Raster<CPU>>, lane: usize) -> Option<&'l Raster<CPU>> {
		(lane < list.len()).then(|| list.element_ref(lane)).filter(|i| i.width > 0 && i.height > 0)
	}
	let (red_el, green_el, blue_el, alpha_el) = (pick(&red, lane), pick(&green, lane), pick(&blue, lane), pick(&alpha, lane));

	// This lane's transform and blending come from the first non-empty channel
	let attr_source = [(red_el.is_some(), &red), (green_el.is_some(), &green), (blue_el.is_some(), &blue), (alpha_el.is_some(), &alpha)]
		.into_iter()
		.find_map(|(present, list)| present.then_some(list.lane(lane)));

	// The channels must have equal dimensions; an unusable lane serves a
	// zero-size raster, which renders as nothing (the legacy form dropped it)
	let channel_dimensions = [
		red_el.map(|r| (r.width, r.height)),
		green_el.map(|g| (g.width, g.height)),
		blue_el.map(|b| (b.width, b.height)),
		alpha_el.map(|a| (a.width, a.height)),
	];
	let mismatched = channel_dimensions
		.iter()
		.flatten()
		.any(|&(x, y)| channel_dimensions.iter().flatten().any(|&(other_x, other_y)| x != other_x || y != other_y));
	let (Some(source), Some(&(width, height)), false) = (attr_source, channel_dimensions.iter().flatten().next(), mismatched) else {
		return Ok((
			Raster::new_cpu(Image::default()),
			Attr(DAffine2::IDENTITY),
			Attr(<BlendModeAttr as Attribute>::default()),
			Attr(1.),
			Attr(1.),
			Attr(false),
			Attr(<EditorLayerPath as Attribute>::default()),
		));
	};

	// Create a new image for the output element
	let mut image = Image::new(width, height, Color::TRANSPARENT);

	// Iterate over all pixels in the image and set the color channels
	for y in 0..image.height() {
		for x in 0..image.width() {
			let image_pixel = image.get_pixel_mut(x, y).unwrap();

			if let Some(r) = red_el.and_then(|r| r.get_pixel(x, y)) {
				image_pixel.set_red(r.l().cast_linear_channel());
			} else {
				image_pixel.set_red(Channel::from_linear(0.));
			}
			if let Some(g) = green_el.and_then(|g| g.get_pixel(x, y)) {
				image_pixel.set_green(g.l().cast_linear_channel());
			} else {
				image_pixel.set_green(Channel::from_linear(0.));
			}
			if let Some(b) = blue_el.and_then(|b| b.get_pixel(x, y)) {
				image_pixel.set_blue(b.l().cast_linear_channel());
			} else {
				image_pixel.set_blue(Channel::from_linear(0.));
			}
			if let Some(a) = alpha_el.and_then(|a| a.get_pixel(x, y)) {
				image_pixel.set_alpha(a.l().cast_linear_channel());
			} else {
				image_pixel.set_alpha(Channel::from_linear(1.));
			}
		}
	}

	// The layer path re-parks into the arena so the borrow outlives the batch
	let layer_path: Vec<core_types::uuid::NodeId> = source.attr::<EditorLayerPath>().to_vec();
	let (layer_path, _) = ctx.arena().alloc(layer_path).ok_or(GraphError {
		kind: core_types::gpoll::ErrorKind::ArenaExhausted,
		trace: Vec::new(),
	})?;

	Ok((
		Raster::new_cpu(image),
		Attr(source.attr::<TransformAttr>()),
		Attr(source.attr::<BlendModeAttr>()),
		Attr(source.attr::<Opacity>()),
		Attr(source.attr::<OpacityFill>()),
		Attr(source.attr::<ClippingMask>()),
		Attr(layer_path.as_slice()),
	))
}

/// The combined level's count is the longest channel's; a lower-bound channel
/// keeps the result a lower bound too, and consumers drain to past-end.
fn combine_channels_extent(
	_primary: ValueIn<'_, ()>,
	red: ListIn<'_, Raster<CPU>>,
	green: ListIn<'_, Raster<CPU>>,
	blue: ListIn<'_, Raster<CPU>>,
	alpha: ListIn<'_, Raster<CPU>>,
	level: LevelIn,
) -> GPoll<Extent> {
	match level.top() {
		true => red.total().zip(green.total()).zip(blue.total()).zip(alpha.total()).map(|(((red, green), blue), alpha)| {
			let totals = [red, green, blue, alpha];
			let bound = totals
				.iter()
				.map(|extent| match extent {
					Extent::Exactly(count) | Extent::AtLeast(count) => *count,
					Extent::Free => 0,
				})
				.max()
				.unwrap_or(0);
			match totals.iter().all(|extent| matches!(extent, Extent::Exactly(_))) {
				true => Extent::Exactly(bound),
				false => Extent::AtLeast(bound),
			}
		}),
		false => GPoll::Final(Extent::Exactly(1)),
	}
}

#[node_macro::node(category("Raster"))]
pub fn mask(
	_: impl Ctx,
	/// The image to be masked.
	(mut image, lane_transform): (Raster<CPU>, Attr<TransformAttr>),
	/// The stencil to be used for masking.
	#[expose]
	stencil: IList<Raster<CPU>>,
) -> (Raster<CPU>, Attr<TransformAttr>) {
	// TODO: Figure out what it means to support multiple stencil items?
	if stencil.is_empty() {
		// No stencil provided so we return the original image
		return (image, Attr(*lane_transform));
	}
	let stencil_element = stencil.element_ref(0);
	let stencil_transform: DAffine2 = stencil.lane(0).attr::<TransformAttr>();
	let stencil_size = DVec2::new(stencil_element.width as f64, stencil_element.height as f64);

	let image_size = DVec2::new(image.width as f64, image.height as f64);
	let mask_size = stencil_transform.scale_magnitudes();

	// A degenerate stencil serves a zero-size raster, which renders as
	// nothing (the legacy form dropped the lane)
	if mask_size == DVec2::ZERO {
		return (Raster::new_cpu(Image::default()), Attr(*lane_transform));
	}

	// Transforms a point from the background image to the foreground image
	let transform_attribute: DAffine2 = *lane_transform;
	let bg_to_fg = transform_attribute * DAffine2::from_scale(1. / image_size);
	let stencil_transform_inverse = stencil_transform.inverse();

	for y in 0..image.height {
		for x in 0..image.width {
			let image_point = DVec2::new(x as f64, y as f64);
			let mask_point = bg_to_fg.transform_point2(image_point);
			let local_mask_point = stencil_transform_inverse.transform_point2(mask_point);
			let mask_point = stencil_transform.transform_point2(local_mask_point.clamp(DVec2::ZERO, DVec2::ONE));
			let mask_point = (DAffine2::from_scale(stencil_size) * stencil_transform.inverse()).transform_point2(mask_point);

			let image_pixel = image.data_mut().get_pixel_mut(x, y).unwrap();
			let mask_pixel = stencil_element.sample(mask_point);
			*image_pixel = image_pixel.multiplied_alpha(mask_pixel.l().cast_linear_channel());
		}
	}

	(image, Attr(transform_attribute))
}

/// The per-lane extend, shared with the brush's plain callers.
pub fn extend_image_to_bounds_core(image: Raster<CPU>, row_transform: DAffine2, bounds: DAffine2) -> (Raster<CPU>, DAffine2) {
	let image_aabb = Bbox::unit().affine_transform(row_transform).to_axis_aligned_bbox();
	let bounds_aabb = Bbox::unit().affine_transform(bounds.transform()).to_axis_aligned_bbox();
	if image_aabb.contains(bounds_aabb.start) && image_aabb.contains(bounds_aabb.end) {
		return (image, row_transform);
	}

	let (image_width, image_height) = (image.width, image.height);
	if image_width == 0 || image_height == 0 {
		return (empty_image_core(bounds, Color::TRANSPARENT), bounds);
	}
	let image_data = &image.data;

	let orig_image_scale = DVec2::new(image_width as f64, image_height as f64);
	let layer_to_image_space = DAffine2::from_scale(orig_image_scale) * row_transform.inverse();
	let bounds_in_image_space = Bbox::unit().affine_transform(layer_to_image_space * bounds).to_axis_aligned_bbox();

	let new_start = bounds_in_image_space.start.floor().min(DVec2::ZERO);
	let new_end = bounds_in_image_space.end.ceil().max(orig_image_scale);
	let new_scale = new_end - new_start;

	// Copy over original image into enlarged image.
	let mut new_image = Image::new(new_scale.x as u32, new_scale.y as u32, Color::TRANSPARENT);
	let offset_in_new_image = (-new_start).as_uvec2();
	for y in 0..image_height {
		let old_start = y * image_width;
		let new_start = (y + offset_in_new_image.y) * new_image.width + offset_in_new_image.x;
		let old_row = &image_data[old_start as usize..(old_start + image_width) as usize];
		let new_row = &mut new_image.data[new_start as usize..(new_start + image_width) as usize];
		new_row.copy_from_slice(old_row);
	}

	// Compute new transform.
	// let layer_to_new_texture_space = (DAffine2::from_scale(1. / new_scale) * DAffine2::from_translation(new_start) * layer_to_image_space).inverse();
	let new_texture_to_layer_space = row_transform * DAffine2::from_scale(1. / orig_image_scale) * DAffine2::from_translation(new_start) * DAffine2::from_scale(new_scale);

	(Raster::new_cpu(new_image), new_texture_to_layer_space)
}

#[node_macro::node(category(""))]
pub fn extend_image_to_bounds(_: impl Ctx, (image, transform): (Raster<CPU>, Attr<TransformAttr>), bounds: DAffine2) -> (Raster<CPU>, Attr<TransformAttr>) {
	let (image, transform) = extend_image_to_bounds_core(image, *transform, bounds);
	(image, Attr(transform))
}

/// The blank texture a transform spans, shared with the brush's plain callers.
pub fn empty_image_core(transform: DAffine2, color: Color) -> Raster<CPU> {
	let width = transform.transform_vector2(DVec2::new(1., 0.)).length() as u32;
	let height = transform.transform_vector2(DVec2::new(0., 1.)).length() as u32;

	Raster::new_cpu(Image::new(width, height, color))
}

#[node_macro::node(category("Debug"))]
pub fn empty_image(_: impl Ctx, transform: DAffine2, color: IList<Color>) -> (Raster<CPU>, Attr<TransformAttr>) {
	let color = match color.len() {
		0 => Color::WHITE,
		_ => color.get(0),
	};
	(empty_image_core(transform, color), Attr(transform))
}

#[node_macro::node(category(""))]
pub fn image(_: impl Ctx, resource: Resource) -> Raster<CPU> {
	let image_data = resource.as_ref();

	// A zero-size raster renders as nothing, matching the legacy empty list.
	let Some(image) = ::image::load_from_memory(image_data).ok() else {
		return Raster::new_cpu(Image::default());
	};
	let image = image.to_rgba32f();
	let image = Image {
		data: image
			.chunks(4)
			.map(|pixel| {
				let alpha = pixel[3];
				Color::from_gamma_srgb_channels(pixel[0] * alpha, pixel[1] * alpha, pixel[2] * alpha, alpha)
			})
			.collect(),
		width: image.width(),
		height: image.height(),
		..Default::default()
	};
	Raster::new_cpu(image)
}

/// Generates customizable procedural noise patterns.
#[node_macro::node(category("Raster: Pattern"))]
#[allow(clippy::too_many_arguments)]
pub fn noise_pattern(
	ctx: impl ExtractFootprint + Ctx,
	_primary: (),
	#[default(true)] clip: bool,
	seed: u32,
	#[widget(ParsedWidgetOverride::Custom = "noise_properties_scale")]
	#[default(10.)]
	scale: f64,
	#[widget(ParsedWidgetOverride::Custom = "noise_properties_noise_type")] noise_type: NoiseType,
	#[widget(ParsedWidgetOverride::Custom = "noise_properties_domain_warp_type")] domain_warp_type: DomainWarpType,
	#[widget(ParsedWidgetOverride::Custom = "noise_properties_domain_warp_amplitude")]
	#[default(100.)]
	domain_warp_amplitude: f64,
	#[widget(ParsedWidgetOverride::Custom = "noise_properties_fractal_type")] fractal_type: FractalType,
	#[widget(ParsedWidgetOverride::Custom = "noise_properties_fractal_octaves")]
	#[default(3)]
	fractal_octaves: u32,
	#[widget(ParsedWidgetOverride::Custom = "noise_properties_fractal_lacunarity")]
	#[default(2.)]
	fractal_lacunarity: f64,
	#[widget(ParsedWidgetOverride::Custom = "noise_properties_fractal_gain")]
	#[default(0.5)]
	fractal_gain: f64,
	#[widget(ParsedWidgetOverride::Custom = "noise_properties_fractal_weighted_strength")] fractal_weighted_strength: f64,
	#[widget(ParsedWidgetOverride::Custom = "noise_properties_ping_pong_strength")]
	#[default(2.)]
	fractal_ping_pong_strength: f64,
	#[widget(ParsedWidgetOverride::Custom = "noise_properties_cellular_distance_function")] cellular_distance_function: CellularDistanceFunction,
	#[widget(ParsedWidgetOverride::Custom = "noise_properties_cellular_return_type")] cellular_return_type: CellularReturnType,
	#[widget(ParsedWidgetOverride::Custom = "noise_properties_cellular_jitter")]
	#[default(1.)]
	cellular_jitter: f64,
) -> (Raster<CPU>, Attr<TransformAttr>) {
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

	// A culled pattern serves a zero-size raster, which renders as nothing
	if size.x <= 0. || size.y <= 0. {
		return (Raster::new_cpu(Image::default()), Attr(DAffine2::IDENTITY));
	}

	let transform = DAffine2::from_translation(offset) * DAffine2::from_scale(size);

	let footprint_scale = footprint.scale();
	let width = (size.x * footprint_scale.x) as u32;
	let height = (size.y * footprint_scale.y) as u32;

	// All
	let mut image = Image::new(width, height, Color::from_luminance(0.5));
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

			return (Raster::new_cpu(image), Attr(transform));
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

	(Raster::new_cpu(image), Attr(transform))
}

#[node_macro::node(category("Raster: Pattern"))]
pub fn mandelbrot(ctx: impl Ctx + ExtractFootprint, _primary: ()) -> (Raster<CPU>, Attr<TransformAttr>) {
	let footprint = ctx.footprint();
	let viewport_bounds = footprint.viewport_bounds_in_local_space();

	let image_bounds = Bbox::from_transform(DAffine2::IDENTITY).to_axis_aligned_bbox();
	let intersection = viewport_bounds.intersect(&image_bounds);
	let size = intersection.size();

	let offset = (intersection.start - image_bounds.start).max(DVec2::ZERO);

	// A culled pattern serves a zero-size raster, which renders as nothing
	if size.x <= 0. || size.y <= 0. {
		return (Raster::new_cpu(Image::default()), Attr(DAffine2::IDENTITY));
	}

	let scale = footprint.scale();
	let width = (size.x * scale.x) as u32;
	let height = (size.y * scale.y) as u32;

	let mut data = Vec::with_capacity(width as usize * height as usize);
	let max_iter = 255;

	let scale = 3. * size.as_vec2() / Vec2::new(width as f32, height as f32);
	let coordinate_offset = offset.as_vec2() * 3. - Vec2::new(2., 1.5);
	for y in 0..height {
		for x in 0..width {
			let pos = Vec2::new(x as f32, y as f32);
			let c = pos * scale + coordinate_offset;

			let iter = mandelbrot_impl(c, max_iter);
			data.push(map_color(iter, max_iter));
		}
	}

	(
		Raster::new_cpu(Image {
			width,
			height,
			data,
			..Default::default()
		}),
		Attr(DAffine2::from_translation(offset) * DAffine2::from_scale(size)),
	)
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
