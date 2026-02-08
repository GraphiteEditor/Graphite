use crate::aabb::Aabb;
use glam::{DVec2, IVec2};
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

pub(crate) struct Grid {
	cell_factor: f64,
	cells: FxHashMap<IVec2, SmallVec<[usize; 6]>>,
}

impl Grid {
	pub(crate) fn new(cell_size: f64, edges: usize) -> Self {
		Grid {
			cell_factor: cell_size.recip(),
			cells: FxHashMap::with_capacity_and_hasher(edges, Default::default()),
		}
	}

	pub(crate) fn insert(&mut self, bbox: &Aabb, index: usize) {
		let min_cell = self.point_to_cell_floor(bbox.min());
		let max_cell = self.point_to_cell_ceil(bbox.max());

		for i in min_cell.x..=max_cell.x {
			for j in min_cell.y..=max_cell.y {
				self.cells.entry((i, j).into()).or_default().push(index);
			}
		}
	}

	pub(crate) fn query(&self, bbox: &Aabb, result: &mut BitVec) {
		let min_cell = self.point_to_cell_floor(bbox.min());
		let max_cell = self.point_to_cell_ceil(bbox.max());

		for i in min_cell.x..=max_cell.x {
			for j in min_cell.y..=max_cell.y {
				if let Some(indices) = self.cells.get(&(i, j).into()) {
					for &index in indices {
						result.set(index);
					}
				}
			}
		}
		// result.sort_unstable();
		// result.dedup();
	}

	fn point_to_cell_ceil(&self, point: DVec2) -> IVec2 {
		(point * self.cell_factor).ceil().as_ivec2()
	}
	fn point_to_cell_floor(&self, point: DVec2) -> IVec2 {
		(point * self.cell_factor).floor().as_ivec2()
	}
}

pub struct BitVec {
	data: Vec<u64>,
}

impl BitVec {
	pub fn new(capacity: usize) -> Self {
		let num_words = capacity.div_ceil(64);
		BitVec { data: vec![0; num_words] }
	}

	pub fn set(&mut self, index: usize) {
		let word_index = index / 64;
		let bit_index = index % 64;
		self.data[word_index] |= 1u64 << bit_index;
	}

	pub fn clear(&mut self) {
		self.data.fill(0);
	}

	pub fn iter_set_bits(&self) -> BitVecIterator<'_> {
		BitVecIterator {
			bit_vec: self,
			current_word: self.data[0],
			word_index: 0,
		}
	}
}

pub struct BitVecIterator<'a> {
	bit_vec: &'a BitVec,
	current_word: u64,
	word_index: usize,
}

impl<'a> Iterator for BitVecIterator<'a> {
	type Item = usize;

	fn next(&mut self) -> Option<Self::Item> {
		loop {
			if self.current_word == 0 {
				self.word_index += 1;
				if self.word_index == self.bit_vec.data.len() {
					return None;
				}
				self.current_word = self.bit_vec.data[self.word_index];
				continue;
			}
			let tz = self.current_word.trailing_zeros() as usize;
			self.current_word ^= 1 << tz;

			let result = self.word_index * 64 + tz;

			return Some(result);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_bitvec() {
		let mut bv = BitVec::new(200);
		bv.set(5);
		bv.set(64);
		bv.set(128);
		bv.set(199);

		let set_bits: Vec<usize> = bv.iter_set_bits().collect();
		assert_eq!(set_bits, vec![5, 64, 128, 199]);
	}
}
