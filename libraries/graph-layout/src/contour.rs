//! The outline of a subtree as the rows it covers in each grid column, which lets subtrees fit into each other's gaps.

use glam::IVec2;
use std::collections::VecDeque;

/// The rows `[top, bottom)` covered in a column, empty when `top >= bottom`.
type Span = [i32; 2];

const EMPTY: Span = [i32::MAX, i32::MIN];

fn is_empty([top, bottom]: Span) -> bool {
	top >= bottom
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
	/// A rectangle `size` cells large with its top left corner at the origin, or no outline at all for a zero size.
	pub fn rectangle(size: IVec2) -> Self {
		let spans = if size.y > 0 { (0..size.x).map(|_| [0, size.y]).collect() } else { VecDeque::new() };
		Self {
			offset: IVec2::ZERO,
			first_column: 0,
			spans,
		}
	}

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

	/// Each column with the rows it covers.
	fn covered_columns(&self) -> impl Iterator<Item = (i32, Span)> + '_ {
		let left = self.left();
		self.spans
			.iter()
			.enumerate()
			.filter(|(_, span)| !is_empty(**span))
			.map(move |(index, span)| (left + index as i32, [span[0] + self.offset.y, span[1] + self.offset.y]))
	}

	fn cover(&mut self, column: i32, [top, bottom]: Span) {
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

	/// The outline covering both, built by adding the columns of the smaller one to the larger one.
	pub fn union(self, other: Self) -> Self {
		let (mut larger, smaller) = if self.columns() >= other.columns() { (self, other) } else { (other, self) };
		for (column, span) in smaller.covered_columns() {
			larger.cover(column, span);
		}
		larger
	}

	/// How far `self` has to move down to lie at least `row_gap` rows below `other` in every column where the two come
	/// within `column_gap` columns of each other, found by walking the columns of the smaller outline.
	pub fn clearance_below(&self, other: &Self, row_gap: i32, column_gap: i32) -> i32 {
		let reach = column_gap;
		let shift_past = |[top, _]: Span, [_, other_bottom]: Span| other_bottom + row_gap - top;

		let shifts: Box<dyn Iterator<Item = i32>> = if self.columns() <= other.columns() {
			Box::new(self.covered_columns().flat_map(move |(column, span)| {
				(column - reach..=column + reach)
					.map(move |other_column| other.span(other_column))
					.filter(|span| !is_empty(*span))
					.map(move |other_span| shift_past(span, other_span))
			}))
		} else {
			Box::new(other.covered_columns().flat_map(move |(other_column, other_span)| {
				(other_column - reach..=other_column + reach)
					.map(move |column| self.span(column))
					.filter(|span| !is_empty(*span))
					.map(move |span| shift_past(span, other_span))
			}))
		};
		shifts.fold(0, i32::max)
	}

	/// How far `self` has to move left so none of its columns covering rows within `row_gap` of `other`'s comes within `column_gap` columns of it.
	/// Only the columns of `self` from just left of `other` rightward can come that close, so the walk starts there.
	pub fn clearance_left(&self, other: &Self, row_gap: i32, column_gap: i32) -> i32 {
		let Some(other_left) = other.covered_columns().next().map(|(column, _)| column) else { return 0 };
		let other_right = other.left() + other.columns() as i32;

		let near_rows = |[top, bottom]: Span| {
			other
				.covered_columns()
				.any(|(_, [other_top, other_bottom])| top < other_bottom + row_gap && other_top < bottom + row_gap)
		};
		let first_close_column = other_left - column_gap;
		let last_column = self.left() + self.columns() as i32 - 1;

		(first_close_column..=last_column)
			.rev()
			.find(|column| {
				let span = self.span(*column);
				!is_empty(span) && near_rows(span) && *column < other_right + column_gap
			})
			.map_or(0, |column| column - first_close_column + 1)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn an_outline_fits_into_the_gap_of_another() {
		// An upside down L: a wide top reaching right, over a narrow column on the left
		let column = Contour::rectangle(IVec2::new(2, 6));
		let roof = Contour::rectangle(IVec2::new(8, 1));
		let shape = column.union(roof);

		// A box under the roof, right of the column, only has to clear the roof
		let under_the_roof = Contour::rectangle(IVec2::new(3, 2)).moved(IVec2::new(4, 0));
		assert_eq!(under_the_roof.clearance_below(&shape, 0, 0), 1);

		// A box reaching across the column has to clear all of it
		let across = Contour::rectangle(IVec2::new(3, 2)).moved(IVec2::new(1, 0));
		assert_eq!(across.clearance_below(&shape, 0, 0), 6);
	}

	#[test]
	fn clearing_left_only_counts_columns_near_the_other_outline() {
		let reader = Contour::rectangle(IVec2::new(5, 2));
		let overlapping = Contour::rectangle(IVec2::new(5, 1)).moved(IVec2::new(-2, 1));
		let far_below = Contour::rectangle(IVec2::new(5, 1)).moved(IVec2::new(-2, 9));
		let to_the_right = Contour::rectangle(IVec2::new(5, 1)).moved(IVec2::new(7, 0));

		assert_eq!(overlapping.clearance_left(&reader, 1, 2), 5);
		assert_eq!(far_below.clearance_left(&reader, 1, 2), 0);
		assert_eq!(to_the_right.clearance_left(&reader, 1, 2), 0);
	}
}
