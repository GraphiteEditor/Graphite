//! An item's effects: an ordered list of image operations on the pixels the item renders to, stored in the `ATTR_EFFECTS` attribute.
//! Each row is an [`EffectKind`] whose parameters ride its item as attributes, read as their defaults when absent.
//! Each renderer decodes the rows into [`EffectOperation`]s and carries them out its own way, at the resolution of each render.

use crate::Graphic;
use core_types::Color;
use core_types::color::SRGBA8;
use core_types::graphene_hash::CacheHash;
use core_types::list::{ATTR_EFFECTS, ATTR_OPACITY, ATTR_OPACITY_FILL, Item, List};
use core_types::transfer_curve::TransferCurve;
use dyn_any::DynAny;
use glam::{DMat2, DVec2};
use std::fmt::Debug;
use vector_types::gradient::GradientEvaluator;
use vector_types::{Gradient, GradientSettings};

/// The image operation an [`Effect`] carries out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, CacheHash)]
pub enum EffectKind {
	/// Softens the content by blurring it.
	#[default]
	Blur,
	/// Casts the content's silhouette as a soft shadow beneath it.
	DropShadow,
	/// Casts a soft shadow inward from the content's edges, as if it were cut out of the surface.
	InnerShadow,
	/// Surrounds the content with a soft glow of its silhouette.
	OuterGlow,
}

impl std::fmt::Display for EffectKind {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(match self {
			Self::Blur => "Blur",
			Self::DropShadow => "Drop Shadow",
			Self::InnerShadow => "Inner Shadow",
			Self::OuterGlow => "Outer Glow",
		})
	}
}

/// How a glow fades away from the content's edges.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash, CacheHash, DynAny, node_macro::ChoiceType)]
#[widget(Radio)]
pub enum GlowTechnique {
	/// Fades like a blur of the content, rounding away its finer details.
	#[default]
	Softer,
	/// Fades evenly with the distance from the content's edges, following its details.
	Precise,
}

/// One of the coverages an effect is drawn from, which a render can output in place of the effects' result, so an SVG can embed it as a map of what its filters can't work out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EffectCapture {
	/// The effect's place in its item's list.
	pub index: usize,
	/// Which of the effect's coverages, for an effect drawn from more than one.
	pub plane: usize,
}

/// The offset a shadow cast by light from `angle` (in degrees counterclockwise from the right) falls at `distance` from the content, in the document's y-down space.
pub fn shadow_offset(angle: f64, distance: f64) -> DVec2 {
	let angle = angle.to_radians();
	DVec2::new(-angle.cos(), angle.sin()) * distance
}

/// How a shadow or glow of `size` splits into the round grow its `spread` percentage hardens and the tent blur softening the rest, both lengths in the same units.
/// The grow is whole units, and a tent narrower than two units widens to two, so that any softening shows.
pub fn grow_and_tent(size: f64, spread: f64) -> (f64, f64) {
	let size = size.max(0.);
	let grow = (size * spread.clamp(0., 100.) / 100. + 0.5).floor().min(size);
	let tent = size - grow;

	// Narrower tents are scaled up to two units, joining the identity at zero without a jump
	let tent = if tent >= 2. { tent } else { (2. * tent).min(2.) };
	(grow, tent)
}

/// The strength, from 0 to 1, of a precise glow at `distance` from the content (zero within it), given the `grow` and `size` it was cast with, all in the same units.
/// It falls in a straight line through half strength at the grown edge, reaching nothing a unit past the size.
pub fn precise_falloff(distance: f64, grow: f64, size: f64) -> f64 {
	(0.5 - 0.5 * (distance - grow) / (size - grow + 1.)).clamp(0., 1.)
}

/// Maps a shadow or glow's strength from 0 to 1 through its `range` percentage, which strengthens it so it reaches full strength at that fraction, and then its `contour`.
pub fn shape_strength(strength: f64, range: f64, contour: &impl Fn(f64) -> f64) -> f64 {
	let range = range.clamp(1., 100.) / 100.;
	contour((strength / range).clamp(0., 1.)).clamp(0., 1.)
}

/// One image operation of an [`Effects`] list: an [`EffectKind`] plus its parameters, carried as attributes on the inner item.
/// Each parameter's attribute is named after the input of the node that stamps the kind. Attributes the kind doesn't take are ignored.
#[derive(Clone, Debug, Default, PartialEq, CacheHash)]
pub struct Effect(pub Item<EffectKind>);

impl Effect {
	/// Creates an effect row carrying out the given operation, stamping only the parameters that differ from their implicit defaults.
	pub fn new(operation: &EffectOperation) -> Self {
		let mut item = Item::new_from_element(operation.kind());
		operation.clone().visit_parameters(&mut WriteParameters(&mut item));
		Self(item)
	}

	/// This effect's kind.
	pub fn kind(&self) -> EffectKind {
		*self.0.element()
	}

	/// Decodes the parameters into an [`EffectOperation`], falling back to the default for any absent attribute and to zero for any length that isn't finite.
	pub fn operation(&self) -> EffectOperation {
		let mut operation = EffectOperation::placeholder(self.kind());
		operation.visit_parameters(&mut ReadParameters(&self.0));
		operation.with_finite_lengths()
	}
}

/// An item's ordered list of effects. A blur acts on the result of those before it, while shadows and glows are cast from the content and stack in order beneath it (or within it, for an inner shadow).
#[derive(Clone, Debug, Default, PartialEq, CacheHash)]
pub struct Effects(pub List<Effect>);

impl Effects {
	/// Creates the list of rows carrying out the given operations in order.
	pub fn from_operations(operations: impl IntoIterator<Item = EffectOperation>) -> Self {
		let mut effects = Self::default();
		for operation in operations {
			effects.0.push(Item::new_from_element(Effect::new(&operation)));
		}
		effects
	}

	/// The effects in the order they're carried out.
	pub fn effects(&self) -> impl Iterator<Item = &Effect> {
		self.0.iter_element_values()
	}

	/// The effects decoded into the operations renderers carry out, in order.
	pub fn operations(&self) -> impl Iterator<Item = EffectOperation> + '_ {
		self.effects().map(Effect::operation)
	}

	/// Whether carrying out the effects leaves the content as it is, like a blur of no radius, so renderers can skip them.
	pub fn change_nothing(&self) -> bool {
		self.operations().all(|operation| operation.changes_nothing())
	}

	/// How far past the content the effects together can paint, in document pixels.
	pub fn reach(&self) -> f64 {
		self.operations().filter(|operation| !operation.changes_nothing()).map(|operation| operation.reach()).sum()
	}

	pub fn len(&self) -> usize {
		self.0.len()
	}

	pub fn is_empty(&self) -> bool {
		self.0.is_empty()
	}
}

/// The most that a length in the document spans along each axis of a space, given the linear map from the document into that space.
pub fn spans_along_axes(document_to_space: DMat2) -> DVec2 {
	DVec2::new(document_to_space.row(0).length(), document_to_space.row(1).length())
}

/// For PSD interop, an outer glow filled with a gradient fades out once its strength before range and contour falls below this.
pub const GRADIENT_GLOW_FADE: f64 = 0.12;

/// What fills a glow: a solid color its strength fades, or a gradient its strength runs across, from the gradient's start at full strength to its end at none.
pub enum GlowFill {
	Solid(Color),
	Gradient {
		evaluator: Box<GradientEvaluator>,
		visible: bool,
		/// The gradient item's opacity, which fades the glow after the gradient's own alpha.
		opacity: f32,
	},
}

impl GlowFill {
	/// The fill a glow's `paint` makes, or `None` for a paint other than a color or a gradient, which fills nothing.
	pub fn of(paint: &Graphic) -> Option<Self> {
		// Faded by the paint's opacity attributes, like any paint
		let opacity = |item_opacity: Option<&f64>, fill_opacity: Option<&f64>| (item_opacity.copied().unwrap_or(1.) * fill_opacity.copied().unwrap_or(1.)) as f32;
		let solid = |item: &Item<Color>| {
			let color = *item.element();
			Self::Solid(color.with_alpha(color.a() * opacity(item.attribute(ATTR_OPACITY), item.attribute(ATTR_OPACITY_FILL))))
		};
		let gradient = |item: &Item<Gradient>| Self::Gradient {
			evaluator: Box::new(item.element().evaluator(GradientSettings::from_item_attributes(item))),
			visible: item.element().0.iter_element_values().any(|stop| stop.a() > 0.),
			opacity: opacity(item.attribute(ATTR_OPACITY), item.attribute(ATTR_OPACITY_FILL)),
		};

		match paint {
			Graphic::Color(item) => Some(solid(item)),
			Graphic::ColorList(list) => list.clone_item(0).map(|item| solid(&item)),
			Graphic::Gradient(item) => Some(gradient(item)),
			Graphic::GradientList(list) => list.clone_item(0).map(|item| gradient(&item)),
			_ => None,
		}
	}

	/// Whether the fill paints nothing anywhere.
	pub fn is_invisible(&self) -> bool {
		match self {
			Self::Solid(color) => color.a() <= 0.,
			Self::Gradient { visible, opacity, .. } => !visible || *opacity <= 0.,
		}
	}

	/// For a gradient, its straight gamma-encoded colors with alpha at `count` evenly spaced positions from its start to its end, and its opacity.
	pub fn gradient_samples(&self, count: usize) -> Option<(Vec<[f32; 4]>, f32)> {
		let Self::Gradient { evaluator, opacity, .. } = self else { return None };
		let samples = (0..count).map(|index| evaluator.evaluate(index as f64 / (count - 1).max(1) as f64).to_gamma_srgb_channels()).collect();
		Some((samples, *opacity))
	}

	/// The color with its alpha that a glow paints where its `strength` is, after its range and contour, given its `raw` strength before them.
	/// For PSD interop, a gradient's own alpha is the glow's, which an outer glow (if `fades`) fades out by as its raw strength nears none.
	pub fn color_at(&self, strength: f64, raw: f64, fades: bool) -> Color {
		match self {
			Self::Solid(color) => color.with_alpha(color.a() * strength as f32),
			Self::Gradient { evaluator, opacity, .. } => {
				let color = evaluator.evaluate(1. - strength);
				let fade = if fades { (raw / GRADIENT_GLOW_FADE).min(1.) } else { 1. };
				color.with_alpha(color.a().min(fade as f32) * opacity)
			}
		}
	}
}

/// Appends an effect to the item's list, after those already applied to it.
pub fn stamp_effect<T>(item: &mut Item<T>, effect: Effect) {
	item.attribute_mut_or_insert_default::<Effects>(ATTR_EFFECTS).0.push(Item::new_from_element(effect));
}

/// An effect's parameters decoded from its row, the form that renderers carry it out from. Lengths are in document pixels.
#[derive(Clone, Debug, PartialEq)]
pub enum EffectOperation {
	Blur {
		/// The standard deviation of the Gaussian, or the half width of the box.
		radius: f64,
		/// Whether to average a box of pixels rather than weighting them by a Gaussian.
		box_blur: bool,
		/// Whether to blend the colors with their gamma encoding rather than in linear light.
		gamma: bool,
	},
	DropShadow {
		/// With the shadow's opacity.
		color: Color,
		/// The direction the light comes from, in degrees counterclockwise from the right.
		angle: f64,
		distance: f64,
		/// The percentage of the size that hardens into a grow of the silhouette rather than softening.
		spread: f64,
		size: f64,
		contour: TransferCurve,
		/// Whether the content hides the shadow where it covers it, even where a fill fades the content.
		knockout: bool,
	},
	InnerShadow {
		color: Color,
		angle: f64,
		distance: f64,
		/// The percentage of the size that hardens into a grow inward from the edges rather than softening.
		choke: f64,
		size: f64,
		contour: TransferCurve,
	},
	OuterGlow {
		/// A solid color with the glow's opacity, or a gradient running from the content's edge outward.
		color: Graphic,
		technique: GlowTechnique,
		spread: f64,
		size: f64,
		/// The percentage of the glow's strength it reaches full opacity at.
		range: f64,
		contour: TransferCurve,
	},
}

impl EffectOperation {
	/// The operation an effect of this kind carries out when its row gives none of its parameters, matching the defaults of the node that stamps it.
	pub fn default_for(kind: EffectKind) -> Self {
		Effect(Item::new_from_element(kind)).operation()
	}

	pub fn kind(&self) -> EffectKind {
		match self {
			Self::Blur { .. } => EffectKind::Blur,
			Self::DropShadow { .. } => EffectKind::DropShadow,
			Self::InnerShadow { .. } => EffectKind::InnerShadow,
			Self::OuterGlow { .. } => EffectKind::OuterGlow,
		}
	}

	/// Whether this operation leaves the content as it is, like a blur of no radius or a shadow of no opacity.
	pub fn changes_nothing(&self) -> bool {
		match self {
			Self::Blur { radius, .. } => *radius <= 0.,
			Self::DropShadow { color, .. } | Self::InnerShadow { color, .. } => color.a() <= 0.,
			Self::OuterGlow { color, .. } => GlowFill::of(color).is_none_or(|fill| fill.is_invisible()),
		}
	}

	/// How far past the content this operation can paint, or reads from, in document pixels.
	pub fn reach(&self) -> f64 {
		match self {
			// A Gaussian is negligible past three standard deviations
			Self::Blur { radius, .. } => 3. * radius.max(0.),
			// An inner shadow paints within the content but reads this far out to find the edges it casts from.
			// Shadows and glows end within their size, past which a precise glow fades over one more unit.
			Self::DropShadow { distance, size, .. } | Self::InnerShadow { distance, size, .. } => distance.abs() + size.max(0.) + 1.,
			Self::OuterGlow { size, .. } => size.max(0.) + 1.,
		}
	}

	/// This operation with each length that isn't finite (like one computed by dividing by zero) made zero, since no renderer can carry it out.
	fn with_finite_lengths(mut self) -> Self {
		let finite = |length: &mut f64| {
			if !length.is_finite() {
				*length = 0.;
			}
		};

		match &mut self {
			Self::Blur { radius, .. } => finite(radius),
			Self::DropShadow { angle, distance, spread, size, .. }
			| Self::InnerShadow {
				angle, distance, choke: spread, size, ..
			} => {
				[angle, distance, spread, size].into_iter().for_each(finite);
			}
			Self::OuterGlow { spread, size, range, .. } => [spread, size, range].into_iter().for_each(finite),
		}
		self
	}

	/// An operation of this kind with arbitrary parameters, for a visitor to fill in.
	fn placeholder(kind: EffectKind) -> Self {
		match kind {
			EffectKind::Blur => Self::Blur {
				radius: 0.,
				box_blur: false,
				gamma: false,
			},
			EffectKind::DropShadow => Self::DropShadow {
				color: Color::TRANSPARENT,
				angle: 0.,
				distance: 0.,
				spread: 0.,
				size: 0.,
				contour: TransferCurve::default(),
				knockout: false,
			},
			EffectKind::InnerShadow => Self::InnerShadow {
				color: Color::TRANSPARENT,
				angle: 0.,
				distance: 0.,
				choke: 0.,
				size: 0.,
				contour: TransferCurve::default(),
			},
			EffectKind::OuterGlow => Self::OuterGlow {
				color: Graphic::default(),
				technique: GlowTechnique::default(),
				spread: 0.,
				size: 0.,
				range: 0.,
				contour: TransferCurve::default(),
			},
		}
	}

	/// Visits each parameter with the attribute it rides and its implicit default, the one listing that writing and reading rows share.
	/// For PSD interop, the defaults are those of layer styles, so imported ones that leave a setting out read the same.
	fn visit_parameters(&mut self, visitor: &mut impl ParameterVisitor) {
		let shadow_color = Color::from(SRGBA8::new(0, 0, 0, 0xbf));
		let glow_color = Color::from(SRGBA8::new(0xff, 0xff, 0xbe, 0xbf));

		match self {
			Self::Blur { radius, box_blur, gamma } => {
				visitor.parameter("radius", radius, 0.);
				visitor.parameter("box_blur", box_blur, false);
				visitor.parameter("gamma", gamma, false);
			}
			Self::DropShadow {
				color,
				angle,
				distance,
				spread,
				size,
				contour,
				knockout,
			} => {
				visitor.parameter("color", color, shadow_color);
				visitor.parameter("angle", angle, 120.);
				visitor.parameter("distance", distance, 5.);
				visitor.parameter("spread", spread, 0.);
				visitor.parameter("size", size, 5.);
				visitor.parameter("contour", contour, TransferCurve::default());
				visitor.parameter("knockout", knockout, true);
			}
			Self::InnerShadow {
				color,
				angle,
				distance,
				choke,
				size,
				contour,
			} => {
				visitor.parameter("color", color, shadow_color);
				visitor.parameter("angle", angle, 120.);
				visitor.parameter("distance", distance, 5.);
				visitor.parameter("choke", choke, 0.);
				visitor.parameter("size", size, 5.);
				visitor.parameter("contour", contour, TransferCurve::default());
			}
			Self::OuterGlow {
				color,
				technique,
				spread,
				size,
				range,
				contour,
			} => {
				visitor.parameter("color", color, Graphic::from(glow_color));
				visitor.parameter("technique", technique, GlowTechnique::default());
				visitor.parameter("spread", spread, 0.);
				visitor.parameter("size", size, 5.);
				visitor.parameter("range", range, 50.);
				visitor.parameter("contour", contour, TransferCurve::default());
			}
		}
	}
}

/// What a value needs to ride an attribute.
pub(crate) trait ParameterValue: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static {}
impl<T: Clone + Send + Sync + Default + Debug + PartialEq + CacheHash + 'static> ParameterValue for T {}

/// Handles each parameter of an operation, given the attribute key it rides and its implicit default.
pub(crate) trait ParameterVisitor {
	fn parameter<T: ParameterValue>(&mut self, key: &str, value: &mut T, default: T);
}

/// Stamps each parameter that differs from its default onto a row.
struct WriteParameters<'a>(&'a mut Item<EffectKind>);

impl ParameterVisitor for WriteParameters<'_> {
	fn parameter<T: ParameterValue>(&mut self, key: &str, value: &mut T, default: T) {
		if *value != default {
			self.0.set_attribute(key, value.clone());
		}
	}
}

/// Reads each parameter from a row, or its default where the row lacks it.
struct ReadParameters<'a>(&'a Item<EffectKind>);

impl ParameterVisitor for ReadParameters<'_> {
	fn parameter<T: ParameterValue>(&mut self, key: &str, value: &mut T, default: T) {
		*value = self.0.attribute::<T>(key).cloned().unwrap_or(default);
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn rows_round_trip_their_operations_eliding_defaults() {
		let operations = [
			EffectOperation::Blur {
				radius: 6.,
				box_blur: true,
				gamma: false,
			},
			EffectOperation::default_for(EffectKind::DropShadow),
			EffectOperation::InnerShadow {
				color: Color::RED,
				angle: 30.,
				distance: 3.,
				choke: 25.,
				size: 6.,
				contour: TransferCurve::new(vec![DVec2::ZERO, DVec2::new(0.5, 1.), DVec2::ONE]),
			},
			EffectOperation::OuterGlow {
				color: Graphic::from(Color::BLUE),
				technique: GlowTechnique::Precise,
				spread: 10.,
				size: 8.,
				range: 75.,
				contour: TransferCurve::default(),
			},
		];

		for operation in operations {
			assert_eq!(Effect::new(&operation).operation(), operation);
		}

		let default_row = Effect::new(&EffectOperation::default_for(EffectKind::DropShadow));
		assert_eq!(default_row.0.attributes().iter_any().count(), 0, "a default operation stamps no parameters");
	}

	#[test]
	fn lengths_that_arent_finite_decode_as_zero() {
		let row = Effect::new(&EffectOperation::DropShadow {
			color: Color::BLACK,
			angle: 90.,
			distance: f64::NAN,
			spread: 3.,
			size: f64::INFINITY,
			contour: TransferCurve::default(),
			knockout: true,
		});

		let EffectOperation::DropShadow { distance, spread, size, .. } = row.operation() else {
			panic!("the row changed kind")
		};
		assert_eq!((distance, spread, size), (0., 3., 0.));
	}

	#[test]
	fn shadows_fall_away_from_the_light() {
		// Light from the upper left casts the shadow down and to the right
		let offset = shadow_offset(120., 10.);
		assert!((offset - DVec2::new(5., 10. * 60_f64.to_radians().sin())).length() < 1e-9);
		assert!((shadow_offset(90., 5.) - DVec2::new(0., 5.)).length() < 1e-9);
	}

	#[test]
	fn spread_hardens_whole_units_of_the_size() {
		assert_eq!(grow_and_tent(16., 0.), (0., 16.));
		assert_eq!(grow_and_tent(16., 50.), (8., 8.));
		// Growing by 3.3 rounds to 3, and by 3.5 rounds up to 4
		assert_eq!(grow_and_tent(10., 33.), (3., 7.));
		assert_eq!(grow_and_tent(7., 50.), (4., 3.));
		assert_eq!(grow_and_tent(16., 100.), (16., 0.));
		// A tent of one unit softens like one of two
		assert_eq!(grow_and_tent(1., 0.), (0., 2.));
		assert_eq!(grow_and_tent(3., 50.), (2., 2.));
	}

	#[test]
	fn precise_glows_fall_through_half_strength_at_the_grown_edge() {
		assert_eq!(precise_falloff(8., 8., 16.), 0.5);
		assert!((precise_falloff(1., 0., 16.) - 16. / 34.).abs() < 1e-12);
		assert_eq!(precise_falloff(17., 0., 16.), 0.);
		assert_eq!(precise_falloff(0., 16., 16.), 1.);
	}

	#[test]
	fn only_effects_that_show_change_something() {
		assert!(Effects::from_operations([EffectOperation::default_for(EffectKind::Blur)]).change_nothing());
		assert!(!Effects::from_operations([EffectOperation::default_for(EffectKind::DropShadow)]).change_nothing());
	}
}
