use graph_craft::proto::types::PixelLength;
use graphene_core::raster::image::{Image, ImageFrameTable};
use graphene_core::raster::{Bitmap, BitmapMut};
use graphene_core::transform::{Transform, TransformMut};
use graphene_core::{Color, Ctx};

enum ConvertFunction {
	ToLinear,
	ToGamma,
}

#[node_macro::node(category("Raster"))]
async fn blur(_: impl Ctx, image_frame: ImageFrameTable<Color>, #[range((0., 100.))] radius: PixelLength, box_blur: bool, gamma: bool) -> ImageFrameTable<Color> {
	let image_frame_transform = image_frame.transform();
	let image_frame_alpha_blending = image_frame.one_instance().alpha_blending;

	let image = image_frame.one_instance().instance.clone();

	// Run blur algorithm
	let blurred_image = if radius < 0.1 {
		// Minimum blur radius
		image.clone()
	} else if box_blur {
		box_blur_algorithm(image, radius, gamma)
	} else {
		gaussian_blur_algorithm(image, radius, gamma)
	};

	let mut result = ImageFrameTable::new(blurred_image);
	*result.transform_mut() = image_frame_transform;
	*result.one_instance_mut().alpha_blending = *image_frame_alpha_blending;

	result
}

// Helper to convert image buffer to linear/nonlinear color spaces in-place
fn convert_color_space(image: &mut Image<Color>, convert: ConvertFunction) {
	for pixel in image.data.iter_mut() {
		*pixel = match convert {
			ConvertFunction::ToLinear => pixel.to_linear_srgb(),
			ConvertFunction::ToGamma => pixel.to_gamma_srgb(),
		};
	}
}

// 1D gaussian kernel
fn gaussian_kernel(radius: f64) -> Vec<f64> {
	// Given radius, compute size of kernel -> 3*radius (approx.)
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
		convert_color_space(&mut original_buffer, ConvertFunction::ToGamma)
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
		convert_color_space(&mut y_axis, ConvertFunction::ToLinear);
	}

	y_axis
}

fn box_blur_algorithm(mut original_buffer: Image<Color>, radius: f64, gamma: bool) -> Image<Color> {
	if gamma {
		convert_color_space(&mut original_buffer, ConvertFunction::ToGamma)
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
		convert_color_space(&mut y_axis, ConvertFunction::ToLinear);
	}

	y_axis
}
