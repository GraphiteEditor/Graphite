use super::misc::{ArcType, AsU64, GridType};
use super::{PointId, SegmentId, StrokeId};
use crate::Ctx;
use crate::registry::types::{Angle, PixelSize};
use crate::subpath;
use crate::table::Table;
use crate::vector::Vector;
use crate::vector::misc::{HandleId, SpiralType};
use glam::DVec2;

trait CornerRadius {
	fn generate(self, size: DVec2, clamped: bool) -> Table<Vector>;
}
impl CornerRadius for f64 {
	fn generate(self, size: DVec2, clamped: bool) -> Table<Vector> {
		let clamped_radius = if clamped { self.clamp(0., size.x.min(size.y).max(0.) / 2.) } else { self };
		Table::new_from_element(Vector::from_subpath(subpath::Subpath::new_rounded_rect(size / -2., size / 2., [clamped_radius; 4])))
	}
}
impl CornerRadius for [f64; 4] {
	fn generate(self, size: DVec2, clamped: bool) -> Table<Vector> {
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
		Table::new_from_element(Vector::from_subpath(subpath::Subpath::new_rounded_rect(size / -2., size / 2., clamped_radius)))
	}
}

#[node_macro::node(category("Vector: Shape"))]
fn circle(
	_: impl Ctx,
	_primary: (),
	#[unit(" px")]
	#[default(50.)]
	radius: f64,
) -> Table<Vector> {
	let radius = radius.abs();
	Table::new_from_element(Vector::from_subpath(subpath::Subpath::new_ellipse(DVec2::splat(-radius), DVec2::splat(radius))))
}

#[node_macro::node(category("Vector: Shape"))]
fn arc(
	_: impl Ctx,
	_primary: (),
	#[unit(" px")]
	#[default(50.)]
	radius: f64,
	start_angle: Angle,
	#[default(270.)]
	#[range((0., 360.))]
	sweep_angle: Angle,
	arc_type: ArcType,
) -> Table<Vector> {
	Table::new_from_element(Vector::from_subpath(subpath::Subpath::new_arc(
		radius,
		start_angle / 360. * std::f64::consts::TAU,
		sweep_angle / 360. * std::f64::consts::TAU,
		match arc_type {
			ArcType::Open => subpath::ArcType::Open,
			ArcType::Closed => subpath::ArcType::Closed,
			ArcType::PieSlice => subpath::ArcType::PieSlice,
		},
	)))
}

#[node_macro::node(category("Vector: Shape"), properties("spiral_properties"))]
fn spiral(
	_: impl Ctx,
	_primary: (),
	spiral_type: SpiralType,
	#[default(5.)] turns: f64,
	#[default(0.)] start_angle: f64,
	#[default(0.)] inner_radius: f64,
	#[default(25)] outer_radius: f64,
	#[default(90.)] angular_resolution: f64,
) -> Table<Vector> {
	Table::new_from_element(Vector::from_subpath(subpath::Subpath::new_spiral(
		inner_radius,
		outer_radius,
		turns,
		start_angle.to_radians(),
		angular_resolution.to_radians(),
		spiral_type,
	)))
}

#[node_macro::node(category("Vector: Shape"))]
fn ellipse(
	_: impl Ctx,
	_primary: (),
	#[unit(" px")]
	#[default(50)]
	radius_x: f64,
	#[unit(" px")]
	#[default(25)]
	radius_y: f64,
) -> Table<Vector> {
	let radius = DVec2::new(radius_x, radius_y);
	let corner1 = -radius;
	let corner2 = radius;

	let mut ellipse = Vector::from_subpath(subpath::Subpath::new_ellipse(corner1, corner2));

	let len = ellipse.segment_domain.ids().len();
	for i in 0..len {
		ellipse
			.colinear_manipulators
			.push([HandleId::end(ellipse.segment_domain.ids()[i]), HandleId::primary(ellipse.segment_domain.ids()[(i + 1) % len])]);
	}

	Table::new_from_element(ellipse)
}

#[node_macro::node(category("Vector: Shape"), properties("rectangle_properties"))]
fn rectangle<T: CornerRadius>(
	_: impl Ctx,
	_primary: (),
	#[unit(" px")]
	#[default(100)]
	width: f64,
	#[unit(" px")]
	#[default(100)]
	height: f64,
	_individual_corner_radii: bool, // TODO: Move this to the bottom once we have a migration capability
	#[implementations(f64, [f64; 4])] corner_radius: T,
	#[default(true)] clamped: bool,
) -> Table<Vector> {
	corner_radius.generate(DVec2::new(width, height), clamped)
}

#[node_macro::node(category("Vector: Shape"))]
fn regular_polygon<T: AsU64>(
	_: impl Ctx,
	_primary: (),
	#[default(6)]
	#[hard_min(3.)]
	#[implementations(u32, u64, f64)]
	sides: T,
	#[unit(" px")]
	#[default(50)]
	radius: f64,
) -> Table<Vector> {
	let points = sides.as_u64();
	let radius: f64 = radius * 2.;
	Table::new_from_element(Vector::from_subpath(subpath::Subpath::new_regular_polygon(DVec2::splat(-radius), points, radius)))
}

#[node_macro::node(category("Vector: Shape"))]
fn star<T: AsU64>(
	_: impl Ctx,
	_primary: (),
	#[default(5)]
	#[hard_min(2.)]
	#[implementations(u32, u64, f64)]
	sides: T,
	#[unit(" px")]
	#[default(50)]
	radius_1: f64,
	#[unit(" px")]
	#[default(25)]
	radius_2: f64,
) -> Table<Vector> {
	let points = sides.as_u64();
	let diameter: f64 = radius_1 * 2.;
	let inner_diameter = radius_2 * 2.;

	Table::new_from_element(Vector::from_subpath(subpath::Subpath::new_star_polygon(DVec2::splat(-diameter), points, diameter, inner_diameter)))
}

#[node_macro::node(category("Vector: Shape"))]
fn line(_: impl Ctx, _primary: (), #[default(0., 0.)] start: PixelSize, #[default(100., 100.)] end: PixelSize) -> Table<Vector> {
	Table::new_from_element(Vector::from_subpath(subpath::Subpath::new_line(start, end)))
}

trait GridSpacing {
	fn as_dvec2(&self) -> DVec2;
}
impl GridSpacing for f64 {
	fn as_dvec2(&self) -> DVec2 {
		DVec2::splat(*self)
	}
}
impl GridSpacing for DVec2 {
	fn as_dvec2(&self) -> DVec2 {
		*self
	}
}

#[node_macro::node(category("Vector: Shape"), properties("grid_properties"))]
fn grid<T: GridSpacing>(
	_: impl Ctx,
	_primary: (),
	grid_type: GridType,
	#[unit(" px")]
	#[hard_min(0.)]
	#[default(10)]
	#[implementations(f64, DVec2)]
	spacing: T,
	#[default(10)] columns: u32,
	#[default(10)] rows: u32,
	#[default(30., 30.)] angles: DVec2,
) -> Table<Vector> {
	let (x_spacing, y_spacing) = spacing.as_dvec2().into();
	let (angle_a, angle_b) = angles.into();

	let mut vector = Vector::default();
	let mut segment_id = SegmentId::ZERO;
	let mut point_id = PointId::ZERO;

	match grid_type {
		GridType::Rectangular => {
			// Create rectangular grid points and connect them with line segments
			for y in 0..rows {
				for x in 0..columns {
					// Add current point to the grid
					let current_index = vector.point_domain.ids().len();
					vector.point_domain.push(point_id.next_id(), DVec2::new(x_spacing * x as f64, y_spacing * y as f64));

					// Helper function to connect points with line segments
					let mut push_segment = |to_index: Option<usize>| {
						if let Some(other_index) = to_index {
							vector
								.segment_domain
								.push(segment_id.next_id(), other_index, current_index, subpath::BezierHandles::Linear, StrokeId::ZERO);
						}
					};

					// Connect to the point to the left (horizontal connection)
					push_segment((x > 0).then(|| current_index - 1));

					// Connect to the point above (vertical connection)
					push_segment(current_index.checked_sub(columns as usize));
				}
			}
		}
		GridType::Isometric => {
			// Calculate isometric grid spacing based on angles
			let tan_a = angle_a.to_radians().tan();
			let tan_b = angle_b.to_radians().tan();
			let spacing = DVec2::new(y_spacing / (tan_a + tan_b), y_spacing);

			// Create isometric grid points and connect them with line segments
			for y in 0..rows {
				for x in 0..columns {
					// Add current point to the grid with offset for odd columns
					let current_index = vector.point_domain.ids().len();

					let a_angles_eaten = x.div_ceil(2) as f64;
					let b_angles_eaten = (x / 2) as f64;

					let offset_y_fraction = b_angles_eaten * tan_b - a_angles_eaten * tan_a;

					let position = DVec2::new(spacing.x * x as f64, spacing.y * y as f64 + offset_y_fraction * spacing.x);
					vector.point_domain.push(point_id.next_id(), position);

					// Helper function to connect points with line segments
					let mut push_segment = |to_index: Option<usize>| {
						if let Some(other_index) = to_index {
							vector
								.segment_domain
								.push(segment_id.next_id(), other_index, current_index, subpath::BezierHandles::Linear, StrokeId::ZERO);
						}
					};

					// Connect to the point to the left
					push_segment((x > 0).then(|| current_index - 1));

					// Connect to the point directly above
					push_segment(current_index.checked_sub(columns as usize));

					// Additional diagonal connections for odd columns (creates hexagonal pattern)
					if x % 2 == 1 {
						// Connect to the point diagonally up-right (if not at right edge)
						push_segment(current_index.checked_sub(columns as usize - 1).filter(|_| x + 1 < columns));

						// Connect to the point diagonally up-left
						push_segment(current_index.checked_sub(columns as usize + 1));
					}
				}
			}
		}
	}

	Table::new_from_element(vector)
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn isometric_grid_test() {
		// Doesn't crash with weird angles
		grid((), (), GridType::Isometric, 0., 5, 5, (0., 0.).into());
		grid((), (), GridType::Isometric, 90., 5, 5, (90., 90.).into());

		// Works properly
		let grid = grid((), (), GridType::Isometric, 10., 5, 5, (30., 30.).into());
		assert_eq!(grid.iter().next().unwrap().element.point_domain.ids().len(), 5 * 5);
		assert_eq!(grid.iter().next().unwrap().element.segment_bezier_iter().count(), 4 * 5 + 4 * 9);
		for (_, bezier, _, _) in grid.iter().next().unwrap().element.segment_bezier_iter() {
			assert_eq!(bezier.handles, subpath::BezierHandles::Linear);
			assert!(
				((bezier.start - bezier.end).length() - 10.).abs() < 1e-5,
				"Length of {} should be 10",
				(bezier.start - bezier.end).length()
			);
		}
	}

	#[test]
	fn skew_isometric_grid_test() {
		let grid = grid((), (), GridType::Isometric, 10., 5, 5, (40., 30.).into());
		assert_eq!(grid.iter().next().unwrap().element.point_domain.ids().len(), 5 * 5);
		assert_eq!(grid.iter().next().unwrap().element.segment_bezier_iter().count(), 4 * 5 + 4 * 9);
		for (_, bezier, _, _) in grid.iter().next().unwrap().element.segment_bezier_iter() {
			assert_eq!(bezier.handles, subpath::BezierHandles::Linear);
			let vector = bezier.start - bezier.end;
			let angle = (vector.angle_to(DVec2::X).to_degrees() + 180.) % 180.;
			assert!([90., 150., 40.].into_iter().any(|target| (target - angle).abs() < 1e-10), "unexpected angle of {angle}")
		}
	}
}
