use graph_craft::proto::types::PixelLength;
use graphene_core::raster::Channel;
use graphene_core::raster::image::{Image, ImageFrameTable};
use graphene_core::transform::{Transform, TransformMut};
use graphene_core::{Color, Ctx};
use image::{DynamicImage, ImageBuffer, Rgba};

#[node_macro::node(category("Raster"))]
async fn blur(_: impl Ctx, image_frame: ImageFrameTable<Color>, #[range((0., 100.))] radius: PixelLength, gaussian_blur: bool, nonlinear: bool) -> ImageFrameTable<Color> {
	let image_frame_transform = image_frame.transform();
	let image_frame_alpha_blending = image_frame.one_instance().alpha_blending;

	let image = image_frame.one_instance().instance;

	// Prepare the image data for processing
	let image_data = bytemuck::cast_vec(image.data.clone());
	let mut image_buffer = image::Rgba32FImage::from_raw(image.width, image.height, image_data).expect("Failed to convert internal image format into image-rs data type.");

	// Run blur algorithm
	let blurred_image = blur_helper(&mut image_buffer, radius, gaussian_blur, nonlinear);

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

// Helpers to convert image buffer to linear/nonlinear color spaces in-place
fn to_linear_helper(image_buffer: &mut ImageBuffer<Rgba<f32>, Vec<f32>>) {
	for pixel in image_buffer.pixels_mut() {
		// Leave alpha channels
		let channels = pixel.0;
		pixel.0[0] = channels[0].to_linear();
		pixel.0[1] = channels[1].to_linear();
		pixel.0[2] = channels[2].to_linear();
	}
}
fn from_linear_helper(image_buffer: &mut ImageBuffer<Rgba<f32>, Vec<f32>>) {
	for pixel in image_buffer.pixels_mut() {
		let channels = pixel.0;
		pixel.0[0] = Channel::from_linear(channels[0]);
		pixel.0[1] = Channel::from_linear(channels[1]);
		pixel.0[2] = Channel::from_linear(channels[2]);
	}
}

fn blur_helper(image_buffer: &mut ImageBuffer<Rgba<f32>, Vec<f32>>, radius: f64, gaussian: bool, nonlinear: bool) -> DynamicImage {
	// For small radius, image would not change much -> just return original image
	if radius < 1 as f64 {
		return image_buffer.clone().into();
	} else {
		// Convert to linear color space by default
		if !nonlinear {
			to_linear_helper(image_buffer);
		}
		// Run the gaussian blur algorithm, if user wants
		if gaussian {
			return gaussian_blur(image_buffer.clone(), radius, nonlinear);
		}
		// Else, run box blur
		else {
			return box_blur(image_buffer.clone(), radius, nonlinear);
		}
	}
}

fn gaussian_blur(original_buffer: ImageBuffer<Rgba<f32>, Vec<f32>>, radius: f64, nonlinear: bool) -> DynamicImage {
	let (width, height) = original_buffer.dimensions();

	// Create 1D gaussian kernel
	let kernel = create_gaussian_kernel(radius);
	let half_kernel = kernel.len() / 2;

	// Intermediate buffer for horizontal pass
	let mut x_axis = ImageBuffer::<Rgba<f32>, Vec<f32>>::new(width, height);
	// Blur along x-axis
	for y in 0..height {
		for x in 0..width {
			let mut r_sum = 0.0;
			let mut g_sum = 0.0;
			let mut b_sum = 0.0;
			let mut a_sum = 0.0;
			let mut weight_sum = 0.0;

			for (i, &weight) in kernel.iter().enumerate() {
				let kx = i as i32 - half_kernel as i32;
				let px = x as i32 + kx;

				if px >= 0 && px < width as i32 {
					let pixel = original_buffer.get_pixel(px as u32, y);

					r_sum += pixel[0] as f64 * weight;
					g_sum += pixel[1] as f64 * weight;
					b_sum += pixel[2] as f64 * weight;
					a_sum += pixel[3] as f64 * weight;
					weight_sum += weight;
				}
			}

			// Normalize
			if weight_sum > 0.0 {
				let r = (r_sum / weight_sum) as f32;
				let g = (g_sum / weight_sum) as f32;
				let b = (b_sum / weight_sum) as f32;
				let a = (a_sum / weight_sum) as f32;

				x_axis.put_pixel(x, y, Rgba([r, g, b, a]));
			} else {
				x_axis.put_pixel(x, y, *original_buffer.get_pixel(x, y));
			}
		}
	}

	// Intermediate buffer for vertical pass
	let mut y_axis = ImageBuffer::<Rgba<f32>, Vec<f32>>::new(width, height);
	// Blur along y-axis
	for y in 0..height {
		for x in 0..width {
			let mut r_sum = 0.0;
			let mut g_sum = 0.0;
			let mut b_sum = 0.0;
			let mut a_sum: f64 = 0.0;
			let mut weight_sum = 0.0;

			for (i, &weight) in kernel.iter().enumerate() {
				let ky = i as i32 - half_kernel as i32;
				let py = y as i32 + ky;

				if py >= 0 && py < height as i32 {
					let pixel = x_axis.get_pixel(x, py as u32);

					r_sum += pixel[0] as f64 * weight;
					g_sum += pixel[1] as f64 * weight;
					b_sum += pixel[2] as f64 * weight;
					a_sum += pixel[3] as f64 * weight;
					weight_sum += weight;
				}
			}

			if weight_sum > 0.0 {
				let r = (r_sum / weight_sum) as f32;
				let g = (g_sum / weight_sum) as f32;
				let b = (b_sum / weight_sum) as f32;
				let a = (a_sum / weight_sum) as f32;

				y_axis.put_pixel(x, y, Rgba([r, g, b, a]));
			} else {
				y_axis.put_pixel(x, y, *x_axis.get_pixel(x, y));
			}
		}
	}

	// Convert linear back to nonlinear if converted initially
	if !nonlinear {
		from_linear_helper(&mut y_axis);
	}
	DynamicImage::ImageRgba32F(y_axis)
}

// 1D gaussian kernel
fn create_gaussian_kernel(radius: f64) -> Vec<f64> {
	// Given radius, compute size of kernel -> 3*radius (approx.)
	let kernel_radius = (3.0 * radius).ceil() as usize;
	let kernel_size = 2 * kernel_radius + 1;
	let mut gaussian_kernel: Vec<f64> = vec![0.0; kernel_size];

	// Kernel values
	let two_radius_squared = 2.0 * radius * radius;
	let mut sum = 0.0;
	for i in 0..kernel_size {
		let x: f64 = i as f64 - kernel_radius as f64;
		let exponent = -(x * x) / two_radius_squared;
		gaussian_kernel[i] = exponent.exp();
		sum += gaussian_kernel[i];
	}

	// Normalize
	for i in 0..kernel_size {
		gaussian_kernel[i] /= sum;
	}

	gaussian_kernel
}

fn box_blur(original_buffer: ImageBuffer<Rgba<f32>, Vec<f32>>, radius: f64, nonlinear: bool) -> DynamicImage {
	let (width, height) = original_buffer.dimensions();
	let mut x_axis = ImageBuffer::new(width, height);
	let mut blurred_image = ImageBuffer::new(width, height);

	// Blur along x-axis
	for y in 0..height {
		for x in 0..width {
			let mut r_sum = 0.0;
			let mut g_sum = 0.0;
			let mut b_sum = 0.0;
			let mut a_sum = 0.0;
			let mut weight_sum = 0.0;

			for dx in (x as i32 - radius as i32).max(0)..=(x as i32 + radius as i32).min(width as i32 - 1) {
				let pixel = original_buffer.get_pixel(dx as u32, y);
				let weight = 1.0;

				r_sum += pixel[0] as f64 * weight;
				g_sum += pixel[1] as f64 * weight;
				b_sum += pixel[2] as f64 * weight;
				a_sum += pixel[3] as f64 * weight;
				weight_sum += weight;
			}

			x_axis.put_pixel(
				x,
				y,
				Rgba([(r_sum / weight_sum) as f32, (g_sum / weight_sum) as f32, (b_sum / weight_sum) as f32, (a_sum / weight_sum) as f32]),
			);
		}
	}

	// Blur along y-axis
	for y in 0..height {
		for x in 0..width {
			let mut r_sum = 0.0;
			let mut g_sum = 0.0;
			let mut b_sum = 0.0;
			let mut a_sum = 0.0;
			let mut weight_sum = 0.0;

			for dy in (y as i32 - radius as i32).max(0)..=(y as i32 + radius as i32).min(height as i32 - 1) {
				let pixel = x_axis.get_pixel(x, dy as u32);
				let weight = 1.0;

				r_sum += pixel[0] as f64 * weight;
				g_sum += pixel[1] as f64 * weight;
				b_sum += pixel[2] as f64 * weight;
				a_sum += pixel[3] as f64 * weight;
				weight_sum += weight;
			}

			blurred_image.put_pixel(
				x,
				y,
				Rgba([(r_sum / weight_sum) as f32, (g_sum / weight_sum) as f32, (b_sum / weight_sum) as f32, (a_sum / weight_sum) as f32]),
			);
		}
	}

	// Convert linear back to nonlinear if converted initially
	if !nonlinear {
		from_linear_helper(&mut blurred_image);
	}
	DynamicImage::ImageRgba32F(blurred_image)
}
