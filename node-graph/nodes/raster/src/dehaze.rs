use crate::gpu_params;
use crate::raster_kernel::{RasterKernel, Wgsl, raster_kernel_pipeline, run_raster_kernel};
use core_types::NodeError;
use core_types::color::Color;
use core_types::context::Ctx;
use core_types::list::Item;
use core_types::ops::ConvertAsync;
use core_types::transform::Footprint;
use glam::DAffine2;
use image::{DynamicImage, GenericImage, GenericImageView, GrayImage, ImageBuffer, Luma, Rgba, RgbaImage};
use ndarray::{Array2, ArrayBase, Dim, OwnedRepr};
use raster_types::Image;
use raster_types::{CPU, GPU, Raster, Storage};
use std::cmp::{max, min};
use wgpu_executor::WgpuPipelineCache;
use wgpu_executor::shader_runtime::raster_kernel_runtime::RasterKernelInput;

#[node_macro::node(category("Raster: Filter"), icon("NodeDehaze"), placeholder = pending_dehaze)]
async fn dehaze<T: DehazeRaster + Clone>(
	_: impl Ctx,
	#[implementations(Raster<CPU>, Raster<GPU>)] image_frame: Item<T>,
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	strength: Item<f64>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, NodeError> {
	let strength = *strength.element();
	T::dehaze(image_frame, strength, pipeline.element()).await
}

fn pending_dehaze<T: Clone>(image_frame: &Item<T>, _strength: &Item<f64>, _pipeline: &Item<WgpuPipelineCache>) -> Item<T> {
	image_frame.clone()
}

fn dehaze_cpu(image_frame: Item<Raster<CPU>>, strength: f64) -> Item<Raster<CPU>> {
	let (image, attributes) = image_frame.into_parts();
	let (width, height) = (image.width, image.height);

	// Prepare the image data for processing
	let image_data = bytemuck::cast_vec(image.into_data().data);
	let image_buffer = image::Rgba32FImage::from_raw(width, height, image_data).expect("Failed to convert internal image format into image-rs data type.");
	let dynamic_image: DynamicImage = image_buffer.into();

	// Run the dehaze algorithm
	let dehazed_dynamic_image = dehaze_image(dynamic_image, strength / 100.);

	// Prepare the image data for returning
	let buffer = dehazed_dynamic_image.to_rgba32f().into_raw();
	let color_vec = bytemuck::cast_vec(buffer);
	let dehazed_image = Image { width, height, data: color_vec };

	Item::from_parts(Raster::new_cpu(dehazed_image), attributes)
}

/// A raster the Dehaze node processes, on the CPU or the GPU.
pub trait DehazeRaster: Sized {
	fn dehaze(image: Item<Self>, strength: f64, pipeline: &WgpuPipelineCache) -> impl Future<Output = Result<Item<Self>, NodeError>> + Send;
}

impl DehazeRaster for Raster<CPU> {
	async fn dehaze(image: Item<Self>, strength: f64, _: &WgpuPipelineCache) -> Result<Item<Self>, NodeError> {
		Ok(dehaze_cpu(image, strength))
	}
}

fn dehaze_input(raster: &Raster<GPU>) -> RasterKernelInput<'_> {
	RasterKernelInput {
		raster,
		output_to_texel: DAffine2::IDENTITY,
	}
}

impl DehazeRaster for Raster<GPU> {
	/// Runs the CPU's algorithm in float without its 8-bit quantization, downloading the dark channel to choose the atmospheric light exactly as the CPU does.
	/// The guided filter's intermediates stay in [`wgpu_executor::RASTER_FORMAT`] since its variances lose their precision at half-float.
	async fn dehaze(image: Item<Self>, strength: f64, pipeline: &WgpuPipelineCache) -> Result<Item<Self>, NodeError> {
		if image.element().is_empty() {
			return Ok(image);
		}

		let executor = pipeline.executor().cloned().ok_or_else(|| NodeError::new("A GPU is required to process a GPU raster"))?;
		let (raster, attributes) = image.into_parts();
		let size = raster.size();

		let radius = (PATCH_SIZE / 2) as i32;
		let eroded = run_raster_kernel(&DehazeErode { radius, vertical: false }, vec![dehaze_input(&raster)], size, pipeline)?;
		let dark_channel = run_raster_kernel(&DehazeErode { radius, vertical: true }, vec![dehaze_input(&eroded)], size, pipeline)?;
		let atmospheric_light = estimate_atmospheric_light_gpu(&dark_channel.clone().convert(Footprint::DEFAULT, executor).await);

		let radius = RADIUS as i32;
		let guide = DehazeGuideHorizontal {
			radius,
			strength: (strength / 100.) as f32,
		};
		let guide = run_raster_kernel(&guide, vec![dehaze_input(&dark_channel)], size, pipeline)?;
		let coefficients = DehazeGuideVertical { radius, epsilon: EPSILON as f32 };
		let coefficients = run_raster_kernel(&coefficients, vec![dehaze_input(&guide)], size, pipeline)?;
		let coefficients = run_raster_kernel(&DehazeCoefficientsHorizontal { radius }, vec![dehaze_input(&coefficients)], size, pipeline)?;
		let recover = DehazeRecover {
			radius,
			minimum_transmission: TX,
			atmospheric_light,
		};
		let dehazed = run_raster_kernel(&recover, vec![dehaze_input(&coefficients), dehaze_input(&raster)], size, pipeline)?;

		Ok(Item::from_parts(dehazed, attributes))
	}
}

/// The average color of the pixels with the brightest dark channel, chosen like [`estimate_atmospheric_light`], from each pixel's color with its dark channel as alpha.
fn estimate_atmospheric_light_gpu(dark_channel: &Raster<CPU>) -> Color {
	let pixels = &dark_channel.data;
	let num_pixels = ((TOP_PERCENT / 100.) * pixels.len() as f64).ceil() as usize;

	let mut order: Vec<usize> = (0..pixels.len()).collect();
	order.select_nth_unstable_by(num_pixels - 1, |&a, &b| pixels[b].a().total_cmp(&pixels[a].a()).then(a.cmp(&b)));

	let mut atm_sum = [0., 0., 0.];
	for &index in &order[..num_pixels] {
		let pixel = pixels[index];
		atm_sum[0] += pixel.r() as f64;
		atm_sum[1] += pixel.g() as f64;
		atm_sum[2] += pixel.b() as f64;
	}

	let num_pixels = num_pixels as f64;
	Color::from_rgbaf32_unchecked((atm_sum[0] / num_pixels) as f32, (atm_sum[1] / num_pixels) as f32, (atm_sum[2] / num_pixels) as f32, 1.)
}

gpu_params! {
	/// One axis of the dark channel's erosion, the horizontal pass eroding each pixel's darkest channel and the vertical pass the horizontal pass's alpha,
	/// both writing the pixel's color clamped to 0 to 1 with the eroded value as alpha.
	#[derive(Clone)]
	struct DehazeErode {
		radius: i32,
		vertical: bool,
	}
}

impl RasterKernel for DehazeErode {
	const INPUTS: usize = 1;
	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/dehaze_erode.wgsl"))
	}
}

gpu_params! {
	/// Horizontal means of the guidance luma, the transmission, the luma squared, and the luma times the transmission.
	#[derive(Clone)]
	struct DehazeGuideHorizontal {
		radius: i32,
		strength: f32,
	}
}

impl RasterKernel for DehazeGuideHorizontal {
	const INPUTS: usize = 1;
	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/dehaze_guide_horizontal.wgsl"))
	}
}

gpu_params! {
	/// Completes the guided filter's means vertically into its linear coefficients.
	#[derive(Clone)]
	struct DehazeGuideVertical {
		radius: i32,
		epsilon: f32,
	}
}

impl RasterKernel for DehazeGuideVertical {
	const INPUTS: usize = 1;
	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/dehaze_guide_vertical.wgsl"))
	}
}

gpu_params! {
	#[derive(Clone)]
	struct DehazeCoefficientsHorizontal {
		radius: i32,
	}
}

impl RasterKernel for DehazeCoefficientsHorizontal {
	const INPUTS: usize = 1;
	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/dehaze_coefficients_horizontal.wgsl"))
	}
}

gpu_params! {
	/// Completes the coefficients' means vertically into the refined transmission, recovering the image from its haze.
	#[derive(Clone)]
	struct DehazeRecover {
		radius: i32,
		minimum_transmission: f32,
		atmospheric_light: Color,
	}
}

impl RasterKernel for DehazeRecover {
	const INPUTS: usize = 2;
	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/dehaze_recover.wgsl"))
	}
}

// There is no real point in modifying these values because they do not change the final result all that much.
// The authors of the paper recommended using these values to get a reasonable balance of performance and quality.
// TODO: The patch size and guided filter radius are whole source pixels, so the effect's scale follows the image's resolution.
// Derive them from the footprint so the dehaze acts at a document-space scale independent of how the image was sampled.
const PATCH_SIZE: u32 = 15;
const TOP_PERCENT: f64 = 0.001;
const RADIUS: u32 = 60;
const EPSILON: f64 = 0.0001;
const TX: f32 = 0.1;

// Dehazing algorithm based on "Single Image Haze Removal Using Dark Channel Prior"
// Paper: <https://www.researchgate.net/publication/220182411_Single_Image_Haze_Removal_Using_Dark_Channel_Prior>
// TODO: Make this algorithm work with negative strength values
fn dehaze_image(image: DynamicImage, strength: f64) -> DynamicImage {
	// TODO: Break out this pair of steps into its own node, with a Memoize node which caches the pair of outputs, so the strength can be adjusted without recomputing these two steps.
	let dark_channel = compute_dark_channel(&image);
	let atmospheric_light = estimate_atmospheric_light(&image, &dark_channel);

	let transmission_map = estimate_transmission_map(&image, &dark_channel, strength);
	let refined_transmission_map = refine_transmission_map(&image, &transmission_map);

	recover(&image, &refined_transmission_map, atmospheric_light)
}

fn compute_dark_channel(image: &DynamicImage) -> DynamicImage {
	let (width, height) = image.dimensions();
	let mut dark_channel = GrayImage::new(width, height);
	let half_patch = PATCH_SIZE / 2;

	for y in 0..height {
		for x in 0..width {
			let pixel = image.get_pixel(x, y);
			let min_intensity = min(min(pixel[0], pixel[1]), pixel[2]);
			dark_channel.put_pixel(x, y, Luma([min_intensity]));
		}
	}

	// The minimum over each square patch is the minimum along its columns of the minimum along its rows, so two passes do it in O(width × height × patch size)
	let row_minimums = GrayImage::from_fn(width, height, |x, y| {
		let (start, end) = (x.saturating_sub(half_patch), (x + half_patch).min(width - 1));
		Luma([(start..=end).map(|nx| dark_channel.get_pixel(nx, y)[0]).min().unwrap_or(u8::MAX)])
	});
	let eroded_channel = RgbaImage::from_fn(width, height, |x, y| {
		let (start, end) = (y.saturating_sub(half_patch), (y + half_patch).min(height - 1));
		let local_min = (start..=end).map(|ny| row_minimums.get_pixel(x, ny)[0]).min().unwrap_or(u8::MAX);
		Rgba([local_min, local_min, local_min, image.get_pixel(x, y)[3]])
	});

	DynamicImage::ImageRgba8(eroded_channel)
}

fn estimate_atmospheric_light(hazy: &DynamicImage, dark_channel: &DynamicImage) -> Rgba<u8> {
	let (width, height) = hazy.dimensions();
	let dark = dark_channel.to_luma_alpha8();
	let total_pixels = (width * height) as usize;
	let num_pixels = ((TOP_PERCENT / 100.) * total_pixels as f64).ceil() as usize;

	let mut intensities: Vec<(u32, u32, f64)> = Vec::with_capacity(total_pixels);

	for y in 0..height {
		for x in 0..width {
			let pixel = dark.get_pixel(x, y);
			let intensity = pixel.0[0] as f64;
			intensities.push((x, y, intensity))
		}
	}

	intensities.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

	let top_intensities = &intensities[..num_pixels];

	let mut atm_sum = [0., 0., 0.];
	for (x, y, _) in top_intensities {
		let pixel = hazy.get_pixel(*x, *y);
		atm_sum[0] += pixel[0] as f64;
		atm_sum[1] += pixel[1] as f64;
		atm_sum[2] += pixel[2] as f64;
	}

	let num_pixels = num_pixels as f64;

	Rgba([(atm_sum[0] / num_pixels) as u8, (atm_sum[1] / num_pixels) as u8, (atm_sum[2] / num_pixels) as u8, 255])
}

fn estimate_transmission_map(image: &DynamicImage, dark_channel: &DynamicImage, omega: f64) -> DynamicImage {
	let (width, height) = image.dimensions();
	let mut transmission_map = RgbaImage::new(width, height);

	for y in 0..height {
		for x in 0..width {
			let min_intensity = dark_channel.get_pixel(x, y).0[0] as f32 / 255.;
			let transmission_value = 1. - omega * min_intensity as f64;
			let alpha = image.get_pixel(x, y)[3];
			transmission_map.put_pixel(
				x,
				y,
				Rgba([(transmission_value * 255.) as u8, (transmission_value * 255.) as u8, (transmission_value * 255.) as u8, alpha]),
			);
		}
	}

	DynamicImage::ImageRgba8(transmission_map)
}

fn refine_transmission_map(img: &DynamicImage, transmission_map: &DynamicImage) -> DynamicImage {
	let gray_image = img.to_luma8();

	let normalized_gray_image: GrayImage = ImageBuffer::from_fn(gray_image.width(), gray_image.height(), |x, y| {
		let pixel = gray_image.get_pixel(x, y);
		let normalized_value = (pixel[0] as f64 / 255.) * 255.;
		Luma([normalized_value as u8])
	});

	let normalized_gray_image = DynamicImage::ImageLuma8(normalized_gray_image);

	guided_filter(&normalized_gray_image, transmission_map, RADIUS, EPSILON)
}

fn recover(im: &DynamicImage, t: &DynamicImage, a: Rgba<u8>) -> DynamicImage {
	let (width, height) = im.dimensions();
	let mut res = DynamicImage::new_rgba8(width, height);

	let a = [a[0] as f32 / 255., a[1] as f32 / 255., a[2] as f32 / 255.];

	for y in 0..height {
		for x in 0..width {
			let im_pixel = im.get_pixel(x, y).0;
			let t_pixel = t.get_pixel(x, y).0;
			let t_val = f32::max(t_pixel[0] as f32 / 255., TX);

			let mut res_pixel = [0; 4];
			for ind in 0..3 {
				res_pixel[ind] = ((((im_pixel[ind] as f32 / 255. - a[ind]) / t_val) + a[ind]).clamp(0., 1.) * 255.) as u8;
			}
			res_pixel[3] = im_pixel[3];

			res.put_pixel(x, y, Rgba(res_pixel));
		}
	}

	res
}

fn guided_filter(guidance_img: &DynamicImage, input_img: &DynamicImage, r: u32, epsilon: f64) -> DynamicImage {
	let (width, height) = guidance_img.dimensions();
	let radius = r as i32;

	let guidance_nd = image_to_ndarray(guidance_img);
	let input_nd = image_to_ndarray(input_img);

	let mean_guidance = box_filter(&guidance_nd, radius);
	let mean_input = box_filter(&input_nd, radius);
	let corr_guidance = box_filter(&(guidance_nd.clone() * guidance_nd.clone()), radius);
	let corr_guidance_input = box_filter(&(guidance_nd.clone() * input_nd.clone()), radius);

	let var_guidance = &corr_guidance - &(mean_guidance.clone() * mean_guidance.clone());
	let cov_guidance_input = &corr_guidance_input - &(mean_guidance.clone() * mean_input.clone());

	let a = &cov_guidance_input / &(var_guidance.clone() + epsilon);
	let b = mean_input - &(a.clone() * mean_guidance);

	let mean_a = box_filter(&a, radius);
	let mean_b = box_filter(&b, radius);

	let q = &mean_a * &guidance_nd + mean_b;

	ndarray_to_image(&q, width, height)
}

fn box_filter(img: &Array2<f64>, radius: i32) -> Array2<f64> {
	let (height, width) = img.dim();
	let mut result = Array2::zeros((height, width));
	let mut integral_image: ArrayBase<OwnedRepr<f64>, Dim<[usize; 2]>> = Array2::zeros((height + 1, width + 1));

	// Compute integral image
	for y in 0..height {
		for x in 0..width {
			integral_image[(y + 1, x + 1)] = img[(y, x)] + integral_image[(y, x + 1)] + integral_image[(y + 1, x)] - integral_image[(y, x)];
		}
	}

	for y in 0..height {
		for x in 0..width {
			let y1 = max(0, y as i32 - radius) as usize;
			let y2 = min(height as i32 - 1, y as i32 + radius) as usize;
			let x1 = max(0, x as i32 - radius) as usize;
			let x2 = min(width as i32 - 1, x as i32 + radius) as usize;

			let area = (y2 - y1 + 1) as f64 * (x2 - x1 + 1) as f64;

			result[(y, x)] = (integral_image[(y2 + 1, x2 + 1)] - integral_image[(y1, x2 + 1)] - integral_image[(y2 + 1, x1)] + integral_image[(y1, x1)]) / area;
		}
	}

	result
}

fn image_to_ndarray(img: &DynamicImage) -> Array2<f64> {
	let (width, height) = img.dimensions();
	let mut array = Array2::zeros((height as usize, width as usize));
	for (x, y, pixel) in img.pixels() {
		let luminance = pixel.0[0] as f64 / 255.;
		array[(y as usize, x as usize)] = luminance;
	}
	array
}

fn ndarray_to_image(array: &Array2<f64>, width: u32, height: u32) -> DynamicImage {
	let mut img = DynamicImage::new_rgba8(width, height);
	for ((y, x), &value) in array.indexed_iter() {
		let clamped_value = (value * 255.).clamp(0., 255.) as u8;
		img.put_pixel(x as u32, y as u32, Rgba([clamped_value, clamped_value, clamped_value, 255]));
	}
	img
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn dark_channel_is_the_minimum_over_each_patch() {
		// Varied channels, with patches clipped where they overhang the edges
		let (width, height) = (37, 23);
		let image = DynamicImage::ImageRgba8(RgbaImage::from_fn(width, height, |x, y| {
			let value = ((x * 97 + y * 57) % 251) as u8;
			Rgba([value, value.wrapping_mul(3), value.wrapping_add(40), 200])
		}));

		let dark_channel = compute_dark_channel(&image);

		let half_patch = PATCH_SIZE / 2;
		for y in 0..height {
			for x in 0..width {
				let mut expected = u8::MAX;
				for ny in y.saturating_sub(half_patch)..=(y + half_patch).min(height - 1) {
					for nx in x.saturating_sub(half_patch)..=(x + half_patch).min(width - 1) {
						let pixel = image.get_pixel(nx, ny);
						expected = expected.min(pixel[0].min(pixel[1]).min(pixel[2]));
					}
				}

				let pixel = dark_channel.get_pixel(x, y);
				assert_eq!((pixel[0], pixel[3]), (expected, 200), "at ({x}, {y})");
			}
		}
	}
}
