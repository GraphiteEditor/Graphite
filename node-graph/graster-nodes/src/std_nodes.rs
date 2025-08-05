use crate::adjustments::{CellularDistanceFunction, CellularReturnType, DomainWarpType, FractalType, NoiseType};
use dyn_any::DynAny;
use fastnoise_lite;
use glam::{DAffine2, DVec2, Vec2};
use graphene_core::blending::AlphaBlending;
use graphene_core::color::Color;
use graphene_core::color::{Alpha, AlphaMut, Channel, LinearChannel, Luminance, RGBMut};
use graphene_core::context::{Ctx, ExtractFootprint};
use graphene_core::math::bbox::Bbox;
use graphene_core::raster::image::Image;
use graphene_core::raster::{Bitmap, BitmapMut};
use graphene_core::raster_types::{CPU, Raster};
use graphene_core::table::{Table, TableRow};
use graphene_core::transform::Transform;
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
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

#[node_macro::node(category("Debug: Raster"))]
pub fn sample_image(ctx: impl ExtractFootprint + Clone + Send, image_frame: Table<Raster<CPU>>) -> Table<Raster<CPU>> {
	image_frame
		.into_iter()
		.filter_map(|mut row| {
			let image_frame_transform = row.transform;
			let image = row.element;

			// Resize the image using the image crate
			let data = bytemuck::cast_vec(image.data.clone());

			let footprint = ctx.footprint();
			let viewport_bounds = footprint.viewport_bounds_in_local_space();
			let image_bounds = Bbox::from_transform(image_frame_transform).to_axis_aligned_bbox();
			let intersection = viewport_bounds.intersect(&image_bounds);
			let image_size = DAffine2::from_scale(DVec2::new(image.width as f64, image.height as f64));
			let size = intersection.size();
			let size_px = image_size.transform_vector2(size).as_uvec2();

			// If the image would not be visible, add nothing.
			if size.x <= 0. || size.y <= 0. {
				return None;
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

			row.transform = new_transform;
			row.element = Raster::new_cpu(image);
			Some(row)
		})
		.collect()
}

#[node_macro::node(category("Raster: Channels"))]
pub fn combine_channels(
	_: impl Ctx,
	_primary: (),
	#[expose] red: Table<Raster<CPU>>,
	#[expose] green: Table<Raster<CPU>>,
	#[expose] blue: Table<Raster<CPU>>,
	#[expose] alpha: Table<Raster<CPU>>,
) -> Table<Raster<CPU>> {
	let max_len = red.len().max(green.len()).max(blue.len()).max(alpha.len());
	let red = red.into_iter().map(Some).chain(std::iter::repeat(None)).take(max_len);
	let green = green.into_iter().map(Some).chain(std::iter::repeat(None)).take(max_len);
	let blue = blue.into_iter().map(Some).chain(std::iter::repeat(None)).take(max_len);
	let alpha = alpha.into_iter().map(Some).chain(std::iter::repeat(None)).take(max_len);

	red.zip(green)
		.zip(blue)
		.zip(alpha)
		.filter_map(|(((red, green), blue), alpha)| {
			// Turn any default zero-sized image rows into None
			let red = red.filter(|i| i.element.width > 0 && i.element.height > 0);
			let green = green.filter(|i| i.element.width > 0 && i.element.height > 0);
			let blue = blue.filter(|i| i.element.width > 0 && i.element.height > 0);
			let alpha = alpha.filter(|i| i.element.width > 0 && i.element.height > 0);

			// Get this row's transform and alpha blending mode from the first non-empty channel
			let (transform, alpha_blending, source_node_id) = [&red, &green, &blue, &alpha]
				.iter()
				.find_map(|i| i.as_ref())
				.map(|i| (i.transform, i.alpha_blending, i.source_node_id))?;

			// Get the common width and height of the channels, which must have equal dimensions
			let channel_dimensions = [
				red.as_ref().map(|r| (r.element.width, r.element.height)),
				green.as_ref().map(|g| (g.element.width, g.element.height)),
				blue.as_ref().map(|b| (b.element.width, b.element.height)),
				alpha.as_ref().map(|a| (a.element.width, a.element.height)),
			];
			if channel_dimensions.iter().all(Option::is_none)
				|| channel_dimensions
					.iter()
					.flatten()
					.any(|&(x, y)| channel_dimensions.iter().flatten().any(|&(other_x, other_y)| x != other_x || y != other_y))
			{
				return None;
			}
			let &(width, height) = channel_dimensions.iter().flatten().next()?;

			// Create a new image for the output element
			let mut image = Image::new(width, height, Color::TRANSPARENT);

			// Iterate over all pixels in the image and set the color channels
			for y in 0..image.height() {
				for x in 0..image.width() {
					let image_pixel = image.get_pixel_mut(x, y).unwrap();

					if let Some(r) = red.as_ref().and_then(|r| r.element.get_pixel(x, y)) {
						image_pixel.set_red(r.l().cast_linear_channel());
					} else {
						image_pixel.set_red(Channel::from_linear(0.));
					}
					if let Some(g) = green.as_ref().and_then(|g| g.element.get_pixel(x, y)) {
						image_pixel.set_green(g.l().cast_linear_channel());
					} else {
						image_pixel.set_green(Channel::from_linear(0.));
					}
					if let Some(b) = blue.as_ref().and_then(|b| b.element.get_pixel(x, y)) {
						image_pixel.set_blue(b.l().cast_linear_channel());
					} else {
						image_pixel.set_blue(Channel::from_linear(0.));
					}
					if let Some(a) = alpha.as_ref().and_then(|a| a.element.get_pixel(x, y)) {
						image_pixel.set_alpha(a.l().cast_linear_channel());
					} else {
						image_pixel.set_alpha(Channel::from_linear(1.));
					}
				}
			}

			Some(TableRow {
				element: Raster::new_cpu(image),
				transform,
				alpha_blending,
				source_node_id,
			})
		})
		.collect()
}

#[node_macro::node(category("Raster"))]
pub fn mask(
	_: impl Ctx,
	/// The image to be masked.
	image: Table<Raster<CPU>>,
	/// The stencil to be used for masking.
	#[expose]
	stencil: Table<Raster<CPU>>,
) -> Table<Raster<CPU>> {
	// TODO: Figure out what it means to support multiple stencil rows?
	let Some(stencil) = stencil.into_iter().next() else {
		// No stencil provided so we return the original image
		return image;
	};
	let stencil_size = DVec2::new(stencil.element.width as f64, stencil.element.height as f64);

	image
		.into_iter()
		.filter_map(|mut row| {
			let image_size = DVec2::new(row.element.width as f64, row.element.height as f64);
			let mask_size = stencil.transform.decompose_scale();

			if mask_size == DVec2::ZERO {
				return None;
			}

			// Transforms a point from the background image to the foreground image
			let bg_to_fg = row.transform * DAffine2::from_scale(1. / image_size);
			let stencil_transform_inverse = stencil.transform.inverse();

			for y in 0..row.element.height {
				for x in 0..row.element.width {
					let image_point = DVec2::new(x as f64, y as f64);
					let mask_point = bg_to_fg.transform_point2(image_point);
					let local_mask_point = stencil_transform_inverse.transform_point2(mask_point);
					let mask_point = stencil.transform.transform_point2(local_mask_point.clamp(DVec2::ZERO, DVec2::ONE));
					let mask_point = (DAffine2::from_scale(stencil_size) * stencil.transform.inverse()).transform_point2(mask_point);

					let image_pixel = row.element.data_mut().get_pixel_mut(x, y).unwrap();
					let mask_pixel = stencil.element.sample(mask_point);
					*image_pixel = image_pixel.multiplied_alpha(mask_pixel.l().cast_linear_channel());
				}
			}

			Some(row)
		})
		.collect()
}

#[node_macro::node(category(""))]
pub fn extend_image_to_bounds(_: impl Ctx, image: Table<Raster<CPU>>, bounds: DAffine2) -> Table<Raster<CPU>> {
	image
		.into_iter()
		.map(|mut row| {
			let image_aabb = Bbox::unit().affine_transform(row.transform).to_axis_aligned_bbox();
			let bounds_aabb = Bbox::unit().affine_transform(bounds.transform()).to_axis_aligned_bbox();
			if image_aabb.contains(bounds_aabb.start) && image_aabb.contains(bounds_aabb.end) {
				return row;
			}

			let image_data = &row.element.data;
			let (image_width, image_height) = (row.element.width, row.element.height);
			if image_width == 0 || image_height == 0 {
				return empty_image((), bounds, Color::TRANSPARENT).into_iter().next().unwrap();
			}

			let orig_image_scale = DVec2::new(image_width as f64, image_height as f64);
			let layer_to_image_space = DAffine2::from_scale(orig_image_scale) * row.transform.inverse();
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
			let new_texture_to_layer_space = row.transform * DAffine2::from_scale(1. / orig_image_scale) * DAffine2::from_translation(new_start) * DAffine2::from_scale(new_scale);

			row.element = Raster::new_cpu(new_image);
			row.transform = new_texture_to_layer_space;
			row
		})
		.collect()
}

#[node_macro::node(category("Debug: Raster"))]
pub fn empty_image(_: impl Ctx, transform: DAffine2, color: Color) -> Table<Raster<CPU>> {
	let width = transform.transform_vector2(DVec2::new(1., 0.)).length() as u32;
	let height = transform.transform_vector2(DVec2::new(0., 1.)).length() as u32;

	let image = Image::new(width, height, color);

	let mut result_table = Table::new_from_element(Raster::new_cpu(image));
	let row = result_table.get_mut(0).unwrap();
	*row.transform = transform;
	*row.alpha_blending = AlphaBlending::default();

	// Callers of empty_image can safely unwrap on returned table
	result_table
}

/// Constructs a raster image.
#[node_macro::node(category(""))]
pub fn image_value(_: impl Ctx, _primary: (), image: Table<Raster<CPU>>) -> Table<Raster<CPU>> {
	image
}

#[node_macro::node(category("Raster: Pattern"))]
#[allow(clippy::too_many_arguments)]
pub fn noise_pattern(
	ctx: impl ExtractFootprint + Ctx,
	_primary: (),
	clip: bool,
	seed: u32,
	scale: f64,
	noise_type: NoiseType,
	domain_warp_type: DomainWarpType,
	domain_warp_amplitude: f64,
	fractal_type: FractalType,
	fractal_octaves: u32,
	fractal_lacunarity: f64,
	fractal_gain: f64,
	fractal_weighted_strength: f64,
	fractal_ping_pong_strength: f64,
	cellular_distance_function: CellularDistanceFunction,
	cellular_return_type: CellularReturnType,
	cellular_jitter: f64,
) -> Table<Raster<CPU>> {
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
		return Table::new();
	}

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

			return Table::new_from_row(TableRow {
				element: Raster::new_cpu(image),
				transform: DAffine2::from_translation(offset) * DAffine2::from_scale(size),
				..Default::default()
			});
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

	Table::new_from_row(TableRow {
		element: Raster::new_cpu(image),
		transform: DAffine2::from_translation(offset) * DAffine2::from_scale(size),
		..Default::default()
	})
}

#[node_macro::node(category("Raster: Pattern"))]
pub fn mandelbrot(ctx: impl ExtractFootprint + Send) -> Table<Raster<CPU>> {
	let footprint = ctx.footprint();
	let viewport_bounds = footprint.viewport_bounds_in_local_space();

	let image_bounds = Bbox::from_transform(DAffine2::IDENTITY).to_axis_aligned_bbox();
	let intersection = viewport_bounds.intersect(&image_bounds);
	let size = intersection.size();

	let offset = (intersection.start - image_bounds.start).max(DVec2::ZERO);

	// If the image would not be visible, return an empty image
	if size.x <= 0. || size.y <= 0. {
		return Table::new();
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

	Table::new_from_row(TableRow {
		element: Raster::new_cpu(Image {
			width,
			height,
			data,
			..Default::default()
		}),
		transform: DAffine2::from_translation(offset) * DAffine2::from_scale(size),
		..Default::default()
	})
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
