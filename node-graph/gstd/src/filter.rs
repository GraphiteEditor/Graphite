use graph_craft::proto::types::PixelLength;
use graphene_core::raster::Channel;
use graphene_core::raster::image::{Image, ImageFrameTable};
use graphene_core::transform::{Transform, TransformMut};
use graphene_core::{Color, Ctx};
use image::{DynamicImage, ImageBuffer, Rgba};

enum ConvertFunction {
	ToLinear,
	ToGamma,
}

#[node_macro::node(category("Raster"))]
async fn blur(_: impl Ctx, image_frame: ImageFrameTable<Color>, #[range((0., 100.))] radius: PixelLength, box_blur: bool, gamma: bool) -> ImageFrameTable<Color> {
	let image_frame_transform = image_frame.transform();
	let image_frame_alpha_blending = image_frame.one_instance().alpha_blending;

	let image = image_frame.one_instance().instance;

	// Prepare the image data for processing
	let image_data = bytemuck::cast_vec(image.data.clone());
	let image_buffer = image::Rgba32FImage::from_raw(image.width, image.height, image_data).expect("Failed to convert internal image format into image-rs data type.");

	// Run blur algorithm
	let blurred_image = if radius < 0.1 {
		// Minimum blur radius
		image_buffer.into()
	} else if box_blur {
		gaussian_blur_algorithm(image_buffer, radius, gamma)
	} else {
		box_blur_algorithm(image_buffer, radius, gamma)
	};

	// Prepare the image data for returning
	let buffer = blurred_image.to_rgba32f().into_raw();
	let color_vec = bytemuck::cast_vec(buffer);
	let processed_image = Image {
		width: image.width,
		height: image.height,
		data: color_vec,
		base64_string: None,
	};

	let mut result = ImageFrameTable::new(processed_image);
	*result.transform_mut() = image_frame_transform;
	*result.one_instance_mut().alpha_blending = *image_frame_alpha_blending;

	result
}

// Helper to convert image buffer to linear/nonlinear color spaces in-place
fn convert_color_space(image_buffer: &mut ImageBuffer<Rgba<f32>, Vec<f32>>, convert: ConvertFunction) {
	for pixel in image_buffer.pixels_mut() {
		// Leave alpha channels
		let channels = pixel.0;

		match convert {
			ConvertFunction::ToLinear => {
				pixel.0[0] = channels[0].to_linear();
				pixel.0[1] = channels[1].to_linear();
				pixel.0[2] = channels[2].to_linear();
			}
			ConvertFunction::ToGamma => {
				pixel.0[0] = Channel::from_linear(channels[0]);
				pixel.0[1] = Channel::from_linear(channels[1]);
				pixel.0[2] = Channel::from_linear(channels[2]);
			}
		}
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

fn gaussian_blur_algorithm(mut original_buffer: ImageBuffer<Rgba<f32>, Vec<f32>>, radius: f64, gamma: bool) -> DynamicImage {
	if !gamma {
		convert_color_space(&mut original_buffer, ConvertFunction::ToLinear)
	}

	let (width, height) = original_buffer.dimensions();

	// Create 1D gaussian kernel
	let kernel = gaussian_kernel(radius);
	let half_kernel = kernel.len() / 2;

	// Intermediate buffer for horizontal and vertical passes
	let mut x_axis = ImageBuffer::<Rgba<f32>, Vec<f32>>::new(width, height);
	let mut y_axis = ImageBuffer::<Rgba<f32>, Vec<f32>>::new(width, height);

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
						let pixel = old_buffer.get_pixel([p as u32, x][pass], [y, p as u32][pass]);

						r_sum += pixel[0] as f64 * weight;
						g_sum += pixel[1] as f64 * weight;
						b_sum += pixel[2] as f64 * weight;
						a_sum += pixel[3] as f64 * weight;
						weight_sum += weight;
					}
				}

				// Normalize
				let rgba = (r_sum, b_sum, g_sum, a_sum);
				if weight_sum > 0. {
					let r = (rgba.0 / weight_sum) as f32;
					let g = (rgba.1 / weight_sum) as f32;
					let b = (rgba.2 / weight_sum) as f32;
					let a = (rgba.3 / weight_sum) as f32;

					current_buffer.put_pixel(x, y, Rgba([r, g, b, a]));
				} else {
					current_buffer.put_pixel(x, y, *old_buffer.get_pixel(x, y));
				}
			}
		}
	}

	if !gamma {
		convert_color_space(&mut y_axis, ConvertFunction::ToGamma);
	}

	DynamicImage::ImageRgba32F(y_axis)
}

fn box_blur_algorithm(mut original_buffer: ImageBuffer<Rgba<f32>, Vec<f32>>, radius: f64, gamma: bool) -> DynamicImage {
	if !gamma {
		convert_color_space(&mut original_buffer, ConvertFunction::ToLinear)
	}

	let (width, height) = original_buffer.dimensions();
	let mut x_axis = ImageBuffer::new(width, height);
	let mut y_axis = ImageBuffer::new(width, height);

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
					let pixel = old_buffer.get_pixel([d as u32, x][pass], [y, d as u32][pass]);
					let weight = 1.;

					r_sum += pixel[0] as f64 * weight;
					g_sum += pixel[1] as f64 * weight;
					b_sum += pixel[2] as f64 * weight;
					a_sum += pixel[3] as f64 * weight;
					weight_sum += weight;
				}

				let pixel = Rgba([(r_sum / weight_sum) as f32, (g_sum / weight_sum) as f32, (b_sum / weight_sum) as f32, (a_sum / weight_sum) as f32]);
				current_buffer.put_pixel(x, y, pixel);
			}
		}
	}

	if !gamma {
		convert_color_space(&mut y_axis, ConvertFunction::ToGamma);
	}

	DynamicImage::ImageRgba32F(y_axis)
}
