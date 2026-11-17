//! The outline of a subtree as the rows it covers in each grid column, with the gaps between them, which lets subtrees fit into each other's gaps.

use glam::IVec2;
use smallvec::SmallVec;
use std::collections::VecDeque;

/// The rows `[top, bottom)` covered in a column.
type Span = [i32; 2];

/// The most spans a column keeps, beyond which its smallest gap is covered, so a crowded column costs no more than a few nodes.
const SPANS_PER_COLUMN: usize = 4;

/// The spans covered in a column from the top, apart from each other with at least one empty row between them.
type Column = SmallVec<[Span; 1]>;

fn cover_span(column: &mut Column, [top, bottom]: Span) {
	let first_touching = column.partition_point(|span| span[1] < top);
	let past_touching = column.partition_point(|span| span[0] <= bottom);
	if first_touching == past_touching {
		column.insert(first_touching, [top, bottom]);
		if column.len() > SPANS_PER_COLUMN {
			close_smallest_gap(column);
		}
		return;
	}

	column[first_touching] = [top.min(column[first_touching][0]), bottom.max(column[past_touching - 1][1])];
	column.drain(first_touching + 1..past_touching);
}

fn close_smallest_gap(column: &mut Column) {
	let Some(above_gap) = (0..column.len() - 1).min_by_key(|index| column[index + 1][0] - column[*index][1]) else {
		return;
	};
	column[above_gap][1] = column[above_gap + 1][1];
	column.remove(above_gap + 1);
}

/// Whether `[top, bottom)` comes closer than `row_gap` rows to `[other_top, other_bottom)`.
fn spans_meet([top, bottom]: Span, [other_top, other_bottom]: Span, row_gap: i32) -> bool {
	top < other_bottom + row_gap && other_top < bottom + row_gap
}

/// A rectangle of grid cells from `min` up to but not including `max`.
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
	pub min: IVec2,
	pub max: IVec2,
}

/// Column buffers of outlines merged into larger ones, handed out again so laying out a graph allocates only as many as are alive at once.
#[derive(Default)]
pub struct ColumnPool(Vec<VecDeque<Column>>);

impl ColumnPool {
	fn take(&mut self) -> VecDeque<Column> {
		self.0.pop().unwrap_or_default()
	}

	fn give_back(&mut self, mut columns: VecDeque<Column>) {
		if columns.capacity() > 0 {
			columns.clear();
			self.0.push(columns);
		}
	}
}

/// The spans of consecutive columns, stored relative to `offset` so moving a whole outline is one addition.
#[derive(Clone, Debug, Default)]
pub struct Contour {
	offset: IVec2,
	/// The column of the first stored column, before `offset`.
	first_column: i32,
	columns: VecDeque<Column>,
}

impl Contour {
	fn width(&self) -> usize {
		self.columns.len()
	}

	pub fn moved(mut self, by: IVec2) -> Self {
		self.offset += by;
		self
	}

	fn left(&self) -> i32 {
		self.offset.x + self.first_column
	}

	/// The spans covered in `column`, before `offset`.
	fn column(&self, column: i32) -> &[Span] {
		let index = column - self.left();
		if index < 0 {
			return &[];
		}
		self.columns.get(index as usize).map_or(&[], |column| column.as_slice())
	}

	fn spans(&self, column: i32) -> impl Iterator<Item = Span> + '_ {
		let row = self.offset.y;
		self.column(column).iter().map(move |[top, bottom]| [top + row, bottom + row])
	}

	fn cover(&mut self, column: i32, [top, bottom]: Span) {
		if self.columns.is_empty() {
			self.first_column = column - self.offset.x;
		}

		let mut index = column - self.left();
		while index < 0 {
			self.columns.push_front(Column::new());
			self.first_column -= 1;
			index += 1;
		}
		while index as usize >= self.columns.len() {
			self.columns.push_back(Column::new());
		}

		cover_span(&mut self.columns[index as usize], [top - self.offset.y, bottom - self.offset.y]);
	}

	/// Adds the cells of `bounds`, taking a buffer from `pool` if this outline has none yet.
	pub fn cover_bounds(&mut self, bounds: Bounds, pool: &mut ColumnPool) {
		if self.columns.capacity() == 0 {
			self.columns = pool.take();
		}
		for column in bounds.min.x..bounds.max.x {
			self.cover(column, [bounds.min.y, bounds.max.y]);
		}
	}

	/// The outline covering both, built by adding the columns of the narrower one to the wider one. The narrower one's buffer goes back to `pool`.
	pub fn union(self, other: Self, pool: &mut ColumnPool) -> Self {
		let (mut wider, narrower) = if self.width() >= other.width() { (self, other) } else { (other, self) };

		let left = narrower.left();
		for (index, column) in narrower.columns.iter().enumerate() {
			for [top, bottom] in column {
				wider.cover(left + index as i32, [top + narrower.offset.y, bottom + narrower.offset.y]);
			}
		}

		pool.give_back(narrower.columns);
		wider
	}

	/// The least distance `self` has to move down so none of its spans comes within `row_gap` rows of a span of `other`
	/// in a column within `column_gap` columns, which may leave it in a gap of `other`.
	/// The columns of the narrower outline are walked round until a whole round passes without moving further.
	pub fn clearance_below(&self, other: &Self, row_gap: i32, column_gap: i32) -> i32 {
		let walking_self = self.width() <= other.width();
		let (walked, looked_up) = if walking_self { (self, other) } else { (other, self) };
		let width = walked.width();
		let left = walked.left();

		let mut shift = 0;
		let mut columns_without_moving = 0;
		let mut index = 0;
		while columns_without_moving < width {
			let column = left + index as i32;
			let mut moving = false;

			for walked_span in walked.spans(column) {
				for near_column in column - column_gap..=column + column_gap {
					let row = looked_up.offset.y;
					let looked_up_spans = looked_up.column(near_column);

					if walking_self {
						let [top, bottom] = walked_span;
						let first_below = looked_up_spans.partition_point(|span| span[1] + row + row_gap <= top + shift);
						for &[fixed_top, fixed_bottom] in &looked_up_spans[first_below..] {
							if !spans_meet([top + shift, bottom + shift], [fixed_top + row, fixed_bottom + row], row_gap) {
								break;
							}
							shift = fixed_bottom + row + row_gap - top;
							moving = true;
						}
					} else {
						let fixed = walked_span;
						let first_below = looked_up_spans.partition_point(|span| span[1] + row + shift + row_gap <= fixed[0]);
						if let Some(&[top, bottom]) = looked_up_spans.get(first_below)
							&& spans_meet([top + row + shift, bottom + row + shift], fixed, row_gap)
						{
							shift = fixed[1] + row_gap - top - row;
							moving = true;
						}
					}
				}
			}

			columns_without_moving = if moving { 0 } else { columns_without_moving + 1 };
			index = (index + 1) % width;
		}
		shift
	}

	/// How far `self` has to move down so its top in every column within `column_gap` of `bounds` lies at least `row_gap` rows below it.
	pub fn clearance_below_bounds(&self, bounds: Bounds, row_gap: i32, column_gap: i32) -> i32 {
		(bounds.min.x - column_gap..bounds.max.x + column_gap)
			.filter_map(|column| self.spans(column).next())
			.map(|[top, _]| bounds.max.y + row_gap - top)
			.fold(0, i32::max)
	}

	/// How far `self` has to move left so none of its columns covering rows within `row_gap` of `bounds` comes within `column_gap` columns of it.
	/// Only the columns of `self` from just left of `bounds` rightward can come that close, so the walk covers only those.
	pub fn clearance_left_of_bounds(&self, bounds: Bounds, row_gap: i32, column_gap: i32) -> i32 {
		let first_close_column = bounds.min.x - column_gap;
		let last_column = (self.left() + self.width() as i32 - 1).min(bounds.max.x + column_gap - 1);

		(first_close_column..=last_column)
			.rev()
			.find(|column| self.spans(*column).any(|span| spans_meet(span, [bounds.min.y, bounds.max.y], row_gap)))
			.map_or(0, |column| column - first_close_column + 1)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn rectangle(min: IVec2, max: IVec2) -> Contour {
		let mut contour = Contour::default();
		contour.cover_bounds(Bounds { min, max }, &mut ColumnPool::default());
		contour
	}

	#[test]
	fn an_outline_fits_into_the_gap_of_another() {
		// An upside down L: a wide top reaching right, over a narrow column on the left
		let mut pool = ColumnPool::default();
		let shape = rectangle(IVec2::ZERO, IVec2::new(2, 6)).union(rectangle(IVec2::ZERO, IVec2::new(8, 1)), &mut pool);

		// A box under the roof, right of the column, only has to clear the roof
		let under_the_roof = rectangle(IVec2::new(4, 0), IVec2::new(7, 2));
		assert_eq!(under_the_roof.clearance_below(&shape, 0, 0), 1);

		// A box reaching across the column has to clear all of it
		let across = rectangle(IVec2::new(1, 0), IVec2::new(4, 2));
		assert_eq!(across.clearance_below(&shape, 0, 0), 6);
	}

	#[test]
	fn an_outline_fits_into_a_gap_between_rows_of_another() {
		// A bar on top and a bar further down, with four empty rows between them
		let mut pool = ColumnPool::default();
		let bars = rectangle(IVec2::ZERO, IVec2::new(8, 1)).union(rectangle(IVec2::new(0, 5), IVec2::new(8, 6)), &mut pool);

		let fitting = rectangle(IVec2::new(2, 0), IVec2::new(5, 3));
		assert_eq!(fitting.clearance_below(&bars, 0, 0), 1);

		let too_tall = rectangle(IVec2::new(2, 0), IVec2::new(5, 5));
		assert_eq!(too_tall.clearance_below(&bars, 0, 0), 6);

		// Walked the other way round, from the narrower outline
		let wide_and_tall = rectangle(IVec2::new(-4, 0), IVec2::new(12, 5));
		assert_eq!(wide_and_tall.clearance_below(&bars, 0, 0), 6);
	}

	#[test]
	fn covering_merges_touching_spans() {
		let mut column = Column::new();
		cover_span(&mut column, [0, 2]);
		cover_span(&mut column, [5, 6]);
		cover_span(&mut column, [9, 10]);
		assert_eq!(column.as_slice(), &[[0, 2], [5, 6], [9, 10]]);

		cover_span(&mut column, [2, 5]);
		assert_eq!(column.as_slice(), &[[0, 6], [9, 10]]);
	}

	#[test]
	fn clearing_left_only_counts_columns_near_the_bounds() {
		let reader = Bounds {
			min: IVec2::ZERO,
			max: IVec2::new(5, 2),
		};
		let overlapping = rectangle(IVec2::new(-2, 1), IVec2::new(3, 2));
		let far_below = rectangle(IVec2::new(-2, 9), IVec2::new(3, 10));
		let to_the_right = rectangle(IVec2::new(7, 0), IVec2::new(12, 1));

		assert_eq!(overlapping.clearance_left_of_bounds(reader, 1, 2), 5);
		assert_eq!(far_below.clearance_left_of_bounds(reader, 1, 2), 0);
		assert_eq!(to_the_right.clearance_left_of_bounds(reader, 1, 2), 0);
	}
}
