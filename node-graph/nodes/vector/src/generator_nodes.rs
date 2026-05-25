use core_types::list::{Item, List};
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
	radius: Item<f64>,
) -> Item<Vector> {
	let radius = radius.element().abs();
	Item::new_from_element(Vector::from_bezpath(shapes::ellipse_bezpath(DVec2::splat(-radius), DVec2::splat(radius))))
}

/// Generates an arc shape forming a portion of a circle which may be open, closed, or a pie slice.
#[node_macro::node(category("Vector: Shape"))]
fn arc(
	_: impl Ctx,
	_primary: (),
	#[unit(" px")]
	#[default(50.)]
	radius: Item<f64>,
	start_angle: Item<Angle>,
	#[default(270.)]
	#[range]
	#[soft(0..360)]
	sweep_angle: Item<Angle>,
	arc_type: Item<ArcType>,
) -> Item<Vector> {
	let (radius, start_angle, sweep_angle, arc_type) = (*radius.element(), *start_angle.element(), *sweep_angle.element(), arc_type.into_element());
	Item::new_from_element(Vector::from_bezpath(shapes::arc_bezpath(
		radius,
		start_angle / 360. * std::f64::consts::TAU,
		sweep_angle / 360. * std::f64::consts::TAU,
		arc_type,
	)))
}

/// Generates a spiral shape that winds from an inner to an outer radius.
#[node_macro::node(category("Vector: Shape"), properties("spiral_properties"))]
fn spiral(
	_: impl Ctx,
	_primary: (),
	spiral_type: Item<SpiralType>,
	#[default(5.)] turns: Item<f64>,
	#[default(0.)] start_angle: Item<f64>,
	#[default(0.)] inner_radius: Item<f64>,
	#[default(25)] outer_radius: Item<f64>,
	#[default(90.)] angular_resolution: Item<f64>,
) -> Item<Vector> {
	let (turns, start_angle, inner_radius, outer_radius, angular_resolution) = (
		*turns.element(),
		*start_angle.element(),
		*inner_radius.element(),
		*outer_radius.element(),
		*angular_resolution.element(),
	);
	Item::new_from_element(Vector::from_bezpath(shapes::spiral_bezpath(
		inner_radius,
		outer_radius,
		turns,
		start_angle.to_radians(),
		angular_resolution.to_radians(),
		spiral_type.into_element(),
	)))
}

/// Generates an ellipse shape (an oval or stretched circle) with the chosen radii.
#[node_macro::node(category("Vector: Shape"))]
fn ellipse(
	_: impl Ctx,
	_primary: (),
	#[unit(" px")]
	#[default(50)]
	radius_x: Item<f64>,
	#[unit(" px")]
	#[default(25)]
	radius_y: Item<f64>,
) -> Item<Vector> {
	let radius = DVec2::new(*radius_x.element(), *radius_y.element());
	let corner1 = -radius;
	let corner2 = radius;

	let mut ellipse = Vector::from_bezpath(shapes::ellipse_bezpath(corner1, corner2));

	let len = ellipse.segment_domain.ids().len();
	for i in 0..len {
		ellipse
			.colinear_manipulators
			.push([HandleId::end(ellipse.segment_domain.ids()[i]), HandleId::primary(ellipse.segment_domain.ids()[(i + 1) % len])]);
	}

	Item::new_from_element(ellipse)
}

/// Generates a rectangle shape with the chosen width and height. It may also have rounded corners if desired.
#[node_macro::node(category("Vector: Shape"), properties("rectangle_properties"))]
fn rectangle(
	_: impl Ctx,
	_primary: (),
	#[unit(" px")]
	#[default(100)]
	width: Item<f64>,
	#[unit(" px")]
	#[default(100)]
	height: Item<f64>,
	corner_radius: Item<BoxCorners>,
	#[default(true)] clamped: Item<bool>,
	_individual_corner_radii: Item<bool>,
) -> Item<Vector> {
	let size = DVec2::new(*width.element(), *height.element());
	let radii = corner_radius.element().to_corner_values();

	// Scale down overlapping adjacent radii to fit, following the CSS spec: <https://drafts.csswg.org/css-backgrounds/#corner-overlap>
	let radii = if *clamped.element() {
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

	Item::new_from_element(Vector::from_bezpath(shapes::rounded_rectangle_bezpath(size / -2., size / 2., radii)))
}

/// Builds a set of four corner values, such as a rectangle's corner radii, from a list of one, two, three, or four values.
#[node_macro::node(category("Vector: Shape"))]
fn box_corners(
	_: impl Ctx,
	/// The corner values, filling the four corners clockwise from the top-left. Give one value for all corners, two for opposite pairs, three for top-left, the two sides, then bottom-right, or four for each corner.
	values: List<f64>,
) -> Item<BoxCorners> {
	let values: Vec<f64> = values.iter_element_values().copied().collect();
	Item::new_from_element(BoxCorners::from(values))
}

/// Generates an regular polygon shape like a triangle, square, pentagon, hexagon, heptagon, octagon, or any higher n-gon.
#[node_macro::node(category("Vector: Shape"))]
fn regular_polygon<T: AsU64>(
	_: impl Ctx,
	_primary: (),
	#[default(6)]
	#[hard(3..)]
	#[implementations(u32, u64, f64)]
	sides: Item<T>,
	#[unit(" px")]
	#[default(50)]
	radius: Item<f64>,
) -> Item<Vector> {
	let points = sides.element().as_u64();
	Item::new_from_element(Vector::from_bezpath(shapes::regular_polygon_bezpath(DVec2::ZERO, points, *radius.element())))
}

/// Generates a heart shape with parametric control over the cleavage, lobes, shoulders, and bottom point.
#[node_macro::node(category("Vector: Shape"))]
fn heart(
	_: impl Ctx,
	_primary: (),
	#[unit(" px")]
	#[default(50)]
	radius: f64,
	/// How far the top V dips below the upper bound of the heart.
	#[default(0.2)]
	#[range((0., 0.6))]
	#[hard_min(0.)]
	#[hard_max(0.6)]
	cleavage_depth: f64,
	/// Half-angle of the top V. Zero collapses the V into a smooth join.
	#[default(45.)]
	#[range((0., 89.))]
	#[hard_min(0.)]
	#[hard_max(89.)]
	cleavage_angle: Angle,
	/// Tangent length leaving the top cusp, controlling the upper roundness of each lobe.
	#[default(0.55)]
	#[range((0., 1.2))]
	#[hard_min(0.)]
	#[hard_max(1.2)]
	lobe_fullness: f64,
	/// Vertical position of the side anchor (positive raises the shoulder).
	#[default(0.5)]
	#[range((-0.5, 0.9))]
	#[hard_min(-0.5)]
	#[hard_max(0.9)]
	shoulder_height: f64,
	/// Horizontal position of the side anchor.
	#[default(1.)]
	#[range((0., 1.4))]
	#[hard_min(0.)]
	#[hard_max(1.4)]
	shoulder_width: f64,
	/// Rotation of the shoulder tangent from vertical. Positive leans the shoulder outward at top.
	#[default(0.)]
	#[range((-60., 60.))]
	#[hard_min(-60.)]
	#[hard_max(60.)]
	shoulder_tilt: Angle,
	/// Tangent length at the shoulder going up, controlling the curvature of the upper lobe side.
	#[default(0.55)]
	#[range((0., 1.2))]
	#[hard_min(0.)]
	#[hard_max(1.2)]
	upper_curvature: f64,
	/// Tangent length at the shoulder going down, controlling the curvature of the lower side.
	#[default(1.)]
	#[range((0., 1.5))]
	#[hard_min(0.)]
	#[hard_max(1.5)]
	lower_curvature: f64,
	/// Half-angle of the bottom V. Zero produces a needle-sharp point with vertical tangents.
	#[default(30.)]
	#[range((0., 89.))]
	#[hard_min(0.)]
	#[hard_max(89.)]
	point_sharpness: Angle,
	/// Tangent length arriving at the bottom cusp, controlling how the sides taper into the point.
	#[default(0.7)]
	#[range((0., 1.2))]
	#[hard_min(0.)]
	#[hard_max(1.2)]
	taper_length: f64,
) -> List<Vector> {
	let cleavage_angle = cleavage_angle.to_radians();
	let point_sharpness = point_sharpness.to_radians();
	let shoulder_tilt = shoulder_tilt.to_radians();

	// Anchor points for the right half plus the y-axis cusps, in normalized coordinates (y points downward).
	let top = DVec2::new(0., -1. + cleavage_depth);
	let shoulder = DVec2::new(shoulder_width, -shoulder_height);
	let bottom = DVec2::new(0., 1.);

	// Unit tangent directions, all measured from the upward vertical.
	let top_dir = DVec2::new(cleavage_angle.sin(), -cleavage_angle.cos());
	let bottom_dir_out = DVec2::new(point_sharpness.sin(), -point_sharpness.cos());
	let shoulder_up = DVec2::new(shoulder_tilt.sin(), -shoulder_tilt.cos());
	let shoulder_down = -shoulder_up;

	// Cubic Bezier control points for the right half.
	let c1 = top + top_dir * lobe_fullness;
	let c2 = shoulder + shoulder_up * upper_curvature;
	let c3 = shoulder + shoulder_down * lower_curvature;
	let c4 = bottom + bottom_dir_out * taper_length;

	let mirror = |p: DVec2| DVec2::new(-p.x, p.y);

	// Closed clockwise path: T → S → B → S' → T. Joins at T and B are sharp; joins at the shoulders are G1.
	let manipulator_groups = [
		subpath::ManipulatorGroup::new(top * radius, Some(mirror(c1) * radius), Some(c1 * radius)),
		subpath::ManipulatorGroup::new(shoulder * radius, Some(c2 * radius), Some(c3 * radius)),
		subpath::ManipulatorGroup::new(bottom * radius, Some(c4 * radius), Some(mirror(c4) * radius)),
		subpath::ManipulatorGroup::new(mirror(shoulder) * radius, Some(mirror(c3) * radius), Some(mirror(c2) * radius)),
	]
	.to_vec();

	List::new_from_element(Vector::from_subpath(subpath::Subpath::new(manipulator_groups, true)))
}

/// Generates an n-pointed star shape with inner and outer points at chosen radii from the center.
#[node_macro::node(category("Vector: Shape"))]
fn star<T: AsU64>(
	_: impl Ctx,
	_primary: (),
	#[default(5)]
	#[hard(2..)]
	#[implementations(u32, u64, f64)]
	sides: Item<T>,
	#[unit(" px")]
	#[default(50)]
	radius_1: Item<f64>,
	#[unit(" px")]
	#[default(25)]
	radius_2: Item<f64>,
) -> Item<Vector> {
	let points = sides.element().as_u64();
	Item::new_from_element(Vector::from_bezpath(shapes::star_polygon_bezpath(DVec2::ZERO, points, *radius_1.element(), *radius_2.element())))
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
	text: Item<String>,
	#[widget(ParsedWidgetOverride::Hidden)] has_size: Item<bool>,
	#[unit(" px")]
	#[hard(1..)]
	#[widget(ParsedWidgetOverride::Custom = "optional_f64")]
	size: Item<f64>,
	error_correction: Item<QRCodeErrorCorrectionLevel>,
	individual_squares: Item<bool>,
) -> Item<Vector> {
	let (text, error_correction) = (text.into_element(), error_correction.into_element());
	let (has_size, size, individual_squares) = (*has_size.element(), *size.element(), *individual_squares.element());

	let ecc = match error_correction {
		QRCodeErrorCorrectionLevel::Low => qrcodegen::QrCodeEcc::Low,
		QRCodeErrorCorrectionLevel::Medium => qrcodegen::QrCodeEcc::Medium,
		QRCodeErrorCorrectionLevel::Quartile => qrcodegen::QrCodeEcc::Quartile,
		QRCodeErrorCorrectionLevel::High => qrcodegen::QrCodeEcc::High,
	};

	let Ok(qr_code) = qrcodegen::QrCode::encode_text(&text, ecc) else {
		return Item::new_from_element(Vector::default());
	};

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

	Item::new_from_element(vector)
}

/// Generates an arrow from the origin to the chosen coordinate.
#[node_macro::node(category("Vector: Shape"))]
fn arrow(
	_: impl Ctx,
	_primary: (),
	#[default(100., 0.)] arrow_to: Item<PixelSize>,
	#[default(10)] shaft_width: Item<PixelLength>,
	#[default(30)] head_width: Item<PixelLength>,
	#[default(20)] head_length: Item<PixelLength>,
) -> Item<Vector> {
	let (arrow_to, shaft_width, head_width, head_length) = (*arrow_to.element(), *shaft_width.element(), *head_width.element(), *head_length.element());
	Item::new_from_element(Vector::from_bezpath(shapes::arrow_bezpath(DVec2::ZERO, arrow_to, shaft_width, head_width, head_length)))
}

#[node_macro::node(category("Vector: Shape"))]
fn line(_: impl Ctx, _primary: (), #[default(100., 100.)] line_to: Item<PixelSize>) -> Item<Vector> {
	Item::new_from_element(Vector::from_bezpath(shapes::line_bezpath(DVec2::ZERO, *line_to.element())))
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
	grid_type: Item<GridType>,
	#[unit(" px")]
	#[hard(0..)]
	#[default(10)]
	#[implementations(f64, DVec2)]
	spacing: Item<T>,
	#[default(10)] columns: Item<u32>,
	#[default(10)] rows: Item<u32>,
	#[default(30., 30.)] angles: Item<DVec2>,
	#[default(true)] connect_cells: Item<bool>,
) -> Item<Vector> {
	let (grid_type, columns, rows, angles, connect_cells) = (grid_type.into_element(), *columns.element(), *rows.element(), *angles.element(), *connect_cells.element());

	let (x_spacing, y_spacing) = spacing.element().as_dvec2().into();
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
		return Item::new_from_element(vector);
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

	Item::new_from_element(vector)
}

#[cfg(test)]
mod tests {
	use super::*;
	use kurbo::ParamCurve;
	use vector_types::vector::misc::point_to_dvec2;

	fn item<T>(value: T) -> Item<T> {
		Item::new_from_element(value)
	}

	#[test]
	fn isometric_grid_test() {
		// Doesn't crash with weird angles
		grid((), (), item(GridType::Isometric), item(0.), item(5_u32), item(5_u32), item((0., 0.).into()), item(true));
		grid((), (), item(GridType::Isometric), item(90.), item(5_u32), item(5_u32), item((90., 90.).into()), item(true));

		// Works properly
		let grid = grid((), (), item(GridType::Isometric), item(10.), item(5_u32), item(5_u32), item((30., 30.).into()), item(true));
		assert_eq!(grid.element().point_domain.ids().len(), 5 * 5);
		assert_eq!(grid.element().segment_iter().count(), 4 * 5 + 4 * 9);
		for (_, segment, _, _) in grid.element().segment_iter() {
			assert!(matches!(segment, kurbo::PathSeg::Line(_)));
			let span = point_to_dvec2(segment.start()) - point_to_dvec2(segment.end());
			assert!((span.length() - 10.).abs() < 1e-5, "Length of {} should be 10", span.length());
		}
	}

	#[test]
	fn skew_isometric_grid_test() {
		let grid = grid((), (), item(GridType::Isometric), item(10.), item(5_u32), item(5_u32), item((40., 30.).into()), item(true));
		assert_eq!(grid.element().point_domain.ids().len(), 5 * 5);
		assert_eq!(grid.element().segment_iter().count(), 4 * 5 + 4 * 9);
		for (_, segment, _, _) in grid.element().segment_iter() {
			assert!(matches!(segment, kurbo::PathSeg::Line(_)));
			let vector = point_to_dvec2(segment.start()) - point_to_dvec2(segment.end());
			let angle = (vector.angle_to(DVec2::X).to_degrees() + 180.) % 180.;
			assert!([90., 150., 40.].into_iter().any(|target| (target - angle).abs() < 1e-10), "unexpected angle of {angle}")
		}
	}

	#[test]
	fn grid_disconnected_cells_test() {
		// A 3x3 rectangular grid has a 2x2 arrangement of cells, each its own closed quad subpath.
		let grid = grid((), (), item(GridType::Rectangular), item(10.), item(3_u32), item(3_u32), item((30., 30.).into()), item(false));
		let vector = grid.element();
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
		let qr = qr_code(
			(),
			(),
			item("https://graphite.art".to_string()),
			item(false),
			item(1.),
			item(QRCodeErrorCorrectionLevel::Low),
			item(true),
		);
		assert!(!qr.element().point_domain.ids().is_empty());
		assert!(!qr.element().segment_domain.ids().is_empty());
	}
}
