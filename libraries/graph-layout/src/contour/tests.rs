use super::column::{self, Column, Span};
use super::{ColumnPool, Contour};
use crate::Rect;
use glam::IVec2;

fn rectangle(min: IVec2, max: IVec2) -> Contour {
	let mut contour = Contour::default();
	contour.cover_rect(Rect::new(min, max), &mut ColumnPool::default());
	contour
}

fn union(first: Contour, second: Contour) -> Contour {
	first.union(second, &mut ColumnPool::default())
}

#[test]
fn an_outline_fits_into_the_gap_of_another() {
	let upside_down_l = union(rectangle(IVec2::ZERO, IVec2::new(2, 6)), rectangle(IVec2::ZERO, IVec2::new(8, 1)));

	let under_the_roof = rectangle(IVec2::new(4, 0), IVec2::new(7, 2));
	assert_eq!(under_the_roof.clearance_below(&upside_down_l, IVec2::ZERO), 1);

	let across_the_column = rectangle(IVec2::new(1, 0), IVec2::new(4, 2));
	assert_eq!(across_the_column.clearance_below(&upside_down_l, IVec2::ZERO), 6);
}

#[test]
fn an_outline_fits_into_a_gap_between_rows_of_another() {
	let bars = union(rectangle(IVec2::ZERO, IVec2::new(8, 1)), rectangle(IVec2::new(0, 5), IVec2::new(8, 6)));

	let fitting = rectangle(IVec2::new(2, 0), IVec2::new(5, 3));
	assert_eq!(fitting.clearance_below(&bars, IVec2::ZERO), 1);

	let too_tall = rectangle(IVec2::new(2, 0), IVec2::new(5, 5));
	assert_eq!(too_tall.clearance_below(&bars, IVec2::ZERO), 6);

	let wider_than_the_bars = rectangle(IVec2::new(-4, 0), IVec2::new(12, 5));
	assert_eq!(wider_than_the_bars.clearance_below(&bars, IVec2::ZERO), 6);
}

#[test]
fn covering_merges_touching_spans() {
	let span = |top, bottom| Span { top, bottom };
	let mut covered = Column::new();
	for added in [span(0, 2), span(5, 6), span(9, 10)] {
		column::cover(&mut covered, added);
	}
	assert_eq!(covered.as_slice(), &[span(0, 2), span(5, 6), span(9, 10)]);

	column::cover(&mut covered, span(2, 5));
	assert_eq!(covered.as_slice(), &[span(0, 6), span(9, 10)]);
}

#[test]
fn clearing_left_only_counts_columns_near_the_rect() {
	let reader = Rect::new(IVec2::ZERO, IVec2::new(5, 2));
	let gap = IVec2::new(2, 1);

	assert_eq!(rectangle(IVec2::new(-2, 1), IVec2::new(3, 2)).clearance_left_of_rect(reader, gap), 5);
	assert_eq!(rectangle(IVec2::new(-2, 9), IVec2::new(3, 10)).clearance_left_of_rect(reader, gap), 0);
	assert_eq!(rectangle(IVec2::new(7, 0), IVec2::new(12, 1)).clearance_left_of_rect(reader, gap), 0);
}
