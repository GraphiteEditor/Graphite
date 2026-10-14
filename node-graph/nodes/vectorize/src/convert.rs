use core_types::Color;
use core_types::list::{ATTR_CLIPPING_MASK, ATTR_GRADIENT_FORM, ATTR_GRADIENT_SPACE, ATTR_OPACITY, ATTR_OPACITY_FILL, ATTR_POSITION, ATTR_TRANSFORM, Item, List};
use diffrast::FillRule;
use glam::{DAffine2, DMat2, DVec2, DVec3};
use graphene_hash::CacheHash;
use graphic_types::{CoverPlacement, Coverage, Graphic, Vector, stamp_coverage};
use kurbo::BezPath;
use vector_types::gradient::{Gradient, GradientForm, GradientSpace};
use vector_types::vector::style::{Stroke, StrokeCap, StrokeJoin};
use vtrace::model::{Curve, Geometry, Instance, Model, Object, Paint, StrokeGeometry};

/// Object's Gaussian blur standard deviation in pixels (`f64`), which Graphite cannot draw on vector content.
pub const ATTR_VTRACE_BLUR: &str = "vtrace:blur";
/// Radial gradient's focal point offset from its center in the unit circle's plane (`DVec2`), which Graphite's radial gradients lack.
pub const ATTR_VTRACE_RADIAL_FOCUS: &str = "vtrace:radial_focus";
/// The [`PathGradient`] an object is painted with, drawn by approximating bands since Graphite has no such paint.
pub const ATTR_VTRACE_PATH_GRADIENT: &str = "vtrace:path_gradient";
/// The [`RepeatStructure`] a group of instances was recognized as.
pub const ATTR_VTRACE_REPEAT: &str = "vtrace:repeat";

/// A paint whose color follows the signed distance from a curve, from the gradient's first stop at `near` to its last at `far`.
#[derive(Clone, Debug, Default, PartialEq, CacheHash)]
pub struct PathGradient {
	pub contour: Vector,
	pub closed: bool,
	pub near: f64,
	pub far: f64,
	pub stops: Gradient,
}

/// How a group of instances repeats its definition: the structure's name ("list", "linear", "grid", or "circular") and its parameters.
#[derive(Clone, Debug, Default, PartialEq, CacheHash)]
pub struct RepeatStructure {
	pub name: String,
	pub parameters: Vec<f64>,
}

/// Converts a traced model in pixel coordinates into Graphite content placed in the document by `pixels_to_document`.
pub fn model_to_graphic(model: &Model, pixels_to_document: DAffine2) -> List<Graphic> {
	let converter = Converter { model };

	let mut list = List::new();
	for item in converter.items(&model.items) {
		let mut item = item;
		let transform: DAffine2 = item.attribute_cloned_or(ATTR_TRANSFORM, DAffine2::IDENTITY);
		item.set_attribute(ATTR_TRANSFORM, pixels_to_document * transform);
		list.push(item);
	}
	list
}

struct Converter<'a> {
	model: &'a Model,
}

impl Converter<'_> {
	/// Converts items bottom to top, wrapping each run of consecutive objects that share a clip into one clipped group.
	fn items(&self, items: &[vtrace::model::Item]) -> Vec<Item<Graphic>> {
		let mut converted = Vec::new();
		let mut run: Option<(usize, Vec<Item<Graphic>>)> = None;

		for item in items {
			let (clip, graphic) = match item {
				vtrace::model::Item::Object(object) => (object.clip, self.object(object)),
				vtrace::model::Item::Group { items, .. } => (None, group(self.items(items))),
				vtrace::model::Item::Instance(instance) => (None, self.instance(instance)),
				vtrace::model::Item::Repeat { structure, instances } => {
					let (name, parameters) = structure.parameters();
					let structure = RepeatStructure { name: name.to_string(), parameters };
					let instances = instances.iter().map(|instance| self.instance(instance)).collect();
					(None, group(instances).with_attribute(ATTR_VTRACE_REPEAT, structure))
				}
			};

			// Close the current run when the next item is not clipped the same way
			if run.as_ref().is_some_and(|(run_clip, _)| Some(*run_clip) != clip)
				&& let Some((run_clip, content)) = run.take()
			{
				converted.push(self.clipped(run_clip, content));
			}

			match clip {
				Some(clip) => run.get_or_insert_with(|| (clip, Vec::new())).1.push(graphic),
				None => converted.push(graphic),
			}
		}

		if let Some((run_clip, content)) = run {
			converted.push(self.clipped(run_clip, content));
		}
		converted
	}

	/// Wraps `content` in a group masked by clip `clip_index`, nested in further groups for the clip's ancestors.
	fn clipped(&self, clip_index: usize, content: Vec<Item<Graphic>>) -> Item<Graphic> {
		let Some(clip) = self.model.clips.get(clip_index) else {
			log::error!("Traced object refers to missing clip {clip_index}");
			return group(content);
		};

		// The clip's shape masks what follows it while its own fill stays invisible
		let base = mask_base(loops_to_bezpath(&clip.loops, clip.rule));
		let masked = std::iter::once(base).chain(content.into_iter().map(with_clipping_mask));
		let clipped = group(masked.collect());

		match clip.parent {
			Some(parent) => self.clipped(parent, vec![clipped]),
			None => clipped,
		}
	}

	fn instance(&self, instance: &Instance) -> Item<Graphic> {
		let Some(definition) = self.model.definitions.get(instance.definition) else {
			log::error!("Traced instance refers to missing definition {}", instance.definition);
			return group(Vec::new());
		};

		let transform = DAffine2::from_cols_array(&instance.transform);
		group(self.items(definition)).with_attribute(ATTR_TRANSFORM, transform)
	}

	fn object(&self, object: &Object) -> Item<Graphic> {
		if let Paint::Gradient {
			from,
			gradient: gradient @ diffrast::Gradient::Path { .. },
		} = &object.paint
		{
			return crate::bands::path_gradient(object, *from, gradient);
		}

		let paint = paint(&object.paint);
		let mut vector = match &object.geometry {
			Geometry::Fill { loops, rule } => vector_item(loops_to_bezpath(loops, *rule)).into_fill(paint),
			Geometry::Stroke(stroke) => vector_item(cubics_to_bezpath(&stroke.centerline, stroke.closed)).into_stroke(&stroke_style(stroke), paint),
		};

		if object.opacity < 1. {
			vector.set_attribute(ATTR_OPACITY, object.opacity);
		}
		if object.blur > 0. {
			vector.set_attribute(ATTR_VTRACE_BLUR, object.blur);
		}
		Item::new_from_element(Graphic::from(vector))
	}
}

/// Styling helpers for a freshly built vector item.
pub(crate) trait VectorItemExt {
	fn into_fill(self, paint: Graphic) -> Self;
	fn into_stroke(self, stroke: &Stroke, paint: Graphic) -> Self;
}

impl VectorItemExt for Item<Vector> {
	fn into_fill(mut self, paint: Graphic) -> Self {
		stamp_coverage(&mut self, Coverage::new_fill(), paint, CoverPlacement::Above);
		self
	}

	fn into_stroke(mut self, stroke: &Stroke, paint: Graphic) -> Self {
		stamp_coverage(&mut self, Coverage::new_stroke(stroke), paint, CoverPlacement::Above);
		self
	}
}

fn stroke_style(stroke: &StrokeGeometry) -> Stroke {
	let (dash_lengths, dash_offset) = stroke.dashes.clone().unwrap_or_default();
	Stroke {
		dash_lengths,
		dash_offset,
		cap: match stroke.cap {
			vtrace::stroke::Cap::Butt => StrokeCap::Butt,
			vtrace::stroke::Cap::Round => StrokeCap::Round,
			vtrace::stroke::Cap::Square => StrokeCap::Square,
		},
		join: match stroke.join {
			vtrace::stroke::Join::Miter => StrokeJoin::Miter,
			vtrace::stroke::Join::Round => StrokeJoin::Round,
			vtrace::stroke::Join::Bevel => StrokeJoin::Bevel,
		},
		..Stroke::new(stroke.width)
	}
}

pub(crate) fn vector_item(path: BezPath) -> Item<Vector> {
	Item::new_from_element(Vector::from_bezpath(path))
}

pub(crate) fn group(items: Vec<Item<Graphic>>) -> Item<Graphic> {
	let mut list = List::new();
	items.into_iter().for_each(|item| list.push(item));
	Item::new_from_element(Graphic::Graphic(Box::new(Item::new_from_element(Graphic::GraphicList(list)))))
}

/// An invisible shape whose coverage masks the clipped items that follow it.
pub(crate) fn mask_base(path: BezPath) -> Item<Graphic> {
	let base = vector_item(path).with_attribute(ATTR_OPACITY_FILL, 0.).into_fill(Graphic::from(Color::BLACK));
	Item::new_from_element(Graphic::from(base))
}

/// Flags the content inside `item` as masked by its preceding sibling, where the renderer reads the flag.
pub(crate) fn with_clipping_mask(item: Item<Graphic>) -> Item<Graphic> {
	let (mut graphic, attributes) = item.into_parts();
	match &mut graphic {
		Graphic::Vector(inner) => inner.set_attribute(ATTR_CLIPPING_MASK, true),
		Graphic::Graphic(inner) => inner.set_attribute(ATTR_CLIPPING_MASK, true),
		_ => log::error!("Vectorize produced an unexpected graphic kind to clip"),
	}
	Item::from_parts(graphic, attributes)
}

/// The sRGB gamma-encoded color vtrace works in, as Graphite's linear color.
pub(crate) fn color(color: DVec3, alpha: f64) -> Color {
	let [red, green, blue] = color.as_vec3().to_array();
	Color::from_gamma_srgb_channels(red, green, blue, alpha as f32)
}

fn paint(paint: &Paint) -> Graphic {
	match paint {
		Paint::Solid(solid) => Graphic::from(color(*solid, 1.)),
		Paint::Gradient { from, gradient } => Graphic::Gradient(gradient_item(*from, gradient)),
	}
}

/// A linear or radial gradient in Graphite's form: its stops along the unit line or circle, placed by its transform.
fn gradient_item(from: DVec3, gradient: &diffrast::Gradient) -> Item<Gradient> {
	let stops = gradient_stops(from, gradient);

	let (form, transform) = match *gradient {
		diffrast::Gradient::Linear { start, end, .. } => {
			let (start, axis) = (start.as_dvec2(), (end - start).as_dvec2());
			(GradientForm::Linear, DAffine2::from_mat2_translation(DMat2::from_cols(axis, axis.perp()), start))
		}
		diffrast::Gradient::Radial { center, radius, transform, .. } => {
			let plane = DMat2::from_cols_array(&transform.to_cols_array().map(f64::from)) * radius as f64;
			(GradientForm::Radial, DAffine2::from_mat2_translation(plane, center.as_dvec2()))
		}
		diffrast::Gradient::Path { .. } => (GradientForm::Linear, DAffine2::IDENTITY),
	};

	let mut item = Item::new_from_element(stops)
		.with_attribute(ATTR_GRADIENT_FORM, form)
		.with_attribute(ATTR_GRADIENT_SPACE, GradientSpace::RgbGamma)
		.with_attribute(ATTR_TRANSFORM, transform);

	if let diffrast::Gradient::Radial { focus, radius, .. } = *gradient
		&& focus != glam::Vec2::ZERO
	{
		item.set_attribute(ATTR_VTRACE_RADIAL_FOCUS, focus.as_dvec2() / radius as f64);
	}
	item
}

/// The ramp as straight-alpha stops, which Graphite interpolates like SVG.
pub(crate) fn gradient_stops(from: DVec3, gradient: &diffrast::Gradient) -> Gradient {
	let mut stops = List::new();
	for (offset, stop_color, alpha) in gradient.svg_stops(from.as_vec3()) {
		let stop = Item::new_from_element(color(stop_color.as_dvec3(), alpha as f64)).with_attribute(ATTR_POSITION, offset as f64);
		stops.push(stop);
	}
	Gradient(stops)
}

pub(crate) fn cubics_to_bezpath(cubics: &[Curve], closed: bool) -> BezPath {
	let point = |p: DVec2| kurbo::Point::new(p.x, p.y);

	let mut path = BezPath::new();
	let Some(first) = cubics.first() else { return path };
	path.move_to(point(first[0]));
	for [_, handle_start, handle_end, end] in cubics {
		path.curve_to(point(*handle_start), point(*handle_end), point(*end));
	}
	if closed {
		path.close_path();
	}
	path
}

pub(crate) fn loops_to_bezpath(loops: &[Vec<Curve>], rule: FillRule) -> BezPath {
	let mut subpaths: Vec<BezPath> = loops.iter().filter(|cubics| !cubics.is_empty()).map(|cubics| cubics_to_bezpath(cubics, true)).collect();
	if rule == FillRule::EvenOdd {
		orient_for_nonzero(&mut subpaths);
	}

	let mut path = BezPath::new();
	subpaths.iter().flat_map(|subpath| subpath.elements()).for_each(|&element| path.push(element));
	path
}

/// Orients the loops so Graphite's nonzero fill rule covers what the even-odd rule would: each loop winds
/// in the direction of its nesting depth's parity, so a hole always cancels the loop around it. O(n²) in the loop count.
fn orient_for_nonzero(subpaths: &mut [BezPath]) {
	use kurbo::{ParamCurve, Shape};

	// A segment's midpoint rather than its start, since loops may touch at their corners
	let depths: Vec<usize> = subpaths
		.iter()
		.enumerate()
		.map(|(index, subpath)| {
			let Some(sample) = subpath.segments().next().map(|segment| segment.eval(0.5)) else { return 0 };
			subpaths.iter().enumerate().filter(|&(other, path)| other != index && path.winding(sample) != 0).count()
		})
		.collect();

	for (subpath, depth) in subpaths.iter_mut().zip(depths) {
		let counterclockwise = subpath.area() > 0.;
		if counterclockwise != (depth % 2 == 0) {
			*subpath = subpath.reverse_subpaths();
		}
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use kurbo::Shape;

	#[test]
	fn nested_loops_alternate_winding() {
		let circle = |radius: f64| kurbo::Circle::new((0., 0.), radius).to_path(0.1);
		let mut subpaths = [circle(10.), circle(6.), circle(3.)];

		orient_for_nonzero(&mut subpaths);

		let counterclockwise = subpaths.map(|subpath| subpath.area() > 0.);
		assert_eq!(counterclockwise, [true, false, true]);
	}
}
