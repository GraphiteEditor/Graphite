//! Converts SVG documents into [`Graphic`] content.
//!
//! Parsing and style resolution are left to `usvg`. This crate maps the resulting tree onto Graphene's graphic types.
//! The conversion is best effort: what has no counterpart is left out.

use core_types::list::{Item, List};
use core_types::{ATTR_OPACITY, ATTR_TRANSFORM, Color};
use glam::DAffine2;
use graphic_types::{CoverPlacement, Coverage, Graphic, Vector, stamp_coverage};
use kurbo::{BezPath, PathEl, Point};
use usvg::tiny_skia_path::{self, PathSegment};

pub use usvg::{Error, Options};

pub fn convert(svg: &str, options: &Options) -> Result<List<Graphic>, Error> {
	let tree = usvg::Tree::from_str(svg, options)?;
	Ok(graphic_list(tree.root()))
}

fn graphic_list(group: &usvg::Group) -> List<Graphic> {
	group.children().iter().filter_map(graphic_item).collect()
}

fn graphic_item(node: &usvg::Node) -> Option<Item<Graphic>> {
	match node {
		usvg::Node::Group(group) => Some(group_item(group)),
		usvg::Node::Path(path) if path.is_visible() => Some(path_item(path)),
		usvg::Node::Text(text) => Some(group_item(text.flattened())),
		usvg::Node::Path(_) | usvg::Node::Image(_) => None,
	}
}

fn group_item(group: &usvg::Group) -> Item<Graphic> {
	let usvg::Transform { sx, kx, ky, sy, tx, ty } = group.transform();
	let transform = DAffine2::from_cols_array(&[sx, ky, kx, sy, tx, ty].map(f64::from));

	let mut item = Item::new_from_element(Graphic::GraphicList(graphic_list(group)));
	if transform != DAffine2::IDENTITY {
		item.set_attribute(ATTR_TRANSFORM, transform);
	}
	if group.opacity() != usvg::Opacity::ONE {
		item.set_attribute(ATTR_OPACITY, f64::from(group.opacity().get()));
	}
	item
}

fn path_item(path: &usvg::Path) -> Item<Graphic> {
	let mut vector = Vector::item_from_bezpath(&bezpath(path.data()));
	if let Some(fill) = path.fill()
		&& let usvg::Paint::Color(color) = fill.paint()
	{
		let color = Color::from_gamma_srgb_channels(color.red as f32 / 255., color.green as f32 / 255., color.blue as f32 / 255., fill.opacity().get());
		stamp_coverage(&mut vector, Coverage::new_fill(), Graphic::from(color), CoverPlacement::Above);
	}

	Item::new_from_element(Graphic::VectorList(List::new_from_item(vector)))
}

fn bezpath(path: &tiny_skia_path::Path) -> BezPath {
	let point = |point: tiny_skia_path::Point| Point::new(point.x.into(), point.y.into());

	path.segments()
		.map(|segment| match segment {
			PathSegment::MoveTo(end) => PathEl::MoveTo(point(end)),
			PathSegment::LineTo(end) => PathEl::LineTo(point(end)),
			PathSegment::QuadTo(handle, end) => PathEl::QuadTo(point(handle), point(end)),
			PathSegment::CubicTo(start_handle, end_handle, end) => PathEl::CurveTo(point(start_handle), point(end_handle), point(end)),
			PathSegment::Close => PathEl::ClosePath,
		})
		.collect()
}
