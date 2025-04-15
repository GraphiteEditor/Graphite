use crate::Ctx;
use crate::vector::{HandleId, VectorData, VectorDataTable};
use bezier_rs::Subpath;
use glam::DVec2;

use super::misc::AsU64;
use super::{PointId, SegmentId, StrokeId};

trait CornerRadius {
	fn generate(self, size: DVec2, clamped: bool) -> VectorDataTable;
}
impl CornerRadius for f64 {
	fn generate(self, size: DVec2, clamped: bool) -> VectorDataTable {
		let clamped_radius = if clamped { self.clamp(0., size.x.min(size.y).max(0.) / 2.) } else { self };
		VectorDataTable::new(VectorData::from_subpath(Subpath::new_rounded_rect(size / -2., size / 2., [clamped_radius; 4])))
	}
}
impl CornerRadius for [f64; 4] {
	fn generate(self, size: DVec2, clamped: bool) -> VectorDataTable {
		let clamped_radius = if clamped {
			// Algorithm follows the CSS spec: <https://drafts.csswg.org/css-backgrounds/#corner-overlap>

			let mut scale_factor: f64 = 1.;
			for i in 0..4 {
				let side_length = if i % 2 == 0 { size.x } else { size.y };
				let adjacent_corner_radius_sum = self[i] + self[(i + 1) % 4];
				if side_length < adjacent_corner_radius_sum {
					scale_factor = scale_factor.min(side_length / adjacent_corner_radius_sum);
				}
			}
			self.map(|x| x * scale_factor)
		} else {
			self
		};
		VectorDataTable::new(VectorData::from_subpath(Subpath::new_rounded_rect(size / -2., size / 2., clamped_radius)))
	}
}

#[node_macro::node(category("Vector: Shape"))]
fn circle(_: impl Ctx, _primary: (), #[default(50.)] radius: f64) -> VectorDataTable {
	VectorDataTable::new(VectorData::from_subpath(Subpath::new_ellipse(DVec2::splat(-radius), DVec2::splat(radius))))
}

#[node_macro::node(category("Vector: Shape"))]
fn ellipse(_: impl Ctx, _primary: (), #[default(50)] radius_x: f64, #[default(25)] radius_y: f64) -> VectorDataTable {
	let radius = DVec2::new(radius_x, radius_y);
	let corner1 = -radius;
	let corner2 = radius;

	let mut ellipse = VectorData::from_subpath(Subpath::new_ellipse(corner1, corner2));

	let len = ellipse.segment_domain.ids().len();
	for i in 0..len {
		ellipse
			.colinear_manipulators
			.push([HandleId::end(ellipse.segment_domain.ids()[i]), HandleId::primary(ellipse.segment_domain.ids()[(i + 1) % len])]);
	}

	VectorDataTable::new(ellipse)
}

#[node_macro::node(category("Vector: Shape"), properties("rectangle_properties"))]
fn rectangle<T: CornerRadius>(
	_: impl Ctx,
	_primary: (),
	#[default(100)] width: f64,
	#[default(100)] height: f64,
	_individual_corner_radii: bool, // TODO: Move this to the bottom once we have a migration capability
	#[implementations(f64, [f64; 4])] corner_radius: T,
	#[default(true)] clamped: bool,
) -> VectorDataTable {
	corner_radius.generate(DVec2::new(width, height), clamped)
}

#[node_macro::node(category("Vector: Shape"))]
fn regular_polygon<T: AsU64>(
	_: impl Ctx,
	_primary: (),
	#[default(6)]
	#[min(3.)]
	#[implementations(u32, u64, f64)]
	sides: T,
	#[default(50)] radius: f64,
) -> VectorDataTable {
	let points = sides.as_u64();
	let radius: f64 = radius * 2.;
	VectorDataTable::new(VectorData::from_subpath(Subpath::new_regular_polygon(DVec2::splat(-radius), points, radius)))
}

#[node_macro::node(category("Vector: Shape"))]
fn star<T: AsU64>(
	_: impl Ctx,
	_primary: (),
	#[default(5)]
	#[min(2.)]
	#[implementations(u32, u64, f64)]
	sides: T,
	#[default(50)] radius: f64,
	#[default(25)] inner_radius: f64,
) -> VectorDataTable {
	let points = sides.as_u64();
	let diameter: f64 = radius * 2.;
	let inner_diameter = inner_radius * 2.;

	VectorDataTable::new(VectorData::from_subpath(Subpath::new_star_polygon(DVec2::splat(-diameter), points, diameter, inner_diameter)))
}

#[node_macro::node(category("Vector: Shape"))]
fn line(_: impl Ctx, _primary: (), #[default((0., -50.))] start: DVec2, #[default((0., 50.))] end: DVec2) -> VectorDataTable {
	VectorDataTable::new(VectorData::from_subpath(Subpath::new_line(start, end)))
}

#[node_macro::node(category("Vector: Shape"))]
fn isometric_grid(
	_: impl Ctx,
	_primary: (),
	#[min(0.)]
	#[default(30)]
	#[max(90.)]
	angle_a: f64,
	#[min(0.)]
	#[default(10)]
	y_axis_spacing: f64,
	#[default(10)] rows: u32,
	#[default(10)] columns: u32,
) -> VectorDataTable {
	let tan_a = angle_a.to_radians().tan();
	let spacing = DVec2::new(y_axis_spacing / (tan_a * 2.), y_axis_spacing);
	let mut vector_data = VectorData::empty();
	let mut segment_id = SegmentId::ZERO;
	let mut point_id = PointId::ZERO;
	for y in 0..rows {
		for x in 0..columns {
			let current_index = vector_data.point_domain.ids().len();
			vector_data
				.point_domain
				.push(point_id.next_id(), DVec2::new(spacing.x * x as f64, spacing.y * (y as f64 - (x % 2) as f64 * 0.5)));

			let mut push_segment = |to_index: Option<usize>| {
				if let Some(other_index) = to_index {
					vector_data
						.segment_domain
						.push(segment_id.next_id(), other_index, current_index, bezier_rs::BezierHandles::Linear, StrokeId::ZERO);
				}
			};

			push_segment((x > 0).then(|| current_index - 1));
			push_segment(current_index.checked_sub(columns as usize));
			if x % 2 == 1 {
				push_segment(current_index.checked_sub(columns as usize - 1).filter(|_| x + 1 < columns));
				push_segment(current_index.checked_sub(columns as usize + 1));
			}
		}
	}

	VectorDataTable::new(vector_data)
}

#[test]
fn isometric_grid_test() {
	// Doesn't crash with weird angles
	isometric_grid((), (), 0., 0., 5, 5);
	isometric_grid((), (), 90., 90., 5, 5);

	// Works properly
	let grid = isometric_grid((), (), 30., 10., 5, 5);
	assert_eq!(grid.one_instance().instance.point_domain.ids().len(), 5 * 5);
	assert_eq!(grid.one_instance().instance.segment_bezier_iter().count(), 4 * 5 + 4 * 9);
	for (_, bezier, _, _) in grid.one_instance().instance.segment_bezier_iter() {
		assert_eq!(bezier.handles, bezier_rs::BezierHandles::Linear);
		assert!(
			((bezier.start - bezier.end).length() - 10.).abs() < 1e-5,
			"Length of {} should be 10",
			(bezier.start - bezier.end).length()
		);
	}
}
