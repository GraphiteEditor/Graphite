use bytemuck::{Pod, Zeroable};
use core_types::color::{Alpha, Color, Pixel, RGB};
use core_types::context::Ctx;
use core_types::list::List;
use core_types::registry::types::PixelLength;
use raster_types::Image;
use raster_types::{Bitmap, BitmapMut};
use raster_types::{CPU, Raster};

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
		base64_string: None,
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
		base64_string: None,
	}
}

/// Blurs the image with a Gaussian or box blur kernel filter.
#[node_macro::node(category("Raster: Filter"))]
async fn blur(
	_: impl Ctx,
	/// The image to be blurred.
	image_frame: List<Raster<CPU>>,
	/// The radius of the blur kernel.
	#[range]
	#[hard(0..)]
	#[soft(..100)]
	radius: PixelLength,
	/// Use a lower-quality box kernel instead of a circular Gaussian kernel. This is faster but produces boxy artifacts.
	box_blur: bool,
	/// Opt to incorrectly apply the filter with color calculations in gamma space for compatibility with the results from other software.
	gamma: bool,
) -> List<Raster<CPU>> {
	image_frame
		.into_iter()
		.map(|mut row| {
			let image = row.element().clone();

			// Run blur algorithm
			let blurred_image = if radius < 0.1 {
				// Minimum blur radius
				image.clone()
			} else if box_blur {
				Raster::new_cpu(box_blur_algorithm(image.into_data(), radius, gamma))
			} else {
				Raster::new_cpu(gaussian_blur_algorithm(image.into_data(), radius, gamma))
			};

			*row.element_mut() = blurred_image;
			row
		})
		.collect()
}

/// Applies a median filter to reduce noise while preserving edges.
#[node_macro::node(category("Raster: Filter"))]
async fn median_filter(
	_: impl Ctx,
	/// The image to be filtered.
	image_frame: List<Raster<CPU>>,
	/// The radius of the filter kernel. Larger values remove more noise but may blur fine details.
	#[range]
	#[hard(0..)]
	#[soft(..50)]
	radius: PixelLength,
) -> List<Raster<CPU>> {
	image_frame
		.into_iter()
		.map(|mut row| {
			let image = row.element().clone();

			// Apply median filter
			let filtered_image = if radius < 0.5 {
				// Minimum filter radius
				image.clone()
			} else {
				Raster::new_cpu(median_filter_algorithm(image.into_data(), radius as u32))
			};

			*row.element_mut() = filtered_image;
			row
		})
		.collect()
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
		working.map_pixels(|px| px.apply_opacity(px.a()));
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
		working.map_pixels(|px| px.apply_opacity(px.a()));
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
