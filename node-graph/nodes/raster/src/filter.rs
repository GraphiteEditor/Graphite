use core_types::color::Color;
use core_types::context::Ctx;
use core_types::registry::types::PixelLength;
use core_types::table::Table;
use raster_types::Image;
use raster_types::{Bitmap, BitmapMut};
use raster_types::{CPU, Raster};

/// Blurs the image with a Gaussian or box blur kernel filter.
#[node_macro::node(category("Raster: Filter"))]
async fn blur(
	_: impl Ctx,
	/// The image to be blurred.
	image_frame: Table<Raster<CPU>>,
	/// The radius of the blur kernel.
	#[range((0., 100.))]
	#[hard_min(0.)]
	radius: PixelLength,
	/// Use a lower-quality box kernel instead of a circular Gaussian kernel. This is faster but produces boxy artifacts.
	box_blur: bool,
	/// Opt to incorrectly apply the filter with color calculations in gamma space for compatibility with the results from other software.
	gamma: bool,
) -> Table<Raster<CPU>> {
	image_frame
		.into_iter()
		.map(|mut row| {
			let image = row.element.clone();

			// Run blur algorithm
			let blurred_image = if radius < 0.1 {
				// Minimum blur radius
				image.clone()
			} else if box_blur {
				Raster::new_cpu(box_blur_algorithm(image.into_data(), radius, gamma))
			} else {
				Raster::new_cpu(gaussian_blur_algorithm(image.into_data(), radius, gamma))
			};

			row.element = blurred_image;
			row
		})
		.collect()
}

/// Applies a median filter to reduce noise while preserving edges.
#[node_macro::node(category("Raster: Filter"))]
async fn median_filter(
	_: impl Ctx,
	/// The image to be filtered.
	image_frame: Table<Raster<CPU>>,
	/// The radius of the filter kernel. Larger values remove more noise but may blur fine details.
	#[range((0., 50.))]
	#[hard_min(0.)]
	radius: PixelLength,
) -> Table<Raster<CPU>> {
	image_frame
		.into_iter()
		.map(|mut row| {
			let image = row.element.clone();

			// Apply median filter
			let filtered_image = if radius < 0.5 {
				// Minimum filter radius
				image.clone()
			} else {
				Raster::new_cpu(median_filter_algorithm(image.into_data(), radius as u32))
			};

			row.element = filtered_image;
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

fn gaussian_blur_algorithm(mut original_buffer: Image<Color>, radius: f64, gamma: bool) -> Image<Color> {
	if gamma {
		original_buffer.map_pixels(|px| px.to_gamma_srgb().to_associated_alpha(px.a()));
	} else {
		original_buffer.map_pixels(|px| px.to_associated_alpha(px.a()));
	}

	let (width, height) = original_buffer.dimensions();

	// Create 1D gaussian kernel
	let kernel = gaussian_kernel(radius);
	let half_kernel = kernel.len() / 2;

	// Intermediate buffer for horizontal and vertical passes
	let mut x_axis = Image::new(width, height, Color::TRANSPARENT);
	let mut y_axis = Image::new(width, height, Color::TRANSPARENT);

	for pass in [false, true] {
		let (max, old_buffer, current_buffer) = match pass {
			false => (width, &original_buffer, &mut x_axis),
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

				// Normalize
				let (r, g, b, a) = if weight_sum > 0. {
					((r_sum / weight_sum) as f32, (g_sum / weight_sum) as f32, (b_sum / weight_sum) as f32, (a_sum / weight_sum) as f32)
				} else {
					let px = old_buffer.get_pixel(x, y).unwrap();
					(px.r(), px.g(), px.b(), px.a())
				};
				current_buffer.set_pixel(x, y, Color::from_rgbaf32_unchecked(r, g, b, a));
			}
		}
	}

	if gamma {
		y_axis.map_pixels(|px| px.to_linear_srgb().to_unassociated_alpha());
	} else {
		y_axis.map_pixels(|px| px.to_unassociated_alpha());
	}

	y_axis
}

fn box_blur_algorithm(mut original_buffer: Image<Color>, radius: f64, gamma: bool) -> Image<Color> {
	if gamma {
		original_buffer.map_pixels(|px| px.to_gamma_srgb().to_associated_alpha(px.a()));
	} else {
		original_buffer.map_pixels(|px| px.to_associated_alpha(px.a()));
	}

	let (width, height) = original_buffer.dimensions();
	let mut x_axis = Image::new(width, height, Color::TRANSPARENT);
	let mut y_axis = Image::new(width, height, Color::TRANSPARENT);

	for pass in [false, true] {
		let (max, old_buffer, current_buffer) = match pass {
			false => (width, &original_buffer, &mut x_axis),
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
				current_buffer.set_pixel(x, y, Color::from_rgbaf32_unchecked(r, g, b, a));
			}
		}
	}

	if gamma {
		y_axis.map_pixels(|px| px.to_linear_srgb().to_unassociated_alpha());
	} else {
		y_axis.map_pixels(|px| px.to_unassociated_alpha());
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
