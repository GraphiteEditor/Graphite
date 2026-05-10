use core_types::list::{Item, List};
use core_types::registry::types::{Angle, PixelLength, PixelSize};
use core_types::{CacheHash, Ctx};
use dyn_any::DynAny;
use glam::DVec2;
use graphic_types::Vector;
use vector_types::subpath;
use vector_types::vector::misc::{ArcType, AsU64, GridType};
use vector_types::vector::misc::{HandleId, SpiralType};
use vector_types::vector::{PointId, SegmentId, StrokeId};

trait CornerRadius {
	fn generate(self, size: DVec2, clamped: bool) -> List<Vector>;
}
impl CornerRadius for f64 {
	fn generate(self, size: DVec2, clamped: bool) -> List<Vector> {
		let clamped_radius = if clamped { self.clamp(0., size.x.min(size.y).max(0.) / 2.) } else { self };
		List::new_from_element(Vector::from_subpath(subpath::Subpath::new_rounded_rectangle(size / -2., size / 2., [clamped_radius; 4])))
	}
}
impl CornerRadius for List<f64> {
	fn generate(self, size: DVec2, clamped: bool) -> List<Vector> {
		// Expand to four corners using the CSS `border-radius` shorthand rules.
		// - `[a]` → `[a, a, a, a]`
		// - `[a, b]` → `[a, b, a, b]`
		// - `[a, b, c]` → `[a, b, c, b]`
		// - `[a, b, c, d, …]` → `[a, b, c, d]`
		// - `[]` → `[0, 0, 0, 0]`
		let values: Vec<f64> = self.iter_element_values().copied().collect();
		let radii: [f64; 4] = match values.as_slice() {
			[] => [0., 0., 0., 0.],
			&[a] => [a, a, a, a],
			&[a, b] => [a, b, a, b],
			&[a, b, c] => [a, b, c, b],
			&[a, b, c, d, ..] => [a, b, c, d],
		};

		let clamped_radius = if clamped {
			// Algorithm follows the CSS spec: <https://drafts.csswg.org/css-backgrounds/#corner-overlap>

			let mut scale_factor: f64 = 1.;
			for i in 0..4 {
				let side_length = if i % 2 == 0 { size.x } else { size.y };
				let adjacent_corner_radius_sum = radii[i] + radii[(i + 1) % 4];
				if side_length < adjacent_corner_radius_sum {
					scale_factor = scale_factor.min(side_length / adjacent_corner_radius_sum);
				}
			}
			radii.map(|x| x * scale_factor)
		} else {
			radii
		};
		List::new_from_element(Vector::from_subpath(subpath::Subpath::new_rounded_rectangle(size / -2., size / 2., clamped_radius)))
	}
}

/// Generates a circle shape with a chosen radius.
#[node_macro::node(category("Vector: Shape"))]
fn circle(
	_: impl Ctx,
	_primary: Item<()>,
	#[unit(" px")]
	#[default(50.)]
	radius: Item<f64>,
) -> Item<List<Vector>> {
	let radius = radius.into_element();

	let radius = radius.abs();
	Item::new_from_element(List::new_from_element(Vector::from_subpath(subpath::Subpath::new_ellipse(DVec2::splat(-radius), DVec2::splat(radius)))))
}

/// Generates an arc shape forming a portion of a circle which may be open, closed, or a pie slice.
#[node_macro::node(category("Vector: Shape"))]
fn arc(
	_: impl Ctx,
	_primary: Item<()>,
	#[unit(" px")]
	#[default(50.)]
	radius: Item<f64>,
	start_angle: Item<Angle>,
	#[default(270.)]
	#[range((0., 360.))]
	sweep_angle: Item<Angle>,
	arc_type: Item<ArcType>,
) -> Item<List<Vector>> {
	let radius = radius.into_element();
	let start_angle = start_angle.into_element();
	let sweep_angle = sweep_angle.into_element();
	let arc_type = arc_type.into_element();

	Item::new_from_element(List::new_from_element(Vector::from_subpath(subpath::Subpath::new_arc(
		radius,
		start_angle / 360. * std::f64::consts::TAU,
		sweep_angle / 360. * std::f64::consts::TAU,
		match arc_type {
			ArcType::Open => subpath::ArcType::Open,
			ArcType::Closed => subpath::ArcType::Closed,
			ArcType::PieSlice => subpath::ArcType::PieSlice,
		},
	))))
}

/// Generates a spiral shape that winds from an inner to an outer radius.
#[node_macro::node(category("Vector: Shape"), properties("spiral_properties"))]
fn spiral(
	_: impl Ctx,
	_primary: Item<()>,
	spiral_type: Item<SpiralType>,
	#[default(5.)] turns: Item<f64>,
	#[default(0.)] start_angle: Item<f64>,
	#[default(0.)] inner_radius: Item<f64>,
	#[default(25)] outer_radius: Item<f64>,
	#[default(90.)] angular_resolution: Item<f64>,
) -> Item<List<Vector>> {
	let spiral_type = spiral_type.into_element();
	let turns = turns.into_element();
	let start_angle = start_angle.into_element();
	let inner_radius = inner_radius.into_element();
	let outer_radius = outer_radius.into_element();
	let angular_resolution = angular_resolution.into_element();

	Item::new_from_element(List::new_from_element(Vector::from_subpath(subpath::Subpath::new_spiral(
		inner_radius,
		outer_radius,
		turns,
		start_angle.to_radians(),
		angular_resolution.to_radians(),
		spiral_type,
	))))
}

/// Generates an ellipse shape (an oval or stretched circle) with the chosen radii.
#[node_macro::node(category("Vector: Shape"))]
fn ellipse(
	_: impl Ctx,
	_primary: Item<()>,
	#[unit(" px")]
	#[default(50)]
	radius_x: Item<f64>,
	#[unit(" px")]
	#[default(25)]
	radius_y: Item<f64>,
) -> Item<List<Vector>> {
	let radius_x = radius_x.into_element();
	let radius_y = radius_y.into_element();

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

	Item::new_from_element(List::new_from_element(ellipse))
}

/// Generates a rectangle shape with the chosen width and height. It may also have rounded corners if desired.
#[node_macro::node(category("Vector: Shape"), properties("rectangle_properties"))]
fn rectangle<T: CornerRadius>(
	_: impl Ctx,
	_primary: Item<()>,
	#[unit(" px")]
	#[default(100)]
	width: Item<f64>,
	#[unit(" px")]
	#[default(100)]
	height: Item<f64>,
	_individual_corner_radii: Item<bool>, // TODO: Move this to the bottom once we have a migration capability
	#[implementations(Item<f64>, Item<List<f64>>)] corner_radius: Item<T>,
	#[default(true)] clamped: Item<bool>,
) -> Item<List<Vector>> {
	let width = width.into_element();
	let height = height.into_element();
	let _individual_corner_radii = _individual_corner_radii.into_element();
	let corner_radius = corner_radius.into_element();
	let clamped = clamped.into_element();

	Item::new_from_element(corner_radius.generate(DVec2::new(width, height), clamped))
}

/// Generates an regular polygon shape like a triangle, square, pentagon, hexagon, heptagon, octagon, or any higher n-gon.
#[node_macro::node(category("Vector: Shape"))]
fn regular_polygon<T: AsU64>(
	_: impl Ctx,
	_primary: Item<()>,
	#[default(6)]
	#[hard_min(3.)]
	#[implementations(Item<u32>, Item<u64>, Item<f64>)]
	sides: Item<T>,
	#[unit(" px")]
	#[default(50)]
	radius: Item<f64>,
) -> Item<List<Vector>> {
	let sides = sides.into_element();
	let radius = radius.into_element();

	let points = sides.as_u64();
	let radius: f64 = radius * 2.;
	Item::new_from_element(List::new_from_element(Vector::from_subpath(subpath::Subpath::new_regular_polygon(DVec2::splat(-radius), points, radius))))
}

/// Generates an n-pointed star shape with inner and outer points at chosen radii from the center.
#[node_macro::node(category("Vector: Shape"))]
fn star<T: AsU64>(
	_: impl Ctx,
	_primary: Item<()>,
	#[default(5)]
	#[hard_min(2.)]
	#[implementations(Item<u32>, Item<u64>, Item<f64>)]
	sides: Item<T>,
	#[unit(" px")]
	#[default(50)]
	radius_1: Item<f64>,
	#[unit(" px")]
	#[default(25)]
	radius_2: Item<f64>,
) -> Item<List<Vector>> {
	let sides = sides.into_element();
	let radius_1 = radius_1.into_element();
	let radius_2 = radius_2.into_element();

	let points = sides.as_u64();
	let diameter: f64 = radius_1 * 2.;
	let inner_diameter = radius_2 * 2.;

	Item::new_from_element(List::new_from_element(Vector::from_subpath(subpath::Subpath::new_star_polygon(
		DVec2::splat(-diameter),
		points,
		diameter,
		inner_diameter,
	))))
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
	_primary: Item<()>,
	#[widget(ParsedWidgetOverride::Custom = "text_area")]
	#[default("https://graphite.art")]
	text: Item<String>,
	#[widget(ParsedWidgetOverride::Hidden)] has_size: Item<bool>,
	#[unit(" px")]
	#[hard_min(1.)]
	#[widget(ParsedWidgetOverride::Custom = "optional_f64")]
	size: Item<f64>,
	error_correction: Item<QRCodeErrorCorrectionLevel>,
	#[default(false)] individual_squares: Item<bool>,
) -> Item<List<Vector>> {
	let text = text.into_element();
	let has_size = has_size.into_element();
	let size = size.into_element();
	let error_correction = error_correction.into_element();
	let individual_squares = individual_squares.into_element();

	let ecc = match error_correction {
		QRCodeErrorCorrectionLevel::Low => qrcodegen::QrCodeEcc::Low,
		QRCodeErrorCorrectionLevel::Medium => qrcodegen::QrCodeEcc::Medium,
		QRCodeErrorCorrectionLevel::Quartile => qrcodegen::QrCodeEcc::Quartile,
		QRCodeErrorCorrectionLevel::High => qrcodegen::QrCodeEcc::High,
	};

	let Ok(qr_code) = qrcodegen::QrCode::encode_text(&text, ecc) else { return Item::new_from_element(List::default()) };

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

	Item::new_from_element(List::new_from_element(vector))
}

/// Generates an arrow from the origin to the chosen coordinate.
#[node_macro::node(category("Vector: Shape"))]
fn arrow(
	_: impl Ctx,
	_primary: Item<()>,
	#[default(100., 0.)] arrow_to: Item<PixelSize>,
	#[default(10)] shaft_width: Item<PixelLength>,
	#[default(30)] head_width: Item<PixelLength>,
	#[default(20)] head_length: Item<PixelLength>,
) -> Item<List<Vector>> {
	let arrow_to = arrow_to.into_element();
	let shaft_width = shaft_width.into_element();
	let head_width = head_width.into_element();
	let head_length = head_length.into_element();

	Item::new_from_element(List::new_from_element(Vector::from_subpath(subpath::Subpath::new_arrow(
		DVec2::ZERO,
		arrow_to,
		shaft_width,
		head_width,
		head_length,
	))))
}

#[node_macro::node(category("Vector: Shape"))]
fn line(_: impl Ctx, _primary: Item<()>, #[default(100., 100.)] line_to: Item<PixelSize>) -> Item<List<Vector>> {
	let line_to = line_to.into_element();

	Item::new_from_element(List::new_from_element(Vector::from_subpath(subpath::Subpath::new_line(DVec2::ZERO, line_to))))
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
	_primary: Item<()>,
	grid_type: Item<GridType>,
	#[unit(" px")]
	#[hard_min(0.)]
	#[default(10)]
	#[implementations(Item<f64>, Item<DVec2>)]
	spacing: Item<T>,
	#[default(10)] columns: Item<u32>,
	#[default(10)] rows: Item<u32>,
	#[default(30., 30.)] angles: Item<DVec2>,
) -> Item<List<Vector>> {
	let grid_type = grid_type.into_element();
	let spacing = spacing.into_element();
	let columns = columns.into_element();
	let rows = rows.into_element();
	let angles = angles.into_element();

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

	Item::new_from_element(List::new_from_element(vector))
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn isometric_grid_test() {
		// Doesn't crash with weird angles
		grid(
			(),
			Item::new_from_element(()),
			Item::new_from_element(GridType::Isometric),
			Item::new_from_element(0.),
			Item::new_from_element(5),
			Item::new_from_element(5),
			Item::new_from_element((0., 0.).into()),
		);
		grid(
			(),
			Item::new_from_element(()),
			Item::new_from_element(GridType::Isometric),
			Item::new_from_element(90.),
			Item::new_from_element(5),
			Item::new_from_element(5),
			Item::new_from_element((90., 90.).into()),
		);

		// Works properly
		let grid = grid(
			(),
			Item::new_from_element(()),
			Item::new_from_element(GridType::Isometric),
			Item::new_from_element(10.),
			Item::new_from_element(5),
			Item::new_from_element(5),
			Item::new_from_element((30., 30.).into()),
		)
		.into_element();
		assert_eq!(grid.element(0).unwrap().point_domain.ids().len(), 5 * 5);
		assert_eq!(grid.element(0).unwrap().segment_bezier_iter().count(), 4 * 5 + 4 * 9);
		for (_, bezier, _, _) in grid.element(0).unwrap().segment_bezier_iter() {
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
		let grid = grid(
			(),
			Item::new_from_element(()),
			Item::new_from_element(GridType::Isometric),
			Item::new_from_element(10.),
			Item::new_from_element(5),
			Item::new_from_element(5),
			Item::new_from_element((40., 30.).into()),
		)
		.into_element();
		assert_eq!(grid.element(0).unwrap().point_domain.ids().len(), 5 * 5);
		assert_eq!(grid.element(0).unwrap().segment_bezier_iter().count(), 4 * 5 + 4 * 9);
		for (_, bezier, _, _) in grid.element(0).unwrap().segment_bezier_iter() {
			assert_eq!(bezier.handles, subpath::BezierHandles::Linear);
			let vector = bezier.start - bezier.end;
			let angle = (vector.angle_to(DVec2::X).to_degrees() + 180.) % 180.;
			assert!([90., 150., 40.].into_iter().any(|target| (target - angle).abs() < 1e-10), "unexpected angle of {angle}")
		}
	}

	#[test]
	fn qr_code_test() {
		let qr = qr_code(
			(),
			Item::new_from_element(()),
			Item::new_from_element("https://graphite.art".to_string()),
			Item::new_from_element(false),
			Item::new_from_element(1.),
			Item::new_from_element(QRCodeErrorCorrectionLevel::Low),
			Item::new_from_element(true),
		)
		.into_element();
		assert!(qr.element(0).unwrap().point_domain.ids().len() > 0);
		assert!(qr.element(0).unwrap().segment_domain.ids().len() > 0);
	}
}
