use core_types::registry::types::{Angle, PixelSize};
use core_types::table::Table;
use core_types::{Ctx, specta};
use dyn_any::DynAny;
use glam::DVec2;
use graphic_types::Vector;
use vector_types::subpath;
use vector_types::vector::misc::{ArcType, AsU64, GridType};
use vector_types::vector::misc::{HandleId, SpiralType};
use vector_types::vector::{PointId, SegmentId, StrokeId};

trait CornerRadius {
	fn generate(self, size: DVec2, clamped: bool) -> Table<Vector>;
}
impl CornerRadius for f64 {
	fn generate(self, size: DVec2, clamped: bool) -> Table<Vector> {
		let clamped_radius = if clamped { self.clamp(0., size.x.min(size.y).max(0.) / 2.) } else { self };
		Table::new_from_element(Vector::from_subpath(subpath::Subpath::new_rounded_rectangle(size / -2., size / 2., [clamped_radius; 4])))
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
		Table::new_from_element(Vector::from_subpath(subpath::Subpath::new_rounded_rectangle(size / -2., size / 2., clamped_radius)))
	}
}

/// Generates a circle shape with a chosen radius.
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

/// Generates an arc shape forming a portion of a circle which may be open, closed, or a pie slice.
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

/// Generates a spiral shape that winds from an inner to an outer radius.
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

/// Generates an ellipse shape (an oval or stretched circle) with the chosen radii.
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

/// Generates a rectangle shape with the chosen width and height. It may also have rounded corners if desired.
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

/// Generates a heart shape with adjustable proportions.
#[node_macro::node(category("Vector: Shape"))]
fn heart(
	_: impl Ctx,
	_primary: (),
	#[unit(" px")]
	#[default(50.)]
	radius: f64,
	#[default(30.)] cleft_angle: f64,
	#[default(45.)] tip_angle: f64,
	#[default(0.4)] cleft_depth: f64,
	#[default(0.9)] tip_depth: f64,
	#[default(0.)] left_bulb_height: f64,
	#[default(0.)] right_bulb_height: f64,
	#[default(0.)] left_bulb_expand: f64,
	#[default(0.)] right_bulb_expand: f64,
) -> Table<Vector> {
	Table::new_from_element(Vector::from_subpath(subpath::Subpath::new_heart(
		DVec2::splat(0.),
		radius,
		cleft_angle.to_radians(),
		tip_angle.to_radians(),
		left_bulb_height,
		right_bulb_height,
		left_bulb_expand,
		right_bulb_expand,
		cleft_depth,
		tip_depth,
	)))
}

/// Generates an regular polygon shape like a triangle, square, pentagon, hexagon, heptagon, octagon, or any higher n-gon.
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

/// Generates an n-pointed star shape with inner and outer points at chosen radii from the center.
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

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash, DynAny, specta::Type, node_macro::ChoiceType)]
#[widget(Radio)]
pub enum QRCodeErrorCorrectionLevel {
	/// Allows recovery from up to 7% data loss.
	#[default]
	Low,
	/// Allows recovery from up to 15% data loss.
	Medium,
	/// Allows recovery from up to 25% data loss.
	Quartile,
	/// Allows recovery from up to 30% data loss.
	High,
}

/// Generates a QR code from the input text.
#[node_macro::node(category("Vector: Shape"), name("QR Code"))]
fn qr_code(
	_: impl Ctx,
	_primary: (),
	#[widget(ParsedWidgetOverride::Custom = "text_area")]
	#[default("https://graphite.art")]
	text: String,
	#[widget(ParsedWidgetOverride::Hidden)] has_size: bool,
	#[unit(" px")]
	#[hard_min(1.)]
	#[widget(ParsedWidgetOverride::Custom = "optional_f64")]
	size: f64,
	error_correction: QRCodeErrorCorrectionLevel,
	#[default(false)] individual_squares: bool,
) -> Table<Vector> {
	let ecc = match error_correction {
		QRCodeErrorCorrectionLevel::Low => qrcodegen::QrCodeEcc::Low,
		QRCodeErrorCorrectionLevel::Medium => qrcodegen::QrCodeEcc::Medium,
		QRCodeErrorCorrectionLevel::Quartile => qrcodegen::QrCodeEcc::Quartile,
		QRCodeErrorCorrectionLevel::High => qrcodegen::QrCodeEcc::High,
	};

	let Ok(qr_code) = qrcodegen::QrCode::encode_text(&text, ecc) else { return Table::default() };

	let mut vector = match individual_squares {
		true => {
			let mut vector = Vector::default();

			let dimension = qr_code.size() as usize;
			for y in 0..dimension {
				for x in 0..dimension {
					if qr_code.get_module(x as i32, y as i32) {
						let corner1 = DVec2::new(x as f64, y as f64);
						let corner2 = corner1 + DVec2::splat(1.);
						vector.append_subpath(
							subpath::Subpath::from_anchors([corner1, DVec2::new(corner2.x, corner1.y), corner2, DVec2::new(corner1.x, corner2.y)], true),
							false,
						);
					}
				}
			}

			vector
		}
		false => crate::merge_qr_squares::merge_qr_squares(&qr_code),
	};

	if has_size {
		vector.transform(glam::DAffine2::from_scale(DVec2::splat(size.max(1.) / qr_code.size() as f64)));
	}

	Table::new_from_element(vector)
}

/// Generates a line with endpoints at the two chosen coordinates.
#[node_macro::node(category("Vector: Shape"))]
fn arrow(
	_: impl Ctx,
	_primary: (),
	#[default(0., 0.)] start: PixelSize,
	#[default(100., 0.)] end: PixelSize,
	#[unit(" px")]
	#[default(10)]
	shaft_width: f64,
	#[unit(" px")]
	#[default(30)]
	head_width: f64,
	#[unit(" px")]
	#[default(20)]
	head_length: f64,
) -> Table<Vector> {
	Table::new_from_element(Vector::from_subpath(subpath::Subpath::new_arrow(start, end, shaft_width, head_width, head_length)))
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

/// Generates a rectangular or isometric grid with the chosen number of columns and rows. Line segments connect the points, forming a vector mesh.
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

	#[test]
	fn qr_code_test() {
		let qr = qr_code((), (), "https://graphite.art".to_string(), false, 1., QRCodeErrorCorrectionLevel::Low, true);
		assert!(qr.iter().next().unwrap().element.point_domain.ids().len() > 0);
		assert!(qr.iter().next().unwrap().element.segment_domain.ids().len() > 0);
	}
}
