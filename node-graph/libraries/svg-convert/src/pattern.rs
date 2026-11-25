use crate::{affine, graphic_list};
use core_types::list::{Item, List};
use core_types::{ATTR_OPACITY, ATTR_TRANSFORM};
use glam::{DAffine2, DVec2};
use graphic_types::{CoverPlacement, Coverage, Graphic, Vector, stamp_coverage};
use kurbo::Shape;

const MAX_TILES: f64 = 1024.;

pub fn tiled(pattern: &usvg::Pattern, covered: usvg::Rect, opacity: usvg::Opacity) -> Option<Graphic> {
	let covered = covered.transform(pattern.transform().invert()?)?;
	let tile = pattern.rect();
	let (origin, size) = (DVec2::new(tile.x().into(), tile.y().into()), DVec2::new(tile.width().into(), tile.height().into()));
	let first = ((DVec2::new(covered.left().into(), covered.top().into()) - origin) / size).floor();
	let end = ((DVec2::new(covered.right().into(), covered.bottom().into()) - origin) / size).ceil().max(first + 1.);
	let count = end - first;
	if count.x * count.y > MAX_TILES {
		return None;
	}

	let mut prototype = Vector::item_from_bezpath(&kurbo::Rect::new(0., 0., size.x, size.y).to_path(0.));
	stamp_coverage(&mut prototype, Coverage::new_fill(), Graphic::GraphicList(graphic_list(pattern.root())), CoverPlacement::Above);
	let placement = affine(pattern.transform());
	let cells = (first.y as i64..end.y as i64).flat_map(|row| (first.x as i64..end.x as i64).map(move |column| DVec2::new(column as f64, row as f64)));
	let tiles = cells.map(|cell| prototype.clone().with_attribute(ATTR_TRANSFORM, placement * DAffine2::from_translation(origin + cell * size)));

	let mut paint = Item::new_from_element(Graphic::VectorList(tiles.collect()));
	if opacity != usvg::Opacity::ONE {
		paint.set_attribute(ATTR_OPACITY, f64::from(opacity.get()));
	}
	Some(Graphic::GraphicList(List::new_from_item(paint)))
}
