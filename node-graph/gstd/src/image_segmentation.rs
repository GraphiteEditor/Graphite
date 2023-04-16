use std::collections::hash_map::HashMap;

use graphene_core::raster::{Color, ImageFrame};
use graphene_core::Node;

fn apply_mask(image_frame: &mut ImageFrame<Color>, x: usize, y: usize, multiplier: u8) {
	let color = &mut image_frame.image.data[y * image_frame.image.width as usize + x];
	let color8 = color.to_rgba8_srgb();
	*color = Color::from_rgba8_srgb(color8[0] * multiplier, color8[1] * multiplier, color8[2] * multiplier, color8[3] * multiplier);
}

pub struct Mask {
	pub data: Vec<u8>,
	pub width: usize,
	pub height: usize,
}

impl Mask {
	fn sample(&self, u: f32, v: f32) -> u8 {
		let x = (u * (self.width as f32)) as usize;
		let y = (v * (self.height as f32)) as usize;

		self.data[y * self.width + x]
	}
}

fn image_segmentation(input_image: &ImageFrame<Color>, input_mask: &Mask) -> Vec<ImageFrame<Color>> {
	const NUM_LABELS: usize = u8::MAX as usize;
	let mut result = Vec::<ImageFrame<Color>>::with_capacity(NUM_LABELS);
	let mut current_label = 0_usize;
	let mut label_appeared = [false; NUM_LABELS + 1];
	let mut max_label = 0_usize;

	if input_mask.data.is_empty() {
		warn!("The mask for the segmentation node is empty!");
		return vec![ImageFrame::empty()];
	}

	result.push(input_image.clone());
	let result_last = result.last_mut().unwrap();

	for y in 0..input_image.image.height {
		let v = (y as f32) / (input_image.image.height as f32);
		for x in 0..input_image.image.width {
			let u = (x as f32) / (input_image.image.width as f32);
			let label = input_mask.sample(u, v) as usize;
			let multiplier = (label == current_label) as u8;

			apply_mask(result_last, x as usize, y as usize, multiplier);

			if label < NUM_LABELS {
				label_appeared[label] = true;
				max_label = max_label.max(label);
			}
		}
	}

	if !label_appeared[current_label] {
		result.pop();
	}

	for i in 1..=max_label.max(NUM_LABELS) {
		current_label = i;

		if !label_appeared[current_label] {
			continue;
		}

		result.push(input_image.clone());
		let result_last = result.last_mut().unwrap();

		for y in 0..input_image.image.height {
			let v = (y as f32) / (input_image.image.height as f32);
			for x in 0..input_image.image.width {
				let u = (x as f32) / (input_image.image.width as f32);
				let label = input_mask.sample(u, v) as usize;
				let multiplier = (label == current_label) as u8;

				apply_mask(result_last, x as usize, y as usize, multiplier);
			}
		}
	}

	result
}

fn convert_image_to_mask(input: &ImageFrame<Color>) -> Vec<u8> {
	let mut result = vec![0_u8; (input.image.width * input.image.height) as usize];
	let mut colors = HashMap::<[u8; 4], usize>::new();
	let mut last_value = 0_usize;

	for (color, result) in input.image.data.iter().zip(result.iter_mut()) {
		let color = color.to_rgba8_srgb();
		if let Some(value) = colors.get(&color) {
			*result = *value as u8;
		} else {
			if last_value > u8::MAX as usize {
				warn!("The limit for number of segments ({}) has been exceeded!", u8::MAX);
				break;
			}

			*result = last_value as u8;
			colors.insert(color, last_value);
			last_value += 1;
		}
	}

	result
}

#[derive(Debug)]
pub struct ImageSegmentationNode<MaskImage> {
	pub(crate) mask_image: MaskImage,
}

#[node_macro::node_fn(ImageSegmentationNode)]
pub(crate) fn image_segmentation(image: ImageFrame<Color>, mask_image: ImageFrame<Color>) -> Vec<ImageFrame<Color>> {
	let mask_data = convert_image_to_mask(&mask_image);
	let mask = Mask {
		data: mask_data,
		width: mask_image.image.width as usize,
		height: mask_image.image.height as usize,
	};

	image_segmentation(&image, &mask)
}
