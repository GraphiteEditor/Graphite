//! The outline of a subtree as the rows it covers in each grid column, which lets subtrees fit into each other's gaps.

use glam::IVec2;
use std::collections::VecDeque;

/// The rows `[top, bottom)` covered in a column, empty when `top >= bottom`.
type Span = [i32; 2];

const EMPTY: Span = [i32::MAX, i32::MIN];

fn is_empty([top, bottom]: Span) -> bool {
	top >= bottom
}

/// A rectangle of grid cells from `min` up to but not including `max`.
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
	pub min: IVec2,
	pub max: IVec2,
}

/// Span buffers of outlines merged into larger ones, handed out again so laying out a graph allocates only as many as are alive at once.
#[derive(Default)]
pub struct SpanPool(Vec<VecDeque<Span>>);

impl SpanPool {
	fn take(&mut self) -> VecDeque<Span> {
		self.0.pop().unwrap_or_default()
	}

	fn give_back(&mut self, mut spans: VecDeque<Span>) {
		if spans.capacity() > 0 {
			spans.clear();
			self.0.push(spans);
		}
	}
}

/// The spans of consecutive columns, stored relative to `offset` so moving a whole outline is one addition.
#[derive(Clone, Debug, Default)]
pub struct Contour {
	offset: IVec2,
	/// The column of the first stored span, before `offset`.
	first_column: i32,
	spans: VecDeque<Span>,
}

impl Contour {
	fn columns(&self) -> usize {
		self.spans.len()
	}

	pub fn moved(mut self, by: IVec2) -> Self {
		self.offset += by;
		self
	}

	fn left(&self) -> i32 {
		self.offset.x + self.first_column
	}

	/// The rows covered in `column`.
	fn span(&self, column: i32) -> Span {
		let index = column - self.left();
		if index < 0 {
			return EMPTY;
		}
		match self.spans.get(index as usize) {
			Some(span) if !is_empty(*span) => [span[0] + self.offset.y, span[1] + self.offset.y],
			_ => EMPTY,
		}
	}

	fn cover(&mut self, column: i32, [top, bottom]: Span) {
		if self.spans.is_empty() {
			self.first_column = column - self.offset.x;
		}

		let mut index = column - self.left();
		while index < 0 {
			self.spans.push_front(EMPTY);
			self.first_column -= 1;
			index += 1;
		}
		while index as usize >= self.spans.len() {
			self.spans.push_back(EMPTY);
		}

		let span = &mut self.spans[index as usize];
		span[0] = span[0].min(top - self.offset.y);
		span[1] = span[1].max(bottom - self.offset.y);
	}

	/// Adds the cells of `bounds`, taking a buffer from `pool` if this outline has none yet.
	pub fn cover_bounds(&mut self, bounds: Bounds, pool: &mut SpanPool) {
		if self.spans.capacity() == 0 {
			self.spans = pool.take();
		}
		for column in bounds.min.x..bounds.max.x {
			self.cover(column, [bounds.min.y, bounds.max.y]);
		}
	}

	/// The outline covering both, built by adding the columns of the smaller one to the larger one. The smaller one's buffer goes back to `pool`.
	pub fn union(self, other: Self, pool: &mut SpanPool) -> Self {
		let (mut larger, smaller) = if self.columns() >= other.columns() { (self, other) } else { (other, self) };
		let left = smaller.left();
		for (index, span) in smaller.spans.iter().enumerate() {
			if !is_empty(*span) {
				larger.cover(left + index as i32, [span[0] + smaller.offset.y, span[1] + smaller.offset.y]);
			}
		}
		pool.give_back(smaller.spans);
		larger
	}

	/// How far `self` has to move down to lie at least `row_gap` rows below `other` in every column where the two come
	/// within `column_gap` columns of each other, found by walking the columns of the smaller outline.
	pub fn clearance_below(&self, other: &Self, row_gap: i32, column_gap: i32) -> i32 {
		let walking_self = self.columns() <= other.columns();
		let (walked, looked_up) = if walking_self { (self, other) } else { (other, self) };

		let mut shift = 0;
		let left = walked.left();
		for (index, span) in walked.spans.iter().enumerate() {
			if is_empty(*span) {
				continue;
			}
			let column = left + index as i32;
			let [top, bottom] = [span[0] + walked.offset.y, span[1] + walked.offset.y];

			for near_column in column - column_gap..=column + column_gap {
				let [near_top, near_bottom] = looked_up.span(near_column);
				if near_top >= near_bottom {
					continue;
				}
				let needed = if walking_self { near_bottom + row_gap - top } else { bottom + row_gap - near_top };
				shift = shift.max(needed);
			}
		}
		shift
	}

	/// How far `self` has to move down to lie at least `row_gap` rows below `bounds` in every column within `column_gap` of it.
	pub fn clearance_below_bounds(&self, bounds: Bounds, row_gap: i32, column_gap: i32) -> i32 {
		(bounds.min.x - column_gap..bounds.max.x + column_gap)
			.map(|column| self.span(column))
			.filter(|span| !is_empty(*span))
			.map(|[top, _]| bounds.max.y + row_gap - top)
			.fold(0, i32::max)
	}

	/// How far `self` has to move left so none of its columns covering rows within `row_gap` of `bounds` comes within `column_gap` columns of it.
	/// Only the columns of `self` from just left of `bounds` rightward can come that close, so the walk covers only those.
	pub fn clearance_left_of_bounds(&self, bounds: Bounds, row_gap: i32, column_gap: i32) -> i32 {
		let first_close_column = bounds.min.x - column_gap;
		let last_column = (self.left() + self.columns() as i32 - 1).min(bounds.max.x + column_gap - 1);

		(first_close_column..=last_column)
			.rev()
			.find(|column| {
				let [top, bottom] = self.span(*column);
				top < bottom && top < bounds.max.y + row_gap && bounds.min.y < bottom + row_gap
			})
			.map_or(0, |column| column - first_close_column + 1)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn rectangle(min: IVec2, max: IVec2) -> Contour {
		let mut contour = Contour::default();
		contour.cover_bounds(Bounds { min, max }, &mut SpanPool::default());
		contour
	}

	#[test]
	fn an_outline_fits_into_the_gap_of_another() {
		// An upside down L: a wide top reaching right, over a narrow column on the left
		let mut pool = SpanPool::default();
		let shape = rectangle(IVec2::ZERO, IVec2::new(2, 6)).union(rectangle(IVec2::ZERO, IVec2::new(8, 1)), &mut pool);

		// A box under the roof, right of the column, only has to clear the roof
		let under_the_roof = rectangle(IVec2::new(4, 0), IVec2::new(7, 2));
		assert_eq!(under_the_roof.clearance_below(&shape, 0, 0), 1);

		// A box reaching across the column has to clear all of it
		let across = rectangle(IVec2::new(1, 0), IVec2::new(4, 2));
		assert_eq!(across.clearance_below(&shape, 0, 0), 6);
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
