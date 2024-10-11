use crate::{Pixel, RawImage};

fn average(data: &[u16], indexes: impl Iterator<Item = i64>) -> u16 {
	let mut sum = 0;
	let mut count = 0;
	for index in indexes {
		if index >= 0 && (index as usize) < data.len() {
			sum += data[index as usize] as u32;
			count += 1;
		}
	}

	(sum / count) as u16
}

// This trait is here only to circumvent Rust's lifetime capturing rules in return type impl Trait.
// See https://youtu.be/CWiz_RtA1Hw?si=j0si4qE2Y20f71Uo
// This should be removed when Rust 2024 edition is released as described in https://blog.rust-lang.org/2024/09/05/impl-trait-capture-rules.html
pub trait Captures<U> {}
impl<T: ?Sized, U> Captures<U> for T {}

impl RawImage {
	pub fn linear_demosaic_iter(&self) -> impl Iterator<Item = Pixel> + Captures<&'_ ()> {
		match self.cfa_pattern {
			[0, 1, 1, 2] => self.linear_demosaic_rggb_iter(),
			_ => todo!(),
		}
	}

	fn linear_demosaic_rggb_iter(&self) -> impl Iterator<Item = Pixel> + Captures<&'_ ()> {
		let width = self.width as i64;
		let height = self.height as i64;

		(0..height).flat_map(move |row| {
			let row_by_width = row * width;

			(0..width).map(move |column| {
				let pixel_index = row_by_width + column;

				let vertical_indexes = [pixel_index + width, pixel_index - width];
				let horizontal_indexes = [pixel_index + 1, pixel_index - 1];
				let cross_indexes = [pixel_index + width, pixel_index - width, pixel_index + 1, pixel_index - 1];
				let diagonal_indexes = [pixel_index + width + 1, pixel_index - width + 1, pixel_index + width - 1, pixel_index - width - 1];

				let pixel_index = pixel_index as usize;
				match (row % 2 == 0, column % 2 == 0) {
					(true, true) => Pixel {
						red: self.data[pixel_index],
						blue: average(&self.data, cross_indexes.into_iter()),
						green: average(&self.data, diagonal_indexes.into_iter()),
						row: row as usize,
						column: column as usize,
					},
					(true, false) => Pixel {
						red: average(&self.data, horizontal_indexes.into_iter()),
						blue: self.data[pixel_index],
						green: average(&self.data, vertical_indexes.into_iter()),
						row: row as usize,
						column: column as usize,
					},
					(false, true) => Pixel {
						red: average(&self.data, vertical_indexes.into_iter()),
						blue: self.data[pixel_index],
						green: average(&self.data, horizontal_indexes.into_iter()),
						row: row as usize,
						column: column as usize,
					},
					(false, false) => Pixel {
						red: average(&self.data, diagonal_indexes.into_iter()),
						blue: average(&self.data, cross_indexes.into_iter()),
						green: self.data[pixel_index],
						row: row as usize,
						column: column as usize,
					},
				}
			})
		})
	}
}
