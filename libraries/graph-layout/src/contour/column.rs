use smallvec::SmallVec;
use std::ops::Range;

const SPANS_PER_COLUMN: usize = 4;

pub(super) type Column = SmallVec<[Span; 1]>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Span {
	pub(super) top: i32,
	pub(super) bottom: i32,
}

impl Span {
	pub(super) fn moved(self, rows: i32) -> Self {
		Self {
			top: self.top + rows,
			bottom: self.bottom + rows,
		}
	}

	pub(super) fn meets(self, other: Self, rows_apart: i32) -> bool {
		self.top < other.bottom + rows_apart && other.top < self.bottom + rows_apart
	}
}

pub(super) fn cover(column: &mut Column, span: Span) {
	let touching = column.partition_point(|covered| covered.bottom < span.top)..column.partition_point(|covered| covered.top <= span.bottom);
	if touching.is_empty() {
		insert(column, touching.start, span);
	} else {
		merge(column, touching, span);
	}
}

fn insert(column: &mut Column, index: usize, span: Span) {
	column.insert(index, span);
	if column.len() > SPANS_PER_COLUMN {
		close_smallest_gap(column);
	}
}

fn merge(column: &mut Column, touching: Range<usize>, span: Span) {
	column[touching.start] = Span {
		top: span.top.min(column[touching.start].top),
		bottom: span.bottom.max(column[touching.end - 1].bottom),
	};
	column.drain(touching.start + 1..touching.end);
}

fn close_smallest_gap(column: &mut Column) {
	let Some(above_gap) = column.windows(2).enumerate().min_by_key(|(_, pair)| pair[1].top - pair[0].bottom).map(|(index, _)| index) else {
		return;
	};
	column[above_gap].bottom = column[above_gap + 1].bottom;
	column.remove(above_gap + 1);
}
