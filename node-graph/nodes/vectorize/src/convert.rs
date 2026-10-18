use core_types::Color;
use core_types::list::{ATTR_CLIPPING_MASK, ATTR_GRADIENT_FORM, ATTR_GRADIENT_SPACE, ATTR_OPACITY, ATTR_OPACITY_FILL, ATTR_POSITION, ATTR_TRANSFORM, Item, List};
use glam::{DAffine2, DMat2, DVec2, DVec3};
use graphene_hash::CacheHash;
use graphic_types::{CoverPlacement, Coverage, Graphic, Vector, stamp_coverage};
use kurbo::BezPath;
use ollev::model::{Curve, FillRule, Geometry, GradientShape, Instance, Model, Object, Paint, StrokeGeometry, Structure};
use vector_types::gradient::{Gradient, GradientForm, GradientSpace};
use vector_types::vector::style::{Stroke, StrokeCap, StrokeJoin};

/// The [`RepeatStructure`] a group of instances was recognized as.
pub const ATTR_OLLEV_REPEAT: &str = "ollev:repeat";

/// How a group of instances repeats its definition, instance `k` counted from 0 and moved from the first, in the traced image's pixels.
#[derive(Clone, Debug, Default, PartialEq, CacheHash)]
pub enum RepeatStructure {
	/// Each instance placed by its own transform.
	#[default]
	List,
	/// Instance `k` moved by `k * step`.
	Linear { step: DVec2 },
	/// Instance `k` moved by `(k % columns) * a + (k / columns) * b`.
	Grid { a: DVec2, b: DVec2, columns: usize },
	/// Instance `k` turned about `center` by `k * step` radians.
	Circular { center: DVec2, step: f64 },
}

impl From<&Structure> for RepeatStructure {
	fn from(structure: &Structure) -> Self {
		match *structure {
			Structure::List => Self::List,
			Structure::Linear { step } => Self::Linear { step },
			Structure::Grid { a, b, columns } => Self::Grid { a, b, columns },
			Structure::Circular { center, step } => Self::Circular { center, step },
		}
	}
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
	fn items(&self, items: &[ollev::model::Item]) -> Vec<Item<Graphic>> {
		let mut converted = Vec::new();
		let mut run: Option<(usize, Vec<Item<Graphic>>)> = None;

		for item in items {
			let (clip, graphic) = match item {
				ollev::model::Item::Object(object) => (object.clip, self.object(object)),
				ollev::model::Item::Group { items, .. } => (None, group(self.items(items))),
				ollev::model::Item::Instance(instance) => (None, self.instance(instance)),
				ollev::model::Item::Repeat { structure, instances } => {
					let instances = instances.iter().map(|instance| self.instance(instance)).collect();
					(None, group(instances).with_attribute(ATTR_OLLEV_REPEAT, RepeatStructure::from(structure)))
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

	/// An object of the lowered model: no even-odd fill, path gradient, blur or radial focus is left (see [`crate::CAPABILITIES`]).
	fn object(&self, object: &Object) -> Item<Graphic> {
		let paint = paint(&object.paint);
		let mut vector = match &object.geometry {
			Geometry::Fill { loops, rule } => vector_item(loops_to_bezpath(loops, *rule)).into_fill(paint),
			Geometry::Stroke(stroke) => vector_item(cubics_to_bezpath(&stroke.centerline, stroke.closed)).into_stroke(&stroke_style(stroke), paint),
		};

		if object.opacity < 1. {
			vector.set_attribute(ATTR_OPACITY, object.opacity);
		}
		Item::new_from_element(Graphic::from(vector))
	}
}

/// Styling helpers for a freshly built vector item.
trait VectorItemExt {
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
			ollev::model::Cap::Butt => StrokeCap::Butt,
			ollev::model::Cap::Round => StrokeCap::Round,
			ollev::model::Cap::Square => StrokeCap::Square,
		},
		join: match stroke.join {
			ollev::model::Join::Miter => StrokeJoin::Miter,
			ollev::model::Join::Round => StrokeJoin::Round,
			ollev::model::Join::Bevel => StrokeJoin::Bevel,
		},
		..Stroke::new(stroke.width)
	}
}

fn vector_item(path: BezPath) -> Item<Vector> {
	Vector::item_from_bezpath(&path)
}

fn group(items: Vec<Item<Graphic>>) -> Item<Graphic> {
	let mut list = List::new();
	items.into_iter().for_each(|item| list.push(item));
	Item::new_from_element(Graphic::Graphic(Box::new(Item::new_from_element(Graphic::GraphicList(list)))))
}

/// An invisible shape whose coverage masks the clipped items that follow it.
fn mask_base(path: BezPath) -> Item<Graphic> {
	let base = vector_item(path).with_attribute(ATTR_OPACITY_FILL, 0.).into_fill(Graphic::from(Color::BLACK));
	Item::new_from_element(Graphic::from(base))
}

/// Flags the content inside `item` as masked by its preceding sibling, where the renderer reads the flag.
fn with_clipping_mask(item: Item<Graphic>) -> Item<Graphic> {
	let (mut graphic, attributes) = item.into_parts();
	match &mut graphic {
		Graphic::Vector(inner) => inner.set_attribute(ATTR_CLIPPING_MASK, true),
		Graphic::Graphic(inner) => inner.set_attribute(ATTR_CLIPPING_MASK, true),
		_ => log::error!("Vectorize produced an unexpected graphic kind to clip"),
	}
	Item::from_parts(graphic, attributes)
}

/// The sRGB gamma-encoded color ollev works in, as Graphite's linear color.
fn color(color: DVec3, alpha: f64) -> Color {
	let [red, green, blue] = color.as_vec3().to_array();
	Color::from_gamma_srgb_channels(red, green, blue, alpha as f32)
}

fn paint(paint: &Paint) -> Graphic {
	match paint {
		Paint::Solid(solid) => Graphic::from(color(*solid, 1.)),
		Paint::Gradient(gradient) => Graphic::Gradient(gradient_item(gradient)),
	}
}

/// A linear or radial gradient in Graphite's form: its stops along the unit line or circle, placed by its transform.
fn gradient_item(gradient: &ollev::model::Gradient) -> Item<Gradient> {
	let mut stops = List::new();
	for stop in gradient.stops() {
		let [red, green, blue, alpha] = stop.color;
		let stop_color = color(DVec3::new(red, green, blue), alpha);
		stops.push(Item::new_from_element(stop_color).with_attribute(ATTR_POSITION, stop.offset));
	}

	let (form, transform) = match gradient.shape() {
		GradientShape::Linear { start, end } => {
			let axis = end - start;
			(GradientForm::Linear, DAffine2::from_mat2_translation(DMat2::from_cols(axis, axis.perp()), start))
		}
		GradientShape::Radial { transform, .. } => (GradientForm::Radial, DAffine2::from_cols_array(&transform)),
		GradientShape::Path { .. } => {
			log::error!("Vectorize got a path gradient its lowering should have drawn as bands");
			(GradientForm::Linear, DAffine2::IDENTITY)
		}
	};

	Item::new_from_element(Gradient(stops))
		.with_attribute(ATTR_GRADIENT_FORM, form)
		.with_attribute(ATTR_GRADIENT_SPACE, GradientSpace::RgbGamma)
		.with_attribute(ATTR_TRANSFORM, transform)
}

fn cubics_to_bezpath(cubics: &[Curve], closed: bool) -> BezPath {
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

/// The loops as one path; the lowering has wound them for Graphite's nonzero fill rule.
fn loops_to_bezpath(loops: &[Vec<Curve>], rule: FillRule) -> BezPath {
	if rule == FillRule::EvenOdd {
		log::error!("Vectorize got an even-odd fill its lowering should have wound for nonzero");
	}
	let mut path = BezPath::new();
	for cubics in loops.iter().filter(|cubics| !cubics.is_empty()) {
		cubics_to_bezpath(cubics, true).elements().iter().for_each(|&element| path.push(element));
	}
	path
}
