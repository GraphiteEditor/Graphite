use core_types::registry::types::{Angle, PixelLength, PixelSize};
use core_types::{CacheHash, Ctx};
use dyn_any::DynAny;
use glam::DVec2;
use graphic_types::Vector;
use vector_types::vector::VectorExt;
use vector_types::vector::algorithms::shapes;
use vector_types::vector::misc::BezierHandles;
use vector_types::vector::misc::{ArcType, AsU64, BoxCorners, GridType};
use vector_types::vector::misc::{HandleId, SpiralType};
use vector_types::vector::{PointId, SegmentId};

/// Generates a circle shape with a chosen radius.
#[node_macro::node(category("Vector: Shape"))]
fn circle(
	_: impl Ctx,
	_primary: (),
	#[unit(" px")]
	#[default(50.)]
	radius: f64,
) -> Vector {
	let radius = radius.abs();
	Vector::from_bezpath(shapes::ellipse_bezpath(DVec2::splat(-radius), DVec2::splat(radius)))
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
	#[range]
	#[soft(0..360)]
	sweep_angle: Angle,
	arc_type: ArcType,
) -> Vector {
	Vector::from_bezpath(shapes::arc_bezpath(
		radius,
		start_angle / 360. * std::f64::consts::TAU,
		sweep_angle / 360. * std::f64::consts::TAU,
		arc_type,
	))
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
) -> Vector {
	Vector::from_bezpath(shapes::spiral_bezpath(
		inner_radius,
		outer_radius,
		turns,
		start_angle.to_radians(),
		angular_resolution.to_radians(),
		spiral_type,
	))
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
) -> Vector {
	let radius = DVec2::new(radius_x, radius_y);
	let corner1 = -radius;
	let corner2 = radius;

	let mut ellipse = Vector::from_bezpath(shapes::ellipse_bezpath(corner1, corner2));

	let len = ellipse.segment_domain.ids().len();
	for i in 0..len {
		ellipse
			.colinear_manipulators
			.push([HandleId::end(ellipse.segment_domain.ids()[i]), HandleId::primary(ellipse.segment_domain.ids()[(i + 1) % len])]);
	}

	ellipse
}

/// Generates a rectangle shape with the chosen width and height. It may also have rounded corners if desired.
#[node_macro::node(category("Vector: Shape"), properties("rectangle_properties"))]
fn rectangle(
	_: impl Ctx,
	_primary: (),
	#[unit(" px")]
	#[default(100)]
	width: f64,
	#[unit(" px")]
	#[default(100)]
	height: f64,
	corner_radius: BoxCorners,
	#[default(true)] clamped: bool,
	_individual_corner_radii: bool,
) -> Vector {
	let size = DVec2::new(width, height);
	let radii = corner_radius.to_corner_values();

	// Scale down overlapping adjacent radii to fit, following the CSS spec: <https://drafts.csswg.org/css-backgrounds/#corner-overlap>
	let radii = if clamped {
		let radii = radii.map(|radius| radius.max(0.));

		let mut scale_factor: f64 = 1.;
		for i in 0..4 {
			let side_length = if i % 2 == 0 { size.x } else { size.y };
			let adjacent_corner_radius_sum = radii[i] + radii[(i + 1) % 4];
			if side_length < adjacent_corner_radius_sum {
				scale_factor = scale_factor.min((side_length / adjacent_corner_radius_sum).max(0.));
			}
		}

		radii.map(|radius| radius * scale_factor)
	} else {
		radii
	};

	Vector::from_bezpath(shapes::rounded_rectangle_bezpath(size / -2., size / 2., radii))
}

/// Builds a set of four corner values, such as a rectangle's corner radii, from a list of one, two, three, or four values.
#[node_macro::node(category("Vector: Shape"))]
fn box_corners(
	_: impl Ctx,
	/// The corner values, filling the four corners clockwise from the top-left. Give one value for all corners, two for opposite pairs, three for top-left, the two sides, then bottom-right, or four for each corner.
	values: IList<f64>,
) -> BoxCorners {
	let values: Vec<f64> = (0..values.len()).map(|index| values.get(index)).collect();
	BoxCorners::from(values)
}

/// Generates an regular polygon shape like a triangle, square, pentagon, hexagon, heptagon, octagon, or any higher n-gon.
#[node_macro::node(category("Vector: Shape"))]
fn regular_polygon<T: AsU64>(
	_: impl Ctx,
	_primary: (),
	#[default(6)]
	#[hard(3..)]
	#[implementations(u32, u64, f64)]
	sides: T,
	#[unit(" px")]
	#[default(50)]
	radius: f64,
) -> Vector {
	let points = sides.as_u64();
	Vector::from_bezpath(shapes::regular_polygon_bezpath(DVec2::ZERO, points, radius))
}

/// Generates an n-pointed star shape with inner and outer points at chosen radii from the center.
#[node_macro::node(category("Vector: Shape"))]
fn star<T: AsU64>(
	_: impl Ctx,
	_primary: (),
	#[default(5)]
	#[hard(2..)]
	#[implementations(u32, u64, f64)]
	sides: T,
	#[unit(" px")]
	#[default(50)]
	radius_1: f64,
	#[unit(" px")]
	#[default(25)]
	radius_2: f64,
) -> Vector {
	let points = sides.as_u64();
	Vector::from_bezpath(shapes::star_polygon_bezpath(DVec2::ZERO, points, radius_1, radius_2))
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, CacheHash, DynAny, node_macro::ChoiceType)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
	#[hard(1..)]
	#[widget(ParsedWidgetOverride::Custom = "optional_f64")]
	size: f64,
	error_correction: QRCodeErrorCorrectionLevel,
	#[default(false)] individual_squares: bool,
) -> Vector {
	let ecc = match error_correction {
		QRCodeErrorCorrectionLevel::Low => qrcodegen::QrCodeEcc::Low,
		QRCodeErrorCorrectionLevel::Medium => qrcodegen::QrCodeEcc::Medium,
		QRCodeErrorCorrectionLevel::Quartile => qrcodegen::QrCodeEcc::Quartile,
		QRCodeErrorCorrectionLevel::High => qrcodegen::QrCodeEcc::High,
	};

	let Ok(qr_code) = qrcodegen::QrCode::encode_text(&text, ecc) else { return Vector::default() };

	let mut vector = match individual_squares {
		true => {
			let mut vector = Vector::default();

			let dimension = qr_code.size() as usize;
			for y in 0..dimension {
				for x in 0..dimension {
					if qr_code.get_module(x as i32, y as i32) {
						let corner1 = DVec2::new(x as f64, y as f64);
						vector.append_bezpath(shapes::rectangle_bezpath(corner1, corner1 + DVec2::splat(1.)));
					}
				}
			}

			vector
		}
		false => crate::merge_qr_squares::merge_qr_squares(&qr_code),
	};

	if has_size {
		vector.transform(glam::DAffine2::from_scale(DVec2::splat(size / qr_code.size() as f64)));
	}

	vector
}

/// Generates an arrow from the origin to the chosen coordinate.
#[node_macro::node(category("Vector: Shape"))]
fn arrow(
	_: impl Ctx,
	_primary: (),
	#[default(100., 0.)] arrow_to: PixelSize,
	#[default(10)] shaft_width: PixelLength,
	#[default(30)] head_width: PixelLength,
	#[default(20)] head_length: PixelLength,
) -> Vector {
	Vector::from_bezpath(shapes::arrow_bezpath(DVec2::ZERO, arrow_to, shaft_width, head_width, head_length))
}

#[node_macro::node(category("Vector: Shape"))]
fn line(_: impl Ctx, _primary: (), #[default(100., 100.)] line_to: PixelSize) -> Vector {
	Vector::from_bezpath(shapes::line_bezpath(DVec2::ZERO, line_to))
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
	#[hard(0..)]
	#[default(10)]
	#[implementations(f64, DVec2)]
	spacing: T,
	#[default(10)] columns: u32,
	#[default(10)] rows: u32,
	#[default(30., 30.)] angles: DVec2,
	#[default(true)] connect_cells: bool,
) -> Vector {
	let (x_spacing, y_spacing) = spacing.as_dvec2().into();
	let (angle_a, angle_b) = angles.into();

	// Isometric grid spacing based on the two skew angles. Unused for rectangular grids.
	let tan_a = angle_a.to_radians().tan();
	let tan_b = angle_b.to_radians().tan();
	let isometric_spacing = DVec2::new(y_spacing / (tan_a + tan_b), y_spacing);

	// The position of the grid point at column `x`, row `y`.
	let position = |x: u32, y: u32| -> DVec2 {
		match grid_type {
			GridType::Rectangular => DVec2::new(x_spacing * x as f64, y_spacing * y as f64),
			GridType::Isometric => {
				// Odd columns are offset vertically so the cells skew into the isometric shape.
				let a_angles_eaten = x.div_ceil(2) as f64;
				let b_angles_eaten = (x / 2) as f64;
				let offset_y_fraction = b_angles_eaten * tan_b - a_angles_eaten * tan_a;
				DVec2::new(isometric_spacing.x * x as f64, isometric_spacing.y * y as f64 + offset_y_fraction * isometric_spacing.x)
			}
		}
	};

	// When the cells aren't connected, each one is its own closed quadrilateral subpath.
	// The vertices are ordered counter-clockwise to match the framework's fill winding.
	if !connect_cells {
		let mut cells = Vec::new();
		for y in 0..rows.saturating_sub(1) {
			for x in 0..columns.saturating_sub(1) {
				cells.push(vec![position(x, y), position(x + 1, y), position(x + 1, y + 1), position(x, y + 1)]);
			}
		}
		let mut vector = Vector::default();
		crate::vector_nodes::replace_with_polygons(&mut vector, cells, connect_cells);
		return vector;
	}

	let mut vector = Vector::default();
	let mut segment_id = SegmentId::ZERO;
	let mut point_id = PointId::ZERO;

	for y in 0..rows {
		for x in 0..columns {
			// Add the current point to the grid.
			let current_index = vector.point_domain.ids().len();
			vector.point_domain.push(point_id.next_id(), position(x, y));

			// Helper function to connect points with line segments.
			let mut push_segment = |to_index: Option<usize>| {
				if let Some(other_index) = to_index {
					vector.segment_domain.push(segment_id.next_id(), other_index, current_index, BezierHandles::Linear);
				}
			};

			// Connect to the point to the left (horizontal connection).
			push_segment((x > 0).then(|| current_index - 1));

			// Connect to the point directly above (vertical connection).
			push_segment(current_index.checked_sub(columns as usize));

			// Isometric grids additionally connect odd columns diagonally, splitting each cell into triangles.
			if grid_type == GridType::Isometric && x % 2 == 1 {
				// Connect to the point diagonally up-right (if not at the right edge).
				push_segment(current_index.checked_sub(columns as usize - 1).filter(|_| x + 1 < columns));

				// Connect to the point diagonally up-left.
				push_segment(current_index.checked_sub(columns as usize + 1));
			}
		}
	}

	vector
}

#[cfg(test)]
mod tests {
	use super::*;
	use kurbo::ParamCurve;
	use vector_types::vector::misc::point_to_dvec2;

	#[test]
	fn isometric_grid_test() {
		// Doesn't crash with weird angles
		grid(&(), (), GridType::Isometric, 0., 5, 5, (0., 0.).into(), true);
		grid(&(), (), GridType::Isometric, 90., 5, 5, (90., 90.).into(), true);

		// Works properly
		let grid = grid(&(), (), GridType::Isometric, 10., 5, 5, (30., 30.).into(), true);
		assert_eq!(grid.point_domain.ids().len(), 5 * 5);
		assert_eq!(grid.segment_iter().count(), 4 * 5 + 4 * 9);
		for (_, segment, _, _) in grid.segment_iter() {
			assert!(matches!(segment, kurbo::PathSeg::Line(_)));
			let span = point_to_dvec2(segment.start()) - point_to_dvec2(segment.end());
			assert!((span.length() - 10.).abs() < 1e-5, "Length of {} should be 10", span.length());
		}
	}

	#[test]
	fn skew_isometric_grid_test() {
		let grid = grid(&(), (), GridType::Isometric, 10., 5, 5, (40., 30.).into(), true);
		assert_eq!(grid.point_domain.ids().len(), 5 * 5);
		assert_eq!(grid.segment_iter().count(), 4 * 5 + 4 * 9);
		for (_, segment, _, _) in grid.segment_iter() {
			assert!(matches!(segment, kurbo::PathSeg::Line(_)));
			let vector = point_to_dvec2(segment.start()) - point_to_dvec2(segment.end());
			let angle = (vector.angle_to(DVec2::X).to_degrees() + 180.) % 180.;
			assert!([90f64, 150., 40.].into_iter().any(|target| (target - angle).abs() < 1e-10), "unexpected angle of {angle}")
		}
	}

	#[test]
	fn grid_disconnected_cells_test() {
		// A 3x3 rectangular grid has a 2x2 arrangement of cells, each its own closed quad subpath.
		let vector = grid(&(), (), GridType::Rectangular, 10., 3, 3, (30., 30.).into(), false);
		assert_eq!(vector.stroke_manipulator_groups().filter(|(_, closed)| *closed).count(), 4);
		assert_eq!(vector.point_domain.ids().len(), 4 * 4);
		assert_eq!(vector.segment_domain.ids().len(), 4 * 4);

		// Each cell winds counter-clockwise (positive signed area), matching the shape generators.
		for (group, closed) in vector.stroke_manipulator_groups() {
			assert!(closed);
			let anchors: Vec<DVec2> = group.iter().map(|g| g.anchor).collect();
			let signed_area: f64 = (0..anchors.len()).map(|i| anchors[i].perp_dot(anchors[(i + 1) % anchors.len()])).sum::<f64>() / 2.;
			assert!(signed_area > 0., "grid cell should wind counter-clockwise");
		}
	}

	#[test]
	fn qr_code_test() {
		let qr = qr_code(&(), (), "https://graphite.art".to_string(), false, 1., QRCodeErrorCorrectionLevel::Low, true);
		assert!(!qr.point_domain.ids().is_empty());
		assert!(!qr.segment_domain.ids().is_empty());
	}
}
