//! Converts SVG documents into [`Graphic`] content.
//!
//! Parsing and style resolution are left to `usvg`. This crate maps the resulting tree onto Graphene's graphic types.
//! The conversion is best effort: what has no counterpart is left out.

mod blend_mode;
mod clip;
mod even_odd;
mod filter;
mod gradient;
mod mask;
mod pattern;
mod raster;
mod stroke;

use core_types::list::{Item, List};
use core_types::{ATTR_BLEND_MODE, ATTR_OPACITY, ATTR_TRANSFORM, Color};
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
		usvg::Node::Image(image) if image.is_visible() => image_item(image),
		usvg::Node::Text(text) => Some(group_item(text.flattened())),
		usvg::Node::Path(_) | usvg::Node::Image(_) => None,
	}
}

fn group_item(group: &usvg::Group) -> Item<Graphic> {
	let content = graphic_list(group);
	let content = filter::filtered(group, content);
	let content = clip::clipped(group, content);
	let content = mask::masked(group, content);

	let mut item = Item::new_from_element(Graphic::GraphicList(content));
	let transform = affine(group.transform());
	if transform != DAffine2::IDENTITY {
		item.set_attribute(ATTR_TRANSFORM, transform);
	}
	if group.opacity() != usvg::Opacity::ONE {
		item.set_attribute(ATTR_OPACITY, f64::from(group.opacity().get()));
	}
	if let Some(blend_mode) = blend_mode::blend_mode(group) {
		item.set_attribute(ATTR_BLEND_MODE, blend_mode);
	}
	item
}

fn path_item(path: &usvg::Path) -> Item<Graphic> {
	let geometry = bezpath(path.data());
	let mut vector = Vector::item_from_bezpath(&geometry);
	if let Some(fill) = path.fill()
		&& let Some(fill_paint) = paint(fill.paint(), fill.opacity(), path.bounding_box())
	{
		stamp_coverage(&mut vector, Coverage::new_fill(), fill_paint, CoverPlacement::Above);
	}
	if let Some(stroke) = path.stroke()
		&& let Some(stroke_paint) = paint(stroke.paint(), stroke.opacity(), path.stroke_bounding_box())
	{
		let placement = match path.paint_order() {
			usvg::PaintOrder::FillAndStroke => CoverPlacement::Above,
			usvg::PaintOrder::StrokeAndFill => CoverPlacement::Below,
		};
		stamp_coverage(&mut vector, Coverage::new_stroke(&stroke::style(stroke)), stroke_paint, placement);
	}

	let vectors = match path.fill().map(usvg::Fill::rule) {
		Some(usvg::FillRule::EvenOdd) => even_odd::nonzero_vectors(vector, &geometry),
		Some(usvg::FillRule::NonZero) | None => List::new_from_item(vector),
	};
	Item::new_from_element(Graphic::VectorList(vectors))
}

fn image_item(image: &usvg::Image) -> Option<Item<Graphic>> {
	match image.kind() {
		usvg::ImageKind::SVG(tree) => Some(group_item(tree.root())),
		usvg::ImageKind::PNG(data) | usvg::ImageKind::JPEG(data) | usvg::ImageKind::GIF(data) | usvg::ImageKind::WEBP(data) => raster::raster_item(data, image.size()),
	}
}

fn paint(paint: &usvg::Paint, opacity: usvg::Opacity, covered: usvg::Rect) -> Option<Graphic> {
	match paint {
		usvg::Paint::Color(rgb) => Some(Graphic::from(color(*rgb, opacity.get()))),
		usvg::Paint::LinearGradient(linear) => Some(gradient::linear(linear, opacity)),
		usvg::Paint::RadialGradient(radial) => Some(gradient::radial(radial, opacity)),
		usvg::Paint::Pattern(pattern) => pattern::tiled(pattern, covered, opacity),
	}
}

fn color(color: usvg::Color, alpha: f32) -> Color {
	let [red, green, blue] = [color.red, color.green, color.blue].map(|channel| channel as f32 / 255.);
	Color::from_gamma_srgb_channels(red, green, blue, alpha)
}

fn affine(transform: usvg::Transform) -> DAffine2 {
	let usvg::Transform { sx, kx, ky, sy, tx, ty } = transform;
	DAffine2::from_cols_array(&[sx, ky, kx, sy, tx, ty].map(f64::from))
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
