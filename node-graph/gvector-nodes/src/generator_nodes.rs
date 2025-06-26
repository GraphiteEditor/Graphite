use super::misc::{ArcType, AsU64, GridType};
use bezier_rs::Subpath;
use glam::DVec2;
use graphene_core::context::Ctx;
use graphene_core::registry::types::{Angle, PixelSize};
use graphene_vector::{HandleId, PointId, SegmentId, StrokeId, VectorData, VectorDataTable};

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
	let radius = radius.abs();
	VectorDataTable::new(VectorData::from_subpath(Subpath::new_ellipse(DVec2::splat(-radius), DVec2::splat(radius))))
}

#[node_macro::node(category("Vector: Shape"))]
fn arc(
	_: impl Ctx,
	_primary: (),
	#[default(50.)] radius: f64,
	start_angle: Angle,
	#[default(270.)]
	#[range((0., 360.))]
	sweep_angle: Angle,
	arc_type: ArcType,
) -> VectorDataTable {
	VectorDataTable::new(VectorData::from_subpath(Subpath::new_arc(
		radius,
		start_angle / 360. * std::f64::consts::TAU,
		sweep_angle / 360. * std::f64::consts::TAU,
		match arc_type {
			ArcType::Open => bezier_rs::ArcType::Open,
			ArcType::Closed => bezier_rs::ArcType::Closed,
			ArcType::PieSlice => bezier_rs::ArcType::PieSlice,
		},
	)))
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
	#[hard_min(3.)]
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
	#[hard_min(2.)]
	#[implementations(u32, u64, f64)]
	sides: T,
	#[default(50)] radius_1: f64,
	#[default(25)] radius_2: f64,
) -> VectorDataTable {
	let points = sides.as_u64();
	let diameter: f64 = radius_1 * 2.;
	let inner_diameter = radius_2 * 2.;

	VectorDataTable::new(VectorData::from_subpath(Subpath::new_star_polygon(DVec2::splat(-diameter), points, diameter, inner_diameter)))
}

#[node_macro::node(category("Vector: Shape"))]
fn line(_: impl Ctx, _primary: (), #[default((0., -50.))] start: PixelSize, #[default((0., 50.))] end: PixelSize) -> VectorDataTable {
	VectorDataTable::new(VectorData::from_subpath(Subpath::new_line(start, end)))
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
	#[hard_min(0.)]
	#[default(10)]
	#[implementations(f64, DVec2)]
	spacing: T,
	#[default(30., 30.)] angles: DVec2,
	#[default(10)] columns: u32,
	#[default(10)] rows: u32,
) -> VectorDataTable {
	let (x_spacing, y_spacing) = spacing.as_dvec2().into();
	let (angle_a, angle_b) = angles.into();

	let mut vector_data = VectorData::default();
	let mut segment_id = SegmentId::ZERO;
	let mut point_id = PointId::ZERO;

	match grid_type {
		GridType::Rectangular => {
			// Create rectangular grid points and connect them with line segments
			for y in 0..rows {
				for x in 0..columns {
					// Add current point to the grid
					let current_index = vector_data.point_domain.ids().len();
					vector_data.point_domain.push(point_id.next_id(), DVec2::new(x_spacing * x as f64, y_spacing * y as f64));

					// Helper function to connect points with line segments
					let mut push_segment = |to_index: Option<usize>| {
						if let Some(other_index) = to_index {
							vector_data
								.segment_domain
								.push(segment_id.next_id(), other_index, current_index, bezier_rs::BezierHandles::Linear, StrokeId::ZERO);
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
					let current_index = vector_data.point_domain.ids().len();

					let a_angles_eaten = x.div_ceil(2) as f64;
					let b_angles_eaten = (x / 2) as f64;

					let offset_y_fraction = b_angles_eaten * tan_b - a_angles_eaten * tan_a;

					let position = DVec2::new(spacing.x * x as f64, spacing.y * y as f64 + offset_y_fraction * spacing.x);
					vector_data.point_domain.push(point_id.next_id(), position);

					// Helper function to connect points with line segments
					let mut push_segment = |to_index: Option<usize>| {
						if let Some(other_index) = to_index {
							vector_data
								.segment_domain
								.push(segment_id.next_id(), other_index, current_index, bezier_rs::BezierHandles::Linear, StrokeId::ZERO);
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

	VectorDataTable::new(vector_data)
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn isometric_grid_test() {
		// Doesn't crash with weird angles
		grid((), (), GridType::Isometric, 0., (0., 0.).into(), 5, 5);
		grid((), (), GridType::Isometric, 90., (90., 90.).into(), 5, 5);

		// Works properly
		let grid = grid((), (), GridType::Isometric, 10., (30., 30.).into(), 5, 5);
		assert_eq!(grid.instance_ref_iter().next().unwrap().instance.point_domain.ids().len(), 5 * 5);
		assert_eq!(grid.instance_ref_iter().next().unwrap().instance.segment_bezier_iter().count(), 4 * 5 + 4 * 9);
		for (_, bezier, _, _) in grid.instance_ref_iter().next().unwrap().instance.segment_bezier_iter() {
			assert_eq!(bezier.handles, bezier_rs::BezierHandles::Linear);
			assert!(
				((bezier.start - bezier.end).length() - 10.).abs() < 1e-5,
				"Length of {} should be 10",
				(bezier.start - bezier.end).length()
			);
		}
	}

	#[test]
	fn skew_isometric_grid_test() {
		let grid = grid((), (), GridType::Isometric, 10., (40., 30.).into(), 5, 5);
		assert_eq!(grid.instance_ref_iter().next().unwrap().instance.point_domain.ids().len(), 5 * 5);
		assert_eq!(grid.instance_ref_iter().next().unwrap().instance.segment_bezier_iter().count(), 4 * 5 + 4 * 9);
		for (_, bezier, _, _) in grid.instance_ref_iter().next().unwrap().instance.segment_bezier_iter() {
			assert_eq!(bezier.handles, bezier_rs::BezierHandles::Linear);
			let vector = bezier.start - bezier.end;
			let angle = (vector.angle_to(DVec2::X).to_degrees() + 180.) % 180.;
			assert!([90., 150., 40.].into_iter().any(|target| (target - angle).abs() < 1e-10), "unexpected angle of {}", angle)
		}
	}
}
