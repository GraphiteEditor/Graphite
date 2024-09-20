use graph_craft::proto::types::Percentage;
use graphene_core::raster::{Image, ImageFrame};
use graphene_core::transform::Footprint;
use graphene_core::Color;
use image::{DynamicImage, GenericImage, GenericImageView, GrayImage, ImageBuffer, Luma, Rgba, RgbaImage};
use ndarray::{Array2, ArrayBase, Dim, OwnedRepr};
use std::cmp::{max, min};

#[node_macro::node(category("Raster: Filter"))]
async fn dehaze_image_node<F: 'n + Send + Sync>(
	#[implementations((), Footprint)] footprint: F,
	#[implementations(((), ImageFrame<Color>),(Footprint, ImageFrame<Color>))] image_frame: impl Node<F, Output = ImageFrame<Color>>,
	#[min(-100.)]
	#[default(0.)]
	strength: Percentage,
) -> ImageFrame<Color> {
	let image_frame = image_frame.eval(footprint).await;
	let image = image_frame.image;
	let data = bytemuck::cast_vec(image.data);
	let image_buffer = image::Rgba32FImage::from_raw(image.width, image.height, data).expect("Failed to convert internal ImageFrame into image-rs data type.");
	let dynamic_image: image::DynamicImage = image_buffer.into();
	let dehazed_dynamic_image = dehaze(dynamic_image, strength);

	let buffer = dehazed_dynamic_image.to_rgba32f();
	let buffer = buffer.into_raw();
	let vec = bytemuck::cast_vec(buffer);

	let dehazed_image = Image {
		width: image.width,
		height: image.height,
		data: vec,
		base64_string: None,
	};

	ImageFrame {
		image: dehazed_image,
		transform: image_frame.transform,
		alpha_blending: image_frame.alpha_blending,
	}
}

// There is no real point in modifying these values because they do not change the final result all that much,
// The authors of the paper recommended using these values to get a reasonable balance of performance and quality
// The only parameter that needs to be modified is omega to control the level of dehaze which is done in the dehaze() function
const PATCH_SIZE: u32 = 15;
const TOP_PERCENT: f64 = 0.001;
const RADIUS: u32 = 60;
const EPSILON: f64 = 0.0001;
const TX: f32 = 0.1;

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

	let mut eroded_channel = RgbaImage::new(width, height);

	for y in 0..height {
		for x in 0..width {
			let mut local_min = u8::MAX;

			for dy in 0..PATCH_SIZE {
				for dx in 0..PATCH_SIZE {
					let nx = x as i32 + dx as i32 - half_patch as i32;
					let ny = y as i32 + dy as i32 - half_patch as i32;

					if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
						let intensity = dark_channel.get_pixel(nx as u32, ny as u32)[0];
						if intensity < local_min {
							local_min = intensity;
						}
					}
				}
			}
			let alpha = image.get_pixel(x, y)[3];
			eroded_channel.put_pixel(x, y, Rgba([local_min, local_min, local_min, alpha]));
		}
	}

	DynamicImage::ImageRgba8(eroded_channel)
}

fn estimate_atmospheric_light(hazy: &DynamicImage, dark_channel: &DynamicImage) -> Rgba<u8> {
	let (width, height) = hazy.dimensions();
	let dark = dark_channel.to_luma_alpha8();
	let total_pixels = (width * height) as usize;
	let num_pixels = ((TOP_PERCENT / 100.0) * total_pixels as f64).ceil() as usize;

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

	let mut atm_sum = [0.0, 0.0, 0.0];
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
			let min_intensity = dark_channel.get_pixel(x, y).0[0] as f32 / 255.0;
			let transmission_value = 1.0 - omega * min_intensity as f64;
			let alpha = image.get_pixel(x, y)[3];
			transmission_map.put_pixel(
				x,
				y,
				Rgba([(transmission_value * 255.0) as u8, (transmission_value * 255.0) as u8, (transmission_value * 255.0) as u8, alpha]),
			);
		}
	}

	DynamicImage::ImageRgba8(transmission_map)
}

fn refine_transmission_map(img: &DynamicImage, transmission_map: &DynamicImage) -> DynamicImage {
	let gray_image = img.to_luma8();

	let normalized_gray_image: GrayImage = ImageBuffer::from_fn(gray_image.width(), gray_image.height(), |x, y| {
		let pixel = gray_image.get_pixel(x, y);
		let normalized_value = (pixel[0] as f64 / 255.0) * 255.0;
		Luma([normalized_value as u8])
	});

	let normalized_gray_image = DynamicImage::ImageLuma8(normalized_gray_image);

	guided_filter(&normalized_gray_image, transmission_map, RADIUS, EPSILON)
}

fn recover(im: &DynamicImage, t: &DynamicImage, a: Rgba<u8>) -> DynamicImage {
	let (width, height) = im.dimensions();
	let mut res = DynamicImage::new_rgba8(width, height);

	let a = [a[0] as f32 / 255.0, a[1] as f32 / 255.0, a[2] as f32 / 255.0];

	for y in 0..height {
		for x in 0..width {
			let im_pixel = im.get_pixel(x, y).0;
			let t_pixel = t.get_pixel(x, y).0;
			let t_val = f32::max(t_pixel[0] as f32 / 255.0, TX);

			let mut res_pixel = [0; 4];
			for ind in 0..3 {
				res_pixel[ind] = ((((im_pixel[ind] as f32 / 255.0 - a[ind]) / t_val) + a[ind]).clamp(0.0, 1.0) * 255.0) as u8;
			}
			res_pixel[3] = im_pixel[3];

			res.put_pixel(x, y, Rgba(res_pixel));
		}
	}

	res
}

fn dehaze(img: DynamicImage, percent_dehaze: f64) -> DynamicImage {
	let dark_channel = compute_dark_channel(&img);

	let atmospheric_light = estimate_atmospheric_light(&img, &dark_channel);

	let scale = (percent_dehaze - 1.0) / 100.0;
	let transmission_map = estimate_transmission_map(&img, &dark_channel, scale);

	let refined_transmission_map = refine_transmission_map(&img, &transmission_map);

	recover(&img, &refined_transmission_map, atmospheric_light)
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
		let luminance = pixel.0[0] as f64 / 255.0;
		array[(y as usize, x as usize)] = luminance;
	}
	array
}

fn ndarray_to_image(array: &Array2<f64>, width: u32, height: u32) -> DynamicImage {
	let mut img = DynamicImage::new_rgba8(width, height);
	for ((y, x), &value) in array.indexed_iter() {
		let clamped_value = (value * 255.0).clamp(0.0, 255.0) as u8;
		img.put_pixel(x as u32, y as u32, Rgba([clamped_value, clamped_value, clamped_value, 255]));
	}
	img
}
