use glam::DVec2;
use image::{ImageFormat, Rgba, RgbaImage};
use std::io::Cursor;
use std::path::Path;

use crate::session::write_file;

const COLUMNS: usize = 2;
const GAP: f64 = 4.;
const CROSSHAIR_HALF_SIZE: f64 = 10.;

/// A capture of the page, or an area of it, with where the pointer was.
pub struct Frame {
	/// The capture as a PNG.
	pub image: Vec<u8>,
	/// The pointer's position measured from the capture's top left corner, in CSS pixels.
	pub pointer: DVec2,
	/// The pointer's position on the page, in CSS pixels.
	pub page_pointer: DVec2,
}

/// Lays captures out in a grid, read left to right then top to bottom, each with a crosshair where the pointer was, and writes it as a PNG.
pub fn write_contact_sheet(path: &Path, frames: &[Frame], scale: f64) -> Result<(), String> {
	let images = frames
		.iter()
		.map(|frame| {
			let mut image = image::load_from_memory_with_format(&frame.image, ImageFormat::Png)
				.map_err(|error| format!("A capture could not be read as a PNG: {error}"))?
				.to_rgba8();
			draw_crosshair(&mut image, frame.pointer, scale);
			Ok(image)
		})
		.collect::<Result<Vec<_>, String>>()?;
	if images.is_empty() {
		return Err("There are no captures to lay out".to_string());
	}

	let columns = COLUMNS.min(images.len()) as u32;
	let rows = images.len().div_ceil(COLUMNS) as u32;
	let gap = (GAP * scale).round() as u32;
	let cell_width = images.iter().map(RgbaImage::width).max().unwrap_or_default();
	let cell_height = images.iter().map(RgbaImage::height).max().unwrap_or_default();

	let sheet_width = columns * cell_width + (columns - 1) * gap;
	let sheet_height = rows * cell_height + (rows - 1) * gap;
	let mut sheet = RgbaImage::from_pixel(sheet_width, sheet_height, Rgba([0, 0, 0, 255]));

	for (index, image) in images.iter().enumerate() {
		let column = index as u32 % columns;
		let row = index as u32 / columns;
		image::imageops::replace(&mut sheet, image, (column * (cell_width + gap)) as i64, (row * (cell_height + gap)) as i64);
	}

	let mut encoded = Cursor::new(Vec::new());
	sheet.write_to(&mut encoded, ImageFormat::Png).map_err(|error| format!("Failed to encode the image: {error}"))?;
	write_file(path, encoded.get_ref())
}

// Inverts the colors under a crosshair one CSS pixel thick, keeping it visible against light and dark backgrounds alike
fn draw_crosshair(image: &mut RgbaImage, pointer: DVec2, scale: f64) {
	let to_device_pixels = |css: DVec2| (css * scale).round().as_i64vec2();

	// Half-open ranges of device pixels for each line's thickness and its reach to either side of the pointer
	let line_start = to_device_pixels(pointer);
	let line_end = to_device_pixels(pointer + 1.);
	let reach_start = to_device_pixels(pointer - CROSSHAIR_HALF_SIZE);
	let reach_end = to_device_pixels(pointer + CROSSHAIR_HALF_SIZE + 1.);

	for y in reach_start.y.max(0)..reach_end.y.min(image.height() as i64) {
		for x in reach_start.x.max(0)..reach_end.x.min(image.width() as i64) {
			let on_vertical_line = (line_start.x..line_end.x).contains(&x);
			let on_horizontal_line = (line_start.y..line_end.y).contains(&y);
			if !on_vertical_line && !on_horizontal_line {
				continue;
			}

			let pixel = image.get_pixel_mut(x as u32, y as u32);
			for channel in &mut pixel.0[..3] {
				*channel = 255 - *channel;
			}
		}
	}
}
