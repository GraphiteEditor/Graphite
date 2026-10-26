use crate::raster_kernel::{FILTERABLE_FORMAT, RasterKernel, ToGpuParams, Wgsl, raster_kernel_pipeline, run_raster_kernel, run_raster_kernel_with_format};
use crate::{gpu_param_enum, gpu_params};
use bytemuck::{Pod, Zeroable};
use core_types::color::{Alpha, Color, Pixel, RGB};
use core_types::context::Ctx;
use core_types::list::Item;
use core_types::{Interrupt, NodeError};
use glam::DAffine2;
use num_enum::IntoPrimitive;
use raster_types::Image;
use raster_types::{Bitmap, BitmapMut};
use raster_types::{CPU, GPU, Raster, Storage};
use wgpu_executor::WgpuPipelineCache;
use wgpu_executor::shader_runtime::raster_kernel_runtime::RasterKernelInput;

/// Working-buffer pixel for the blur algorithms' `gamma` mode: premultiplied sRGB-gamma `f32` channels.
/// Only used internally so the working buffer's color space is reflected in the type instead of stuffed into `Color` (which is linear-light by invariant).
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Pod, Zeroable)]
struct PremultipliedGammaPixel {
	r: f32,
	g: f32,
	b: f32,
	a: f32,
}

impl Pixel for PremultipliedGammaPixel {}

impl RGB for PremultipliedGammaPixel {
	type ColorChannel = f32;
	fn red(&self) -> f32 {
		self.r
	}
	fn green(&self) -> f32 {
		self.g
	}
	fn blue(&self) -> f32 {
		self.b
	}
}

impl Alpha for PremultipliedGammaPixel {
	type AlphaChannel = f32;
	const TRANSPARENT: Self = Self { r: 0., g: 0., b: 0., a: 0. };
	fn alpha(&self) -> f32 {
		self.a
	}
	fn multiplied_alpha(&self, mult: f32) -> Self {
		Self {
			r: self.r * mult,
			g: self.g * mult,
			b: self.b * mult,
			a: self.a * mult,
		}
	}
}

fn premultiply_gamma(buffer: Image<Color>) -> Image<PremultipliedGammaPixel> {
	Image {
		width: buffer.width,
		height: buffer.height,
		data: buffer
			.data
			.into_iter()
			.map(|px| {
				let [r, g, b, a] = px.to_gamma_srgb_channels();
				PremultipliedGammaPixel { r: r * a, g: g * a, b: b * a, a }
			})
			.collect(),
	}
}

fn unpremultiply_gamma_to_linear(buffer: Image<PremultipliedGammaPixel>) -> Image<Color> {
	Image {
		width: buffer.width,
		height: buffer.height,
		data: buffer
			.data
			.into_iter()
			.map(|px| {
				if px.a > 0. {
					let inv_a = 1. / px.a;
					Color::from_gamma_srgb_channels(px.r * inv_a, px.g * inv_a, px.b * inv_a, px.a)
				} else {
					Color::TRANSPARENT
				}
			})
			.collect(),
	}
}

/// Blurs the image with a Gaussian or box blur kernel filter.
#[node_macro::node(category("Raster: Filter"), icon("NodeBlur"))]
fn blur<T: BlurRaster>(
	_: impl Ctx,
	/// The image to be blurred.
	#[implementations(Raster<CPU>, Raster<GPU>)]
	image_frame: Item<T>,
	/// The radius of the blur kernel.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..100)]
	radius: Item<f64>,
	/// Use a lower-quality box kernel instead of a circular Gaussian kernel. This is faster but produces boxy artifacts.
	box_blur: Item<bool>,
	/// Opt to incorrectly apply the filter with color calculations in gamma space for compatibility with the results from other software.
	gamma: Item<bool>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	let (radius, box_blur, gamma) = (*radius.element(), *box_blur.element(), *gamma.element());
	Ok(T::blur(image_frame, radius, box_blur, gamma, pipeline.element())?)
}

fn blur_cpu(image_frame: Item<Raster<CPU>>, radius: f64, box_blur: bool, gamma: bool) -> Item<Raster<CPU>> {
	let (image, attributes) = image_frame.into_parts();

	// TODO: Scale the radius through the footprint so it acts as the document-space length its Pixels unit promises, with a
	// window that varies continuously rather than snapping to whole source pixels
	let blurred_image = if radius < 0.1 {
		// Minimum blur radius
		image
	} else if box_blur {
		Raster::new_cpu(box_blur_algorithm(image.into_data(), radius, gamma))
	} else {
		Raster::new_cpu(gaussian_blur_algorithm(image.into_data(), radius, gamma))
	};

	Item::from_parts(blurred_image, attributes)
}

/// Applies a median filter to reduce noise while preserving edges.
#[node_macro::node(category("Raster: Filter"), icon("NodeMedianFilter"))]
fn median_filter<T: MedianFilterRaster>(
	_: impl Ctx,
	/// The image to be filtered.
	#[implementations(Raster<CPU>, Raster<GPU>)]
	image_frame: Item<T>,
	/// The radius of the filter kernel. Larger values remove more noise but may blur fine details.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..50)]
	radius: Item<f64>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	let radius = *radius.element();

	let (image, attributes) = image_frame.into_parts();

	// TODO: Scale the radius through the footprint so it acts as the document-space length its Pixels unit promises, with a
	// window that varies continuously rather than snapping to whole source pixels
	let filtered_image = if radius < 0.5 {
		// Minimum filter radius
		image
	} else {
		T::median_filter(image, radius as u32, pipeline.element())?
	};

	Ok(Item::from_parts(filtered_image, attributes))
}

/// A raster the Median Filter node processes, on the CPU or the GPU.
pub trait MedianFilterRaster: Sized {
	fn median_filter(image: Self, radius: u32, pipeline: &WgpuPipelineCache) -> Result<Self, NodeError>;
}

impl MedianFilterRaster for Raster<CPU> {
	fn median_filter(image: Self, radius: u32, _: &WgpuPipelineCache) -> Result<Self, NodeError> {
		Ok(Raster::new_cpu(median_filter_algorithm(image.into_data(), radius)))
	}
}

impl MedianFilterRaster for Raster<GPU> {
	fn median_filter(image: Self, radius: u32, pipeline: &WgpuPipelineCache) -> Result<Self, NodeError> {
		if image.is_empty() {
			return Ok(image);
		}

		let size = image.size();
		let kernel = MedianWindow {
			radius: radius.min(size.max_element()),
		};
		let input = RasterKernelInput {
			raster: &image,
			output_to_texel: DAffine2::IDENTITY,
		};
		run_raster_kernel(&kernel, vec![input], size, pipeline)
	}
}

gpu_params! {
	#[derive(Clone)]
	struct MedianWindow {
		radius: u32,
	}
}

impl RasterKernel for MedianWindow {
	const INPUTS: usize = 1;
	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/median_filter.wgsl"))
	}
}

// 1D gaussian kernel
fn gaussian_kernel(radius: f64) -> Vec<f64> {
	// Given radius, compute the size of the kernel that's approximately three times the radius
	let kernel_radius = (3. * radius).ceil() as usize;
	let kernel_size = 2 * kernel_radius + 1;
	let mut gaussian_kernel: Vec<f64> = vec![0.; kernel_size];

	// Kernel values
	let two_radius_squared = 2. * radius * radius;
	let sum = gaussian_kernel
		.iter_mut()
		.enumerate()
		.map(|(i, value_at_index)| {
			let x = i as f64 - kernel_radius as f64;
			let exponent = -(x * x) / two_radius_squared;
			*value_at_index = exponent.exp();
			*value_at_index
		})
		.sum::<f64>();

	// Normalize
	gaussian_kernel.iter_mut().for_each(|value_at_index| *value_at_index /= sum);

	gaussian_kernel
}

fn gaussian_blur_algorithm(buffer: Image<Color>, radius: f64, gamma: bool) -> Image<Color> {
	let kernel = gaussian_kernel(radius);
	if gamma {
		let working = premultiply_gamma(buffer);
		let blurred = gaussian_separable(working, &kernel, |r, g, b, a| PremultipliedGammaPixel { r, g, b, a });
		unpremultiply_gamma_to_linear(blurred)
	} else {
		let mut working = buffer;
		working.map_pixels(|px| px.to_associated_alpha());
		let mut blurred = gaussian_separable(working, &kernel, Color::from_rgbaf32_unchecked);
		blurred.map_pixels(|px| px.to_unassociated_alpha());
		blurred
	}
}

fn box_blur_algorithm(buffer: Image<Color>, radius: f64, gamma: bool) -> Image<Color> {
	if gamma {
		let working = premultiply_gamma(buffer);
		let blurred = box_separable(working, radius, |r, g, b, a| PremultipliedGammaPixel { r, g, b, a });
		unpremultiply_gamma_to_linear(blurred)
	} else {
		let mut working = buffer;
		working.map_pixels(|px| px.to_associated_alpha());
		let mut blurred = box_separable(working, radius, Color::from_rgbaf32_unchecked);
		blurred.map_pixels(|px| px.to_unassociated_alpha());
		blurred
	}
}

fn gaussian_separable<P, F>(buffer: Image<P>, kernel: &[f64], construct: F) -> Image<P>
where
	P: Pixel + Copy + RGB<ColorChannel = f32> + Alpha<AlphaChannel = f32>,
	F: Fn(f32, f32, f32, f32) -> P,
{
	let (width, height) = buffer.dimensions();
	let half_kernel = kernel.len() / 2;

	let mut x_axis = Image::new(width, height, P::default());
	let mut y_axis = Image::new(width, height, P::default());

	for pass in [false, true] {
		let (max, old_buffer, current_buffer) = match pass {
			false => (width, &buffer, &mut x_axis),
			true => (height, &x_axis, &mut y_axis),
		};
		let pass = pass as usize;

		for y in 0..height {
			for x in 0..width {
				let (mut r_sum, mut g_sum, mut b_sum, mut a_sum, mut weight_sum) = (0., 0., 0., 0., 0.);

				for (i, &weight) in kernel.iter().enumerate() {
					let p = [x, y][pass] as i32 + (i as i32 - half_kernel as i32);

					if p >= 0
						&& p < max as i32 && let Some(px) = old_buffer.get_pixel([p as u32, x][pass], [y, p as u32][pass])
					{
						r_sum += px.r() as f64 * weight;
						g_sum += px.g() as f64 * weight;
						b_sum += px.b() as f64 * weight;
						a_sum += px.a() as f64 * weight;
						weight_sum += weight;
					}
				}

				let (r, g, b, a) = if weight_sum > 0. {
					((r_sum / weight_sum) as f32, (g_sum / weight_sum) as f32, (b_sum / weight_sum) as f32, (a_sum / weight_sum) as f32)
				} else {
					let px = old_buffer.get_pixel(x, y).unwrap();
					(px.r(), px.g(), px.b(), px.a())
				};
				current_buffer.set_pixel(x, y, construct(r, g, b, a));
			}
		}
	}

	y_axis
}

fn box_separable<P, F>(buffer: Image<P>, radius: f64, construct: F) -> Image<P>
where
	P: Pixel + Copy + RGB<ColorChannel = f32> + Alpha<AlphaChannel = f32>,
	F: Fn(f32, f32, f32, f32) -> P,
{
	let (width, height) = buffer.dimensions();
	let mut x_axis = Image::new(width, height, P::default());
	let mut y_axis = Image::new(width, height, P::default());

	for pass in [false, true] {
		let (max, old_buffer, current_buffer) = match pass {
			false => (width, &buffer, &mut x_axis),
			true => (height, &x_axis, &mut y_axis),
		};
		let pass = pass as usize;

		for y in 0..height {
			for x in 0..width {
				let (mut r_sum, mut g_sum, mut b_sum, mut a_sum, mut weight_sum) = (0., 0., 0., 0., 0.);

				let i = [x, y][pass];
				for d in (i as i32 - radius as i32).max(0)..=(i as i32 + radius as i32).min(max as i32 - 1) {
					if let Some(px) = old_buffer.get_pixel([d as u32, x][pass], [y, d as u32][pass]) {
						let weight = 1.;
						r_sum += px.r() as f64 * weight;
						g_sum += px.g() as f64 * weight;
						b_sum += px.b() as f64 * weight;
						a_sum += px.a() as f64 * weight;
						weight_sum += weight;
					}
				}

				let (r, g, b, a) = ((r_sum / weight_sum) as f32, (g_sum / weight_sum) as f32, (b_sum / weight_sum) as f32, (a_sum / weight_sum) as f32);
				current_buffer.set_pixel(x, y, construct(r, g, b, a));
			}
		}
	}

	y_axis
}

/// A raster the Blur node processes, on the CPU or the GPU.
pub trait BlurRaster: Sized {
	fn blur(image: Item<Self>, radius: f64, box_blur: bool, gamma: bool, pipeline: &WgpuPipelineCache) -> Result<Item<Self>, NodeError>;
}

impl BlurRaster for Raster<CPU> {
	fn blur(image: Item<Self>, radius: f64, box_blur: bool, gamma: bool, _: &WgpuPipelineCache) -> Result<Item<Self>, NodeError> {
		Ok(blur_cpu(image, radius, box_blur, gamma))
	}
}

/// The farthest texel from the center a blur pass weighs, beyond which the image is first halved until the kernel fits.
const MAX_BLUR_REACH: i32 = 64;

/// The radius of a blur over texels `scale` image pixels wide, and the farthest texel from the center it weighs, as the CPU kernels size them at a `scale` of 1.
///
/// A box covers its whole texels and a fraction of the texel at each end, which keeps the window as wide as the CPU's whole pixel window once halved,
/// and a Gaussian leaves out the blur that halving and doubling the image add.
fn blur_extent(radius: f64, box_blur: bool, scale: u32) -> (f32, i32) {
	let scale = scale as f64;
	if box_blur {
		let radius = ((radius as i32) as f64 + 0.5) / scale - 0.5;
		(radius as f32, radius.ceil() as i32)
	} else {
		let radius = (radius * radius - (scale * scale - 1.) / 3.).max(0.).sqrt() / scale;
		(radius as f32, (3. * radius).ceil() as i32)
	}
}

fn blur_input(raster: &Raster<GPU>) -> Vec<RasterKernelInput<'_>> {
	vec![RasterKernelInput {
		raster,
		output_to_texel: DAffine2::IDENTITY,
	}]
}

impl BlurRaster for Raster<GPU> {
	/// Each pass sums premultiplied colors into a [`FILTERABLE_FORMAT`] intermediate, and a kernel reaching past [`MAX_BLUR_REACH`] runs on the image halved
	/// until it fits, weighing each halved texel by how much of the image it covers, then is scaled back up bilinearly, approximating the CPU's blur.
	fn blur(image: Item<Self>, radius: f64, box_blur: bool, gamma: bool, pipeline: &WgpuPipelineCache) -> Result<Item<Self>, NodeError> {
		if radius < 0.1 || image.element().is_empty() {
			return Ok(image);
		}
		let (raster, attributes) = image.into_parts();
		let space = if gamma { BlurSpace::Gamma } else { BlurSpace::Linear };
		let size = raster.size();
		let full_size = size.as_ivec2().to_array();

		let mut levels = 0;
		while blur_extent(radius, box_blur, 1 << levels).1 > MAX_BLUR_REACH {
			levels += 1;
		}
		let level_size = |level: u32| size.map(|side| side.div_ceil(1 << level));

		let mut source = raster;
		for level in 1..=levels {
			let kernel = BlurHalve {
				space: if level == 1 { space } else { BlurSpace::Premultiplied },
				scale: 1 << (level - 1),
				full_size,
			};
			source = run_raster_kernel_with_format(&kernel, blur_input(&source), level_size(level), FILTERABLE_FORMAT, pipeline)?;
		}

		let (radius, reach) = blur_extent(radius, box_blur, 1 << levels);
		let pass = BlurPass {
			radius,
			reach,
			box_blur,
			space: if levels == 0 { space } else { BlurSpace::Premultiplied },
			scale: 1 << levels,
			full_size,
		};
		let horizontal = run_raster_kernel_with_format(&BlurHorizontal(pass.clone()), blur_input(&source), level_size(levels), FILTERABLE_FORMAT, pipeline)?;
		if levels == 0 {
			let blurred = run_raster_kernel(&BlurVertical(pass), blur_input(&horizontal), size, pipeline)?;
			return Ok(Item::from_parts(blurred, attributes));
		}
		let vertical = run_raster_kernel_with_format(&BlurVertical(pass), blur_input(&horizontal), level_size(levels), FILTERABLE_FORMAT, pipeline)?;

		let upsample = BlurUpsample { scale: (1 << levels) as f32, space };
		let blurred = run_raster_kernel(&upsample, blur_input(&vertical), size, pipeline)?;
		Ok(Item::from_parts(blurred, attributes))
	}
}

/// The colors a blur pass reads from its input or writes to its output, around the premultiplied colors it sums.
#[derive(Clone, Copy, IntoPrimitive)]
#[repr(u32)]
enum BlurSpace {
	Premultiplied,
	Linear,
	Gamma,
}

gpu_param_enum!(u32: BlurSpace);

gpu_params! {
	/// Averages each 2x2 block of texels `scale` image pixels wide, weighed by how much of the `full_size` image each covers.
	#[derive(Clone)]
	struct BlurHalve {
		space: BlurSpace,
		scale: i32,
		full_size: [i32; 2],
	}
}

impl RasterKernel for BlurHalve {
	const INPUTS: usize = 1;
	fn wgsl() -> Wgsl {
		Wgsl::function(concat!(include_str!("wgsl/blur.wgsl"), include_str!("wgsl/blur_halve.wgsl")))
	}
}

gpu_params! {
	/// One axis of a blur over texels `scale` image pixels wide, each weighed by how much of the `full_size` image it covers.
	#[derive(Clone)]
	struct BlurPass {
		radius: f32,
		reach: i32,
		box_blur: bool,
		space: BlurSpace,
		scale: i32,
		full_size: [i32; 2],
	}
}

struct BlurHorizontal(BlurPass);

impl ToGpuParams for BlurHorizontal {
	type Params = BlurPass;
	fn to_params(&self) -> BlurPass {
		self.0.clone()
	}
}

impl RasterKernel for BlurHorizontal {
	const INPUTS: usize = 1;
	fn wgsl() -> Wgsl {
		Wgsl::function(concat!(include_str!("wgsl/blur.wgsl"), include_str!("wgsl/blur_horizontal.wgsl")))
	}
}

/// Reads its input in pairs of texels sampled bilinearly between them, so the input must be in [`FILTERABLE_FORMAT`].
struct BlurVertical(BlurPass);

impl ToGpuParams for BlurVertical {
	type Params = BlurPass;
	fn to_params(&self) -> BlurPass {
		self.0.clone()
	}
}

impl RasterKernel for BlurVertical {
	const INPUTS: usize = 1;
	const FILTERED: bool = true;
	fn wgsl() -> Wgsl {
		Wgsl::function(concat!(include_str!("wgsl/blur.wgsl"), include_str!("wgsl/blur_vertical.wgsl")))
	}
}

gpu_params! {
	#[derive(Clone)]
	struct BlurUpsample {
		scale: f32,
		space: BlurSpace,
	}
}

impl RasterKernel for BlurUpsample {
	const INPUTS: usize = 1;
	const FILTERED: bool = true;
	fn wgsl() -> Wgsl {
		Wgsl::function(concat!(include_str!("wgsl/blur.wgsl"), include_str!("wgsl/blur_upsample.wgsl")))
	}
}

fn median_filter_algorithm(original_buffer: Image<Color>, radius: u32) -> Image<Color> {
	let (width, height) = original_buffer.dimensions();
	let mut output = Image::new(width, height, Color::TRANSPARENT);

	// Pre-allocate and reuse buffers outside the loops to avoid repeated allocations.
	let window_capacity = ((2 * radius + 1).pow(2)) as usize;
	let mut r_vals: Vec<f32> = Vec::with_capacity(window_capacity);
	let mut g_vals: Vec<f32> = Vec::with_capacity(window_capacity);
	let mut b_vals: Vec<f32> = Vec::with_capacity(window_capacity);
	let mut a_vals: Vec<f32> = Vec::with_capacity(window_capacity);

	for y in 0..height {
		for x in 0..width {
			r_vals.clear();
			g_vals.clear();
			b_vals.clear();
			a_vals.clear();

			// Use saturating_add to avoid potential overflow in extreme cases
			let y_max = y.saturating_add(radius).min(height - 1);
			let x_max = x.saturating_add(radius).min(width - 1);

			for ny in y.saturating_sub(radius)..=y_max {
				for nx in x.saturating_sub(radius)..=x_max {
					if let Some(px) = original_buffer.get_pixel(nx, ny) {
						r_vals.push(px.r());
						g_vals.push(px.g());
						b_vals.push(px.b());
						a_vals.push(px.a());
					}
				}
			}

			let r = median_quickselect(&mut r_vals);
			let g = median_quickselect(&mut g_vals);
			let b = median_quickselect(&mut b_vals);
			let a = median_quickselect(&mut a_vals);

			output.set_pixel(x, y, Color::from_rgbaf32_unchecked(r, g, b, a));
		}
	}

	output
}
/// Finds the median of a slice using quickselect for O(n) average case performance.
/// This is more efficient than sorting the entire slice which would be O(n log n).
fn median_quickselect(values: &mut [f32]) -> f32 {
	let mid: usize = values.len() / 2;
	// nth_unstable is like quickselect: average O(n)
	// Use total_cmp for safe NaN handling instead of partial_cmp().unwrap()
	*values.select_nth_unstable_by(mid, |a, b| a.total_cmp(b)).1
}
