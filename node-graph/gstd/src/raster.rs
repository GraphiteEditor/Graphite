use crate::wasm_application_io::WasmEditorApi;

use dyn_any::DynAny;
use graph_craft::imaginate_input::{ImaginateController, ImaginateMaskStartingFill, ImaginateSamplingMethod};
use graph_craft::proto::DynFuture;
use graphene_core::raster::bbox::Bbox;
use graphene_core::raster::{
	Alpha, Bitmap, BitmapMut, CellularDistanceFunction, CellularReturnType, DomainWarpType, FractalType, Image, ImageFrame, Linear, LinearChannel, Luminance, NoiseType, Pixel, RGBMut, RedGreenBlue,
	Sample,
};
use graphene_core::transform::{Footprint, Transform};
use graphene_core::{AlphaBlending, Color, Node};

use fastnoise_lite;
use glam::{DAffine2, DVec2, Vec2};
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::marker::PhantomData;

#[derive(Debug, DynAny)]
pub enum Error {
	IO(std::io::Error),
	Image(image::ImageError),
}

impl From<std::io::Error> for Error {
	fn from(e: std::io::Error) -> Self {
		Error::IO(e)
	}
}

#[node_macro::node(category("Debug: Raster"))]
fn sample_image(footprint: Footprint, image_frame: ImageFrame<Color>) -> ImageFrame<Color> {
	// Resize the image using the image crate
	let image = image_frame.image;
	let data = bytemuck::cast_vec(image.data);

	let viewport_bounds = footprint.viewport_bounds_in_local_space();
	let image_bounds = Bbox::from_transform(image_frame.transform).to_axis_aligned_bbox();
	let intersection = viewport_bounds.intersect(&image_bounds);
	let image_size = DAffine2::from_scale(DVec2::new(image.width as f64, image.height as f64));
	let size = intersection.size();
	let size_px = image_size.transform_vector2(size).as_uvec2();

	// If the image would not be visible, return an empty image
	if size.x <= 0. || size.y <= 0. {
		return ImageFrame::empty();
	}

	let image_buffer = image::Rgba32FImage::from_raw(image.width, image.height, data).expect("Failed to convert internal ImageFrame into image-rs data type.");

	let dynamic_image: image::DynamicImage = image_buffer.into();
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
		cropped.resize_exact(new_width, new_height, image::imageops::Triangle)
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

	let new_transform = image_frame.transform * DAffine2::from_translation(offset) * DAffine2::from_scale(size);
	ImageFrame {
		image,
		transform: new_transform,
		alpha_blending: image_frame.alpha_blending,
	}
}

#[derive(Debug, Clone, Copy)]
pub struct MapImageNode<P, MapFn> {
	map_fn: MapFn,
	_p: PhantomData<P>,
}

#[node_macro::old_node_fn(MapImageNode<_P>)]
fn map_image<MapFn, _P, Img: BitmapMut<Pixel = _P>>(image: Img, map_fn: &'input MapFn) -> Img
where
	MapFn: for<'any_input> Node<'any_input, _P, Output = _P> + 'input,
{
	let mut image = image;

	image.map_pixels(|c| map_fn.eval(c));
	image
}

#[derive(Debug, Clone, Copy)]
pub struct InsertChannelNode<P, S, Insertion, TargetChannel> {
	insertion: Insertion,
	target_channel: TargetChannel,
	_p: PhantomData<P>,
	_s: PhantomData<S>,
}

#[node_macro::old_node_fn(InsertChannelNode<_P, _S>)]
fn insert_channel<
	// _P is the color of the input image.
	_P: RGBMut,
	_S: Pixel + Luminance,
	// Input image
	Input: BitmapMut<Pixel = _P>,
	Insertion: Bitmap<Pixel = _S>,
>(
	mut image: Input,
	insertion: Insertion,
	target_channel: RedGreenBlue,
) -> Input
where
	_P::ColorChannel: Linear,
{
	if insertion.width() == 0 {
		return image;
	}

	if insertion.width() != image.width() || insertion.height() != image.height() {
		log::warn!("Stencil and image have different sizes. This is not supported.");
		return image;
	}

	for y in 0..image.height() {
		for x in 0..image.width() {
			let image_pixel = image.get_pixel_mut(x, y).unwrap();
			let insertion_pixel = insertion.get_pixel(x, y).unwrap();
			match target_channel {
				RedGreenBlue::Red => image_pixel.set_red(insertion_pixel.l().cast_linear_channel()),
				RedGreenBlue::Green => image_pixel.set_green(insertion_pixel.l().cast_linear_channel()),
				RedGreenBlue::Blue => image_pixel.set_blue(insertion_pixel.l().cast_linear_channel()),
			}
		}
	}

	image
}

#[derive(Debug, Clone, Copy)]
pub struct MaskImageNode<P, S, Stencil> {
	stencil: Stencil,
	_p: PhantomData<P>,
	_s: PhantomData<S>,
}

#[node_macro::old_node_fn(MaskImageNode<_P, _S>)]
fn mask_image<
	// _P is the color of the input image. It must have an alpha channel because that is going to
	// be modified by the mask
	_P: Alpha,
	// _S is the color of the stencil. It must have a luminance channel because that is used to
	// mask the input image
	_S: Luminance,
	// Input image
	Input: Transform + BitmapMut<Pixel = _P>,
	// Stencil
	Stencil: Transform + Sample<Pixel = _S>,
>(
	mut image: Input,
	stencil: Stencil,
) -> Input {
	let image_size = DVec2::new(image.width() as f64, image.height() as f64);
	let mask_size = stencil.transform().decompose_scale();

	if mask_size == DVec2::ZERO {
		return image;
	}

	// Transforms a point from the background image to the foreground image
	let bg_to_fg = image.transform() * DAffine2::from_scale(1. / image_size);
	let stencil_transform_inverse = stencil.transform().inverse();

	let area = bg_to_fg.transform_vector2(DVec2::ONE);
	for y in 0..image.height() {
		for x in 0..image.width() {
			let image_point = DVec2::new(x as f64, y as f64);
			let mut mask_point = bg_to_fg.transform_point2(image_point);
			let local_mask_point = stencil_transform_inverse.transform_point2(mask_point);
			mask_point = stencil.transform().transform_point2(local_mask_point.clamp(DVec2::ZERO, DVec2::ONE));

			let image_pixel = image.get_pixel_mut(x, y).unwrap();
			if let Some(mask_pixel) = stencil.sample(mask_point, area) {
				*image_pixel = image_pixel.multiplied_alpha(mask_pixel.l().cast_linear_channel());
			}
		}
	}

	image
}

#[derive(Debug, Clone, Copy)]
pub struct BlendImageTupleNode<P, Fg, MapFn> {
	map_fn: MapFn,
	_p: PhantomData<P>,
	_fg: PhantomData<Fg>,
}

#[node_macro::old_node_fn(BlendImageTupleNode<_P, _Fg>)]
fn blend_image_tuple<_P: Alpha + Pixel + Debug, MapFn, _Fg: Sample<Pixel = _P> + Transform>(images: (ImageFrame<_P>, _Fg), map_fn: &'input MapFn) -> ImageFrame<_P>
where
	MapFn: for<'any_input> Node<'any_input, (_P, _P), Output = _P> + 'input + Clone,
{
	let (background, foreground) = images;

	blend_image(foreground, background, map_fn)
}

fn blend_image<'input, _P: Alpha + Pixel + Debug, MapFn, Frame: Sample<Pixel = _P> + Transform, Background: BitmapMut<Pixel = _P> + Transform + Sample<Pixel = _P>>(
	foreground: Frame,
	background: Background,
	map_fn: &'input MapFn,
) -> Background
where
	MapFn: Node<'input, (_P, _P), Output = _P>,
{
	blend_image_closure(foreground, background, |a, b| map_fn.eval((a, b)))
}

pub fn blend_image_closure<_P: Alpha + Pixel + Debug, MapFn, Frame: Sample<Pixel = _P> + Transform, Background: BitmapMut<Pixel = _P> + Transform + Sample<Pixel = _P>>(
	foreground: Frame,
	mut background: Background,
	map_fn: MapFn,
) -> Background
where
	MapFn: Fn(_P, _P) -> _P,
{
	let background_size = DVec2::new(background.width() as f64, background.height() as f64);
	// Transforms a point from the background image to the foreground image
	let bg_to_fg = background.transform() * DAffine2::from_scale(1. / background_size);

	// Footprint of the foreground image (0,0) (1, 1) in the background image space
	let bg_aabb = Bbox::unit().affine_transform(background.transform().inverse() * foreground.transform()).to_axis_aligned_bbox();

	// Clamp the foreground image to the background image
	let start = (bg_aabb.start * background_size).max(DVec2::ZERO).as_uvec2();
	let end = (bg_aabb.end * background_size).min(background_size).as_uvec2();

	let area = bg_to_fg.transform_point2(DVec2::new(1., 1.)) - bg_to_fg.transform_point2(DVec2::ZERO);
	for y in start.y..end.y {
		for x in start.x..end.x {
			let bg_point = DVec2::new(x as f64, y as f64);
			let fg_point = bg_to_fg.transform_point2(bg_point);

			if let Some(src_pixel) = foreground.sample(fg_point, area) {
				if let Some(dst_pixel) = background.get_pixel_mut(x, y) {
					*dst_pixel = map_fn(src_pixel, *dst_pixel);
				}
			}
		}
	}

	background
}

#[derive(Debug, Clone, Copy)]
pub struct ExtendImageToBoundsNode<Bounds> {
	bounds: Bounds,
}

#[node_macro::old_node_fn(ExtendImageToBoundsNode)]
fn extend_image_to_bounds(image: ImageFrame<Color>, bounds: DAffine2) -> ImageFrame<Color> {
	let image_aabb = Bbox::unit().affine_transform(image.transform()).to_axis_aligned_bbox();
	let bounds_aabb = Bbox::unit().affine_transform(bounds.transform()).to_axis_aligned_bbox();
	if image_aabb.contains(bounds_aabb.start) && image_aabb.contains(bounds_aabb.end) {
		return image;
	}

	if image.image.width == 0 || image.image.height == 0 {
		return empty_image((), bounds, Color::TRANSPARENT);
	}

	let orig_image_scale = DVec2::new(image.image.width as f64, image.image.height as f64);
	let layer_to_image_space = DAffine2::from_scale(orig_image_scale) * image.transform.inverse();
	let bounds_in_image_space = Bbox::unit().affine_transform(layer_to_image_space * bounds).to_axis_aligned_bbox();

	let new_start = bounds_in_image_space.start.floor().min(DVec2::ZERO);
	let new_end = bounds_in_image_space.end.ceil().max(orig_image_scale);
	let new_scale = new_end - new_start;

	// Copy over original image into enlarged image.
	let mut new_img = Image::new(new_scale.x as u32, new_scale.y as u32, Color::TRANSPARENT);
	let offset_in_new_image = (-new_start).as_uvec2();
	for y in 0..image.image.height {
		let old_start = y * image.image.width;
		let new_start = (y + offset_in_new_image.y) * new_img.width + offset_in_new_image.x;
		let old_row = &image.image.data[old_start as usize..(old_start + image.image.width) as usize];
		let new_row = &mut new_img.data[new_start as usize..(new_start + image.image.width) as usize];
		new_row.copy_from_slice(old_row);
	}

	// Compute new transform.
	// let layer_to_new_texture_space = (DAffine2::from_scale(1. / new_scale) * DAffine2::from_translation(new_start) * layer_to_image_space).inverse();
	let new_texture_to_layer_space = image.transform * DAffine2::from_scale(1. / orig_image_scale) * DAffine2::from_translation(new_start) * DAffine2::from_scale(new_scale);
	ImageFrame {
		image: new_img,
		transform: new_texture_to_layer_space,
		alpha_blending: image.alpha_blending,
	}
}

#[node_macro::node(category("Debug: Raster"))]
fn empty_image<P: Pixel>(_: (), transform: DAffine2, #[implementations(Color)] color: P) -> ImageFrame<P> {
	let width = transform.transform_vector2(DVec2::new(1., 0.)).length() as u32;
	let height = transform.transform_vector2(DVec2::new(0., 1.)).length() as u32;

	let image = Image::new(width, height, color);

	ImageFrame {
		image,
		transform,
		alpha_blending: AlphaBlending::default(),
	}
}

#[cfg(feature = "serde")]
macro_rules! generate_imaginate_node {
	($($val:ident: $t:ident: $o:ty,)*) => {
		pub struct ImaginateNode<P: Pixel, E, C, G, $($t,)*> {
			editor_api: E,
			controller: C,
			generation_id: G,
			$($val: $t,)*
			cache: std::sync::Arc<std::sync::Mutex<HashMap<u64, Image<P>>>>,
			last_generation: std::sync::atomic::AtomicU64,
		}

		impl<'e, P: Pixel, E, C, G, $($t,)*> ImaginateNode<P, E, C, G, $($t,)*>
		where $($t: for<'any_input> Node<'any_input, (), Output = DynFuture<'any_input, $o>>,)*
			E: for<'any_input> Node<'any_input, (), Output = DynFuture<'any_input, &'e WasmEditorApi>>,
			C: for<'any_input> Node<'any_input, (), Output = DynFuture<'any_input, ImaginateController>>,
			G: for<'any_input> Node<'any_input, (), Output = DynFuture<'any_input, u64>>,
		{
			#[allow(clippy::too_many_arguments)]
			pub fn new(editor_api: E, controller: C, $($val: $t,)*  generation_id: G ) -> Self {
				Self { editor_api, controller, generation_id, $($val,)* cache: Default::default(), last_generation: std::sync::atomic::AtomicU64::new(u64::MAX) }
			}
		}

		impl<'i, 'e: 'i, P: Pixel + 'i + Hash + Default + Send, E: 'i, C: 'i, G: 'i, $($t: 'i,)*> Node<'i, ImageFrame<P>> for ImaginateNode<P, E, C, G, $($t,)*>
		where $($t: for<'any_input> Node<'any_input, (), Output = DynFuture<'any_input, $o>>,)*
			E: for<'any_input> Node<'any_input, (), Output = DynFuture<'any_input, &'e WasmEditorApi>>,
			C: for<'any_input> Node<'any_input, (), Output = DynFuture<'any_input, ImaginateController>>,
			G: for<'any_input> Node<'any_input, (), Output = DynFuture<'any_input, u64>>,
		{
			type Output = DynFuture<'i, ImageFrame<P>>;

			fn eval(&'i self, frame: ImageFrame<P>) -> Self::Output {
				let controller = self.controller.eval(());
				$(let $val = self.$val.eval(());)*

				use std::hash::Hasher;
				let mut hasher = rustc_hash::FxHasher::default();
				frame.image.hash(&mut hasher);
				let hash = hasher.finish();
				let editor_api = self.editor_api.eval(());
				let cache = self.cache.clone();
				let generation_future = self.generation_id.eval(());
				let last_generation = &self.last_generation;

				Box::pin(async move {
					let controller: ImaginateController = controller.await;
					let generation_id = generation_future.await;
					if generation_id !=  last_generation.swap(generation_id, std::sync::atomic::Ordering::SeqCst) {
						let image = super::imaginate::imaginate(frame.image, editor_api, controller, $($val,)*).await;

						cache.lock().unwrap().insert(hash, image.clone());

						return wrap_image_frame(image, frame.transform);
					}
					let image = cache.lock().unwrap().get(&hash).cloned().unwrap_or_default();

					return wrap_image_frame(image, frame.transform);
				})
			}
		}
	}
}

fn wrap_image_frame<P: Pixel>(image: Image<P>, transform: DAffine2) -> ImageFrame<P> {
	if !transform.decompose_scale().abs_diff_eq(DVec2::ZERO, 0.00001) {
		ImageFrame {
			image,
			transform,
			alpha_blending: AlphaBlending::default(),
		}
	} else {
		let resolution = DVec2::new(image.height as f64, image.width as f64);
		ImageFrame {
			image,
			transform: DAffine2::from_scale_angle_translation(resolution, 0., transform.translation),
			alpha_blending: AlphaBlending::default(),
		}
	}
}

#[cfg(feature = "serde")]
generate_imaginate_node! {
	seed: Seed: f64,
	res: Res: Option<DVec2>,
	samples: Samples: u32,
	sampling_method: SamplingMethod: ImaginateSamplingMethod,
	prompt_guidance: PromptGuidance: f64,
	prompt: Prompt: String,
	negative_prompt: NegativePrompt: String,
	adapt_input_image: AdaptInputImage: bool,
	image_creativity: ImageCreativity: f64,
	inpaint: Inpaint: bool,
	mask_blur: MaskBlur: f64,
	mask_starting_fill: MaskStartingFill: ImaginateMaskStartingFill,
	improve_faces: ImproveFaces: bool,
	tiling: Tiling: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct ImageFrameNode<P, Transform> {
	transform: Transform,
	_p: PhantomData<P>,
}
#[node_macro::old_node_fn(ImageFrameNode<_P>)]
fn image_frame<_P: Pixel>(image: Image<_P>, transform: DAffine2) -> ImageFrame<_P> {
	ImageFrame {
		image,
		transform,
		alpha_blending: AlphaBlending::default(),
	}
}

#[node_macro::node(category("Raster: Generator"))]
#[allow(clippy::too_many_arguments)]
fn noise_pattern(
	footprint: Footprint,
	// The first input is not shown in the properties panel, so this spacer is needed.
	_empty_spacer_input: bool,
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
) -> ImageFrame<Color> {
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
		return ImageFrame::empty();
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
					let luminance = rng.gen_range(0.0..1.) as f32;
					*pixel = Color::from_luminance(luminance);
				}
			}

			return ImageFrame::<Color> {
				image,
				transform: DAffine2::from_translation(offset) * DAffine2::from_scale(size),
				alpha_blending: AlphaBlending::default(),
			};
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

	// Return the coherent noise image
	ImageFrame::<Color> {
		image,
		transform: DAffine2::from_translation(offset) * DAffine2::from_scale(size),
		alpha_blending: AlphaBlending::default(),
	}
}

#[node_macro::node(category("Raster: Generator"))]
fn mandelbrot(footprint: Footprint) -> ImageFrame<Color> {
	let viewport_bounds = footprint.viewport_bounds_in_local_space();

	let image_bounds = Bbox::from_transform(DAffine2::IDENTITY).to_axis_aligned_bbox();
	let intersection = viewport_bounds.intersect(&image_bounds);
	let size = intersection.size();

	let offset = (intersection.start - image_bounds.start).max(DVec2::ZERO);

	// If the image would not be visible, return an empty image
	if size.x <= 0. || size.y <= 0. {
		return ImageFrame::empty();
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
	ImageFrame {
		image: Image {
			width,
			height,
			data,
			..Default::default()
		},
		transform: DAffine2::from_translation(offset) * DAffine2::from_scale(size),
		..Default::default()
	}
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
