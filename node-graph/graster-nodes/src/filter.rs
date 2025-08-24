use graphene_core::color::Color;
use graphene_core::context::Ctx;
use graphene_core::raster::image::Image;
use graphene_core::raster::{Bitmap, BitmapMut};
use graphene_core::raster_types::{CPU, Raster};
use graphene_core::registry::types::PixelLength;
use graphene_core::table::Table;

/// Blurs the image with a Gaussian or blur kernel filter.
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

					if p >= 0 && p < max as i32 {
						if let Some(px) = old_buffer.get_pixel([p as u32, x][pass], [y, p as u32][pass]) {
							r_sum += px.r() as f64 * weight;
							g_sum += px.g() as f64 * weight;
							b_sum += px.b() as f64 * weight;
							a_sum += px.a() as f64 * weight;
							weight_sum += weight;
						}
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
