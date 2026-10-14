use crate::parallel_rows::fill_rows;
use core_types::color::Color;
use core_types::context::Ctx;
use core_types::list::Item;
use raster_types::Bitmap;
use raster_types::Image;
use raster_types::{CPU, Raster};

/// Applies a median filter to reduce noise while preserving edges.
#[node_macro::node(category("Raster: Filter"), icon("NodeMedianFilter"), memoize)]
fn median_filter(
	_: impl Ctx,
	/// The image to be filtered.
	image_frame: Item<Raster<CPU>>,
	/// The radius of the filter kernel. Larger values remove more noise but may blur fine details.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..50)]
	radius: Item<f64>,
) -> Item<Raster<CPU>> {
	let radius = *radius.element();

	let (image, attributes) = image_frame.into_parts();

	// TODO: Scale the radius through the footprint so it acts as the document-space length its Pixels unit promises, with a
	// window that varies continuously rather than snapping to whole source pixels
	let filtered_image = if radius < 0.5 {
		// Minimum filter radius
		image
	} else {
		Raster::new_cpu(median_filter_algorithm(image.into_data(), radius as u32))
	};

	Item::from_parts(filtered_image, attributes)
}

fn median_filter_algorithm(original_buffer: Image<Color>, radius: u32) -> Image<Color> {
	let (width, height) = original_buffer.dimensions();
	let mut output = Image::new(width, height, Color::TRANSPARENT);
	let window_capacity = ((2 * radius + 1).pow(2)) as usize;

	fill_rows(&mut output.data, width as usize, |y, row| {
		let y = y as u32;

		// Allocated once for the row and reused for each of its pixels
		let mut r_vals: Vec<f32> = Vec::with_capacity(window_capacity);
		let mut g_vals: Vec<f32> = Vec::with_capacity(window_capacity);
		let mut b_vals: Vec<f32> = Vec::with_capacity(window_capacity);
		let mut a_vals: Vec<f32> = Vec::with_capacity(window_capacity);

		for (x, output_pixel) in row.iter_mut().enumerate() {
			let x = x as u32;
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

			*output_pixel = Color::from_rgbaf32_unchecked(r, g, b, a);
		}
	});

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
