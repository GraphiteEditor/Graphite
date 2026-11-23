mod column;
#[cfg(test)]
mod tests;

use crate::Rect;
use column::{Column, Span};
use glam::IVec2;
use std::collections::VecDeque;
use std::ops::{ControlFlow, RangeInclusive};

#[derive(Default)]
pub(crate) struct ColumnPool(Vec<VecDeque<Column>>);

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

#[derive(Clone, Debug, Default)]
pub(crate) struct Contour {
	offset: IVec2,
	first_column: i32,
	columns: VecDeque<Column>,
}

impl Contour {
	pub(crate) fn moved(mut self, by: IVec2) -> Self {
		self.offset += by;
		self
	}

	pub(crate) fn cover_rect(&mut self, rect: Rect, pool: &mut ColumnPool) {
		if self.columns.capacity() == 0 {
			self.columns = pool.take();
		}
		let span = Span { top: rect.min.y, bottom: rect.max.y };
		(rect.min.x..rect.max.x).for_each(|column| self.cover(column, span));
	}

	pub(crate) fn release(self, pool: &mut ColumnPool) {
		pool.give_back(self.columns);
	}

	pub(crate) fn union(self, other: Self, pool: &mut ColumnPool) -> Self {
		let (mut wider, narrower) = if self.columns.len() >= other.columns.len() { (self, other) } else { (other, self) };
		narrower.column_spans().for_each(|(column, span)| wider.cover(column, span));
		pool.give_back(narrower.columns);
		wider
	}

	pub(crate) fn clearance_below(&self, other: &Self, gap: IVec2) -> i32 {
		if self.columns.len() <= other.columns.len() {
			walk_until_a_whole_round_stays(self.columns_near(other, gap.x), |column, shift| self.pushed_from_own_column(column, other, gap, shift))
		} else {
			walk_until_a_whole_round_stays(other.columns_near(self, gap.x), |other_column, shift| self.pushed_from_other_column(other_column, other, gap, shift))
		}
	}

	pub(crate) fn clearance_below_rect(&self, rect: Rect, gap: IVec2) -> i32 {
		(rect.min.x - gap.x..rect.max.x + gap.x)
			.filter_map(|column| self.spans(column).next())
			.map(|span| rect.max.y + gap.y - span.top)
			.fold(0, i32::max)
	}

	pub(crate) fn clearance_left_of_rect(&self, rect: Rect, gap: IVec2) -> i32 {
		let rows = Span { top: rect.min.y, bottom: rect.max.y };
		let first_close_column = rect.min.x - gap.x;
		let last_close_column = self.right().min(rect.max.x + gap.x - 1);

		(first_close_column..=last_close_column)
			.rev()
			.find(|column| self.spans(*column).any(|span| span.meets(rows, gap.y)))
			.map_or(0, |column| column - first_close_column + 1)
	}

	fn columns_near(&self, other: &Self, columns_apart: i32) -> RangeInclusive<i32> {
		self.left().max(other.left() - columns_apart)..=self.right().min(other.right() + columns_apart)
	}

	fn pushed_from_own_column(&self, column: i32, other: &Self, gap: IVec2, shift: i32) -> i32 {
		self.spans(column).fold(shift, |shift, moving| {
			(column - gap.x..=column + gap.x).fold(shift, |shift, other_column| other.spans(other_column).fold(shift, |shift, fixed| pushed(moving, fixed, gap.y, shift)))
		})
	}

	fn pushed_from_other_column(&self, other_column: i32, other: &Self, gap: IVec2, shift: i32) -> i32 {
		other.spans(other_column).fold(shift, |shift, fixed| {
			(other_column - gap.x..=other_column + gap.x).fold(shift, |shift, column| self.spans(column).fold(shift, |shift, moving| pushed(moving, fixed, gap.y, shift)))
		})
	}

	fn column_spans(&self) -> impl Iterator<Item = (i32, Span)> + '_ {
		let rows = self.offset.y;
		(self.left()..)
			.zip(&self.columns)
			.flat_map(move |(column, spans)| spans.iter().map(move |span| (column, span.moved(rows))))
	}

	fn spans(&self, column: i32) -> impl Iterator<Item = Span> + '_ {
		let rows = self.offset.y;
		self.stored_spans(column).iter().map(move |span| span.moved(rows))
	}

	fn stored_spans(&self, column: i32) -> &[Span] {
		usize::try_from(column - self.left())
			.ok()
			.and_then(|index| self.columns.get(index))
			.map_or(&[], |column| column.as_slice())
	}

	fn cover(&mut self, column: i32, span: Span) {
		let index = self.index_extended_to(column);
		let rows = self.offset.y;
		column::cover(&mut self.columns[index], span.moved(-rows));
	}

	fn index_extended_to(&mut self, column: i32) -> usize {
		if self.columns.is_empty() {
			self.first_column = column - self.offset.x;
		}

		let missing_left = (self.left() - column).max(0);
		self.first_column -= missing_left;
		(0..missing_left).for_each(|_| self.columns.push_front(Column::new()));

		let index = (column - self.left()).unsigned_abs() as usize;
		(self.columns.len()..=index).for_each(|_| self.columns.push_back(Column::new()));
		index
	}

	fn left(&self) -> i32 {
		self.offset.x + self.first_column
	}

	fn right(&self) -> i32 {
		self.left() + i32::try_from(self.columns.len()).unwrap_or(i32::MAX) - 1
	}
}

fn walk_until_a_whole_round_stays(columns: RangeInclusive<i32>, pushed_from: impl Fn(i32, i32) -> i32) -> i32 {
	let column_count = (columns.end() - columns.start() + 1).max(0);
	let walk = columns.cycle().try_fold((0, 0), |(shift, unmoved_columns), column| {
		if unmoved_columns == column_count {
			return ControlFlow::Break(shift);
		}
		let pushed = pushed_from(column, shift);
		ControlFlow::Continue((pushed, if pushed == shift { unmoved_columns + 1 } else { 0 }))
	});
	match walk {
		ControlFlow::Break(shift) | ControlFlow::Continue((shift, _)) => shift,
	}
}

fn pushed(moving: Span, fixed: Span, rows_apart: i32, shift: i32) -> i32 {
	if moving.moved(shift).meets(fixed, rows_apart) {
		fixed.bottom + rows_apart - moving.top
	} else {
		shift
	}
}
