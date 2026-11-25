use crate::card::{Connector, DataType, Highlight};
use crate::icons;
use crate::metrics::{CHECKER_CELL, HOVERED_CONNECTOR_RING, HOVERED_CONNECTOR_SCALE, ICON_SIZE};
use core_types::Color;
use core_types::color::SRGBA8;
use core_types::list::{Item, List};
use core_types::transform::ApplyTransform;
use glam::{DAffine2, DVec2};
use graphic_types::{CoverPlacement, Coverage, Graphic, Vector, stamp_coverage};
use std::sync::LazyLock;
use vector_types::vector::algorithms::shapes::rounded_rectangle_bezpath;
use vector_types::vector::style::{Stroke, StrokeJoin};
use vello_gpu::kurbo::{Affine, BezPath, Rect, Shape};

pub(crate) const CARD_OPACITY: f32 = 0.33;
pub(crate) const GRID_DOT_OPACITY: f32 = 0.2;
pub(crate) const BACKDROP: SRGBA8 = SRGBA8::new_opaque(0x22, 0x22, 0x22);
pub(crate) const SELECTED_DOT_RING_OPACITY: f32 = 0.5;
pub(crate) const BORDER: SRGBA8 = SRGBA8::new_opaque(0x55, 0x55, 0x55);
pub(crate) const PREVIEWED_BORDER: SRGBA8 = SRGBA8::new_opaque(0xee, 0xee, 0xee);
pub(crate) const TEXT: SRGBA8 = SRGBA8::new_opaque(0xee, 0xee, 0xee);
pub(crate) const LAYER_IN_INACTIVE_SELECTION: SRGBA8 = SRGBA8::new_opaque(0x55, 0x55, 0x55);
pub(crate) const LAYER_IN_ACTIVE_SELECTION: SRGBA8 = SRGBA8::new_opaque(0x66, 0x66, 0x66);
pub(crate) const HIDDEN_CARD: SRGBA8 = SRGBA8::new_opaque(0x44, 0x44, 0x44);
pub(crate) const HIDDEN_TEXT: SRGBA8 = SRGBA8::new_opaque(0xaa, 0xaa, 0xaa);
pub(crate) const SEPARATOR: SRGBA8 = SRGBA8::new_opaque(0x88, 0x88, 0x88);
pub(crate) const HOVERED_BUTTON: SRGBA8 = SRGBA8::new_opaque(0x55, 0x55, 0x55);
pub(crate) const DISABLED_ICON: SRGBA8 = SRGBA8::new_opaque(0x88, 0x88, 0x88);
pub(crate) const BOX_SELECTION: SRGBA8 = SRGBA8::new_opaque(0x00, 0xa8, 0xff);
pub(crate) const BOX_SELECTION_FILL_OPACITY: f32 = 0.05;
pub(crate) const ERROR_RED: SRGBA8 = SRGBA8::new_opaque(0xd6, 0x53, 0x6e);
pub(crate) const ERROR_TEXT: SRGBA8 = SRGBA8::new_opaque(0x22, 0x22, 0x22);
pub(crate) const ERROR_OPACITY: f64 = 0.5;
pub(crate) const WIRE_SPLIT: SRGBA8 = SRGBA8::new_opaque(0x44, 0x44, 0x44);
pub(crate) const THUMBNAIL_BACKGROUND: SRGBA8 = SRGBA8::new_opaque(0x22, 0x22, 0x22);
const CHECKER_LIGHT: SRGBA8 = SRGBA8::new_opaque(0xff, 0xff, 0xff);
const CHECKER_DARK: SRGBA8 = SRGBA8::new_opaque(0xcc, 0xcc, 0xcc);
pub(crate) const CIRCLE_TOLERANCE: f64 = 1e-3;

pub(crate) static CONNECTOR_OUTLINE: LazyLock<BezPath> = LazyLock::new(|| parsed("M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z"));
pub(crate) static STACK_OUTPUT_OUTLINE: LazyLock<BezPath> = LazyLock::new(|| parsed("M0,6.953l2.521,-1.694a2.649,2.649,0,0,1,2.959,0l2.52,1.694v5.047h-8z"));
pub(crate) static STACK_INPUT_OUTLINE: LazyLock<BezPath> = LazyLock::new(|| parsed("M0,0H8V8L5.479,6.319a2.666,2.666,0,0,0-2.959,0L0,8Z"));
pub(crate) static STACK_OUTPUT_WIRE_END: LazyLock<BezPath> = LazyLock::new(|| {
	parsed(
		"M0,4.5 C0,4.5 0,-3.5 0,-3.5 C0,-3.5 3,-3.5 3,-3.5 C3,-3.5 3,2.565 3,2.565 C2.834,2.632 2.673,2.717 2.52,2.819 C2.52,2.819 0,4.5 0,4.5 Z\
		M5,2.566 C5,2.566 5,-3.5 5,-3.5 C5,-3.5 8,-3.5 8,-3.5 C8,-3.5 8,4.5 8,4.5 C8,4.5 5.479,2.819 5.479,2.819 C5.326,2.717 5.166,2.633 5,2.566 Z",
	)
});
pub(crate) static STACK_INPUT_WIRE_END: LazyLock<BezPath> = LazyLock::new(|| {
	parsed(
		"M2.512,9.26 C2.673,9.157 2.834,9.072 3,9 C3,9 3,16 3,16 C3,16 0,16 0,16 C0,16 0,10.95 0,10.95 C0,10.95 2.512,9.26 2.512,9.26 Z\
		M5,16 C5,16 5,9 5,9 C5.166,9.073 5.327,9.158 5.48,9.26 C5.48,9.26 8,10.95 8,10.95 C8,10.95 8,16 8,16 C8,16 5,16 5,16 Z",
	)
});

fn parsed(path_data: &str) -> BezPath {
	BezPath::from_svg(path_data).expect("A connector outline is valid path data")
}

pub(crate) fn data_type_colors(data_type: DataType) -> [SRGBA8; 2] {
	let colors = match data_type {
		DataType::General => [0xcfcfcf, 0x8a8a8a],
		DataType::Number => [0xc9a699, 0x886b60],
		DataType::Artboard => [0xfbf9eb, 0xb9b9a9],
		DataType::Graphic => [0x68c587, 0x37754c],
		DataType::Raster => [0xe4bb72, 0x9a7b43],
		DataType::Vector => [0x65bbe5, 0x417892],
		DataType::Color => [0xce6ea7, 0x924071],
		DataType::Gradient => [0xaf81eb, 0x6c489b],
		DataType::Typography => [0xeea7a7, 0x955252],
		DataType::Invalid => [0xd6536e, 0xa7324a],
	};
	colors.map(|color| {
		let [_, red, green, blue] = u32::to_be_bytes(color);
		SRGBA8::new_opaque(red, green, blue)
	})
}

pub(crate) fn connector(outline: &BezPath, top_left: DVec2, connector: &Connector) -> List<Vector> {
	drawn_connector(outline, top_left, connector.data_type, connector.connected, connector.highlight)
}

pub(crate) fn drawn_connector(outline: &BezPath, top_left: DVec2, data_type: DataType, connected: bool, highlight: Highlight) -> List<Vector> {
	let [bright, dim] = data_type_colors(data_type);
	match highlight {
		Highlight::None => List::new_from_item(outlined(outline, top_left, if connected { bright } else { dim })),
		Highlight::WireSource => List::new_from_item(outlined(outline, top_left, bright)),
		Highlight::Hovered | Highlight::WireTarget => {
			let grown = Affine::translate((top_left.x, top_left.y)) * Affine::scale_about(HOVERED_CONNECTOR_SCALE, outline.bounding_box().center()) * outline.clone();
			let grown = Vector::item_from_bezpath(&grown);
			let ring = Stroke {
				join: StrokeJoin::Round,
				..Stroke::new(HOVERED_CONNECTOR_RING * HOVERED_CONNECTOR_SCALE)
			};
			List::from_iter([painted(grown.clone(), Coverage::new_stroke(&ring), dim.into()), painted(grown, Coverage::new_fill(), bright.into())])
		}
	}
}

pub(crate) fn outlined(outline: &BezPath, top_left: DVec2, color: SRGBA8) -> Item<Vector> {
	let mut outline = Vector::item_from_bezpath(outline);
	outline.left_apply_transform(&DAffine2::from_translation(top_left));
	painted(outline, Coverage::new_fill(), color.into())
}

pub(crate) fn rounded_rect(min: DVec2, max: DVec2, radii: [f64; 4]) -> Item<Vector> {
	Vector::item_from_bezpath(&rounded_rectangle_bezpath(min, max, radii))
}

pub(crate) fn icon(name: &str, top_left: DVec2, current: SRGBA8) -> List<Vector> {
	let Some(icon) = icons::icon(name, current) else { return List::new() };
	let mut graphic = icon.shapes;
	if icon.overflows {
		let mut clip = rounded_rect(DVec2::ZERO, DVec2::splat(ICON_SIZE), [0.; 4]);
		stamp_coverage(&mut clip, Coverage::new_fill(), Graphic::VectorList(graphic), CoverPlacement::Above);
		graphic = List::new_from_item(clip);
	}
	graphic.left_apply_transform(&DAffine2::from_translation(top_left));
	graphic
}

pub(crate) fn text(outline: &List<Vector>, top_left: DVec2, color: SRGBA8) -> List<Vector> {
	let glyphs = outline.clone().into_iter();
	let mut glyphs: List<Vector> = glyphs.map(|glyph| painted(glyph, Coverage::new_fill(), color.into())).collect();
	glyphs.left_apply_transform(&DAffine2::from_translation(top_left));
	glyphs
}

pub(crate) fn checkerboard(min: DVec2, max: DVec2) -> List<Vector> {
	let mut dark = BezPath::new();
	let cells = ((max - min) / CHECKER_CELL).ceil().as_uvec2();
	for (column, row) in (0..cells.y)
		.flat_map(|row| (0..cells.x).map(move |column| (column, row)))
		.filter(|(column, row)| (column + row) % 2 == 1)
	{
		let cell_min = min + DVec2::new(column as f64, row as f64) * CHECKER_CELL;
		let cell_max = (cell_min + CHECKER_CELL).min(max);
		dark.extend(Rect::new(cell_min.x, cell_min.y, cell_max.x, cell_max.y).path_elements(0.));
	}
	let light = painted(rounded_rect(min, max, [0.; 4]), Coverage::new_fill(), CHECKER_LIGHT.into());
	List::from_iter([light, painted(Vector::item_from_bezpath(&dark), Coverage::new_fill(), CHECKER_DARK.into())])
}

pub(crate) fn painted(mut item: Item<Vector>, coverage: Coverage, color: Color) -> Item<Vector> {
	stamp_coverage(&mut item, coverage, Graphic::from(color), CoverPlacement::Above);
	item
}
