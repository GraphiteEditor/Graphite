//! Spreads per-row image work across threads on platforms that have them.

/// Below this many pixels, starting threads costs more than they save.
#[cfg(not(target_family = "wasm"))]
const MIN_PIXELS_FOR_THREADS: usize = 65_536;

/// Calls `fill_row` with the index and pixels of each row of `pixels`, an image `width` pixels wide.
/// On native platforms, bands of rows run on separate threads, so `fill_row` must not depend on the order rows are filled in.
pub fn fill_rows<P: Send>(pixels: &mut [P], width: usize, fill_row: impl Fn(usize, &mut [P]) + Sync) {
	if width == 0 {
		return;
	}

	#[cfg(not(target_family = "wasm"))]
	if pixels.len() >= MIN_PIXELS_FOR_THREADS {
		let threads = std::thread::available_parallelism().map_or(1, |threads| threads.get());
		let rows_per_band = (pixels.len() / width).div_ceil(threads).max(1);

		std::thread::scope(|scope| {
			for (band_index, band) in pixels.chunks_mut(rows_per_band * width).enumerate() {
				let fill_row = &fill_row;
				scope.spawn(move || {
					for (row_in_band, row) in band.chunks_mut(width).enumerate() {
						fill_row(band_index * rows_per_band + row_in_band, row);
					}
				});
			}
		});
		return;
	}

	for (y, row) in pixels.chunks_mut(width).enumerate() {
		fill_row(y, row);
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn every_row_is_filled_with_its_own_index() {
		// Large enough to run on threads where there are any, with a ragged count of rows per band
		let (width, height) = (300, 301);
		let mut pixels = vec![0_usize; width * height];

		fill_rows(&mut pixels, width, |y, row| row.iter_mut().enumerate().for_each(|(x, pixel)| *pixel = y * width + x));

		assert!(pixels.iter().enumerate().all(|(index, pixel)| *pixel == index));
	}
}
