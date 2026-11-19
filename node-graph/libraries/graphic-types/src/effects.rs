//! An item's effects: an ordered list of image operations on the pixels the item renders to, stored in the `ATTR_EFFECTS` attribute.
//! Each row is an [`EffectKind`] whose parameters ride its item as attributes, read as their defaults when absent.
//! Each renderer decodes the rows into [`EffectOperation`]s and carries them out its own way, at the resolution of each render.

use crate::Graphic;
use crate::adjustment::Adjustment;
use crate::masks::{MaskMode, mask_feather_sigma};
use crate::parameters::{ParameterVisitor, ReadParameters, WriteParameters};
use core_types::ATTR_TRANSFORM;
use core_types::Color;
use core_types::blending::BlendMode;
use core_types::color::SRGBA8;
use core_types::graphene_hash::CacheHash;
use core_types::list::{ATTR_EFFECTS, ATTR_OPACITY, ATTR_OPACITY_FILL, Item, List};
use core_types::transfer_curve::{TransferCurve, TransferCurveEvaluator};
use dyn_any::DynAny;
use glam::{DAffine2, DMat2, DVec2, DVec3};
use std::borrow::Cow;
use std::fmt::Debug;
use std::ops::Range;
use vector_types::gradient::GradientEvaluator;
use vector_types::vector::style::StrokeAlign;
use vector_types::{Gradient, GradientSettings};

/// The image operation an [`Effect`] carries out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, CacheHash)]
pub enum EffectKind {
	/// Softens the content by blurring it, evenly or more where its radius mask is brighter.
	#[default]
	Blur,
	/// Casts the content's silhouette as a soft shadow beneath it.
	DropShadow,
	/// Casts a soft shadow inward from the content's edges, as if it were cut out of the surface.
	InnerShadow,
	/// Surrounds the content with a soft glow of its silhouette.
	OuterGlow,
	/// Glows inward from the content's edges, or outward from its center.
	InnerGlow,
	/// Shades the content's interior with soft bands from its silhouette cast two ways.
	Satin,
	/// Paints a band along the content's silhouette.
	Outline,
	/// Lights the content as if raised from the surface or pressed into it, with highlights and shadows along its edges.
	BevelAndEmboss,
	/// Paints over the content within its silhouette, mixing with its colors by a blend mode. A solid color paint is the color overlay adjustment.
	PaintOverlay,
	/// Hides the content where another graphic is dark or transparent.
	Mask,
	/// Fades the content out toward its edges, reaching nothing at them.
	Feather,
	// The adjustments, which change each color of the content on its own, as their nodes of the same names do
	Desaturate,
	GammaCorrection,
	MakeOpaque,
	BrightnessContrast,
	Levels,
	Curves,
	BlackAndWhite,
	HueSaturation,
	Invert,
	Threshold,
	GradientMap,
	Vibrance,
	ChannelMixer,
	SelectiveColor,
	Posterize,
	Exposure,
	ColorBalance,
	PhotoFilter,
	ColorLookup,
}

impl std::fmt::Display for EffectKind {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(match self {
			Self::Blur => "Blur",
			Self::DropShadow => "Drop Shadow",
			Self::InnerShadow => "Inner Shadow",
			Self::OuterGlow => "Outer Glow",
			Self::InnerGlow => "Inner Glow",
			Self::Satin => "Satin",
			Self::Outline => "Outline",
			Self::BevelAndEmboss => "Bevel & Emboss",
			Self::PaintOverlay => "Paint Overlay",
			Self::Mask => "Mask",
			Self::Feather => "Feather",
			Self::Desaturate => "Desaturate",
			Self::GammaCorrection => "Gamma Correction",
			Self::MakeOpaque => "Make Opaque",
			Self::BrightnessContrast => "Brightness/Contrast",
			Self::Levels => "Levels",
			Self::Curves => "Curves",
			Self::BlackAndWhite => "Black & White",
			Self::HueSaturation => "Hue/Saturation",
			Self::Invert => "Invert",
			Self::Threshold => "Threshold",
			Self::GradientMap => "Gradient Map",
			Self::Vibrance => "Vibrance",
			Self::ChannelMixer => "Channel Mixer",
			Self::SelectiveColor => "Selective Color",
			Self::Posterize => "Posterize",
			Self::Exposure => "Exposure",
			Self::ColorBalance => "Color Balance",
			Self::PhotoFilter => "Photo Filter",
			Self::ColorLookup => "Color Lookup",
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

/// Where an inner glow starts from.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash, CacheHash, DynAny, node_macro::ChoiceType)]
#[widget(Radio)]
pub enum GlowSource {
	/// Glows inward from the edges.
	#[default]
	Edge,
	/// Glows outward from the center.
	Center,
}

/// One of the coverages an effect is drawn from, which a render can output in place of the effects' result, so an SVG can embed it as a map of what its filters can't work out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EffectCapture {
	/// The effect's place in its item's list, or in its run of that list once a renderer splits the list into runs.
	pub index: usize,
	/// Which of the effect's coverages, for an effect drawn from more than one.
	pub plane: usize,
}

impl EffectCapture {
	/// Not an effect's coverage but the share of each pixel within the content that its fill lets what lies beneath show through,
	/// which the parts of interior pieces blending with what lies beneath are cut to (see [`crate::effect_layers::PiecePlace::Through`]).
	pub const THROUGH_MASK: Self = Self { index: usize::MAX, plane: 0 };
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

/// Which slopes a bevel shapes from the content's edges.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash, CacheHash, DynAny, node_macro::ChoiceType)]
#[widget(Dropdown)]
pub enum BevelStyle {
	/// Slopes up from the edges into the content.
	#[default]
	InnerBevel,
	/// Slopes down from the edges away from the content.
	OuterBevel,
	/// Slopes across the edges, half inside the content and half outside.
	Emboss,
	/// Slopes away from the edges both ways, as if pressed into the surface along them.
	PillowEmboss,
}

/// Whether a bevel rises from the surface or sinks into it.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash, CacheHash, DynAny, node_macro::ChoiceType)]
#[widget(Radio)]
pub enum BevelDirection {
	/// Raised from the surface.
	#[default]
	Up,
	/// Pressed into the surface.
	Down,
}

/// How a bevel shapes its slopes from the content's edges.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash, CacheHash, DynAny, node_macro::ChoiceType)]
#[widget(Radio)]
pub enum BevelTechnique {
	/// Rounded slopes, like a blur of the content, which soften its finer details.
	#[default]
	Smooth,
	/// Straight slopes measured from the edges, which follow the content's details.
	ChiselHard,
	/// Straight slopes like Chisel Hard, eased at their edges.
	ChiselSoft,
}

/// The unit vector toward a bevel's light from its `angle` and `altitude` in degrees, with `lengths_to_target` mapping its effect space's directions into the y-down space it's lit in.
pub fn bevel_light(angle: f64, altitude: f64, lengths_to_target: DMat2) -> DVec3 {
	let (angle, altitude) = (angle.to_radians(), altitude.clamp(0., 90.).to_radians());
	let across = (lengths_to_target * DVec2::new(angle.cos(), -angle.sin())).try_normalize().unwrap_or(DVec2::X);
	(across * altitude.cos()).extend(altitude.sin())
}

/// How lit a bevel's surface sloping up by `gradient` (its rise per unit along each axis) is by the `light`, from 0 facing away from it to 1 facing it.
pub fn bevel_lit(gradient: DVec2, light: DVec3) -> f64 {
	(-gradient).extend(1.).normalize().dot(light).clamp(0., 1.)
}

/// A bevel's highlight and shadow from 0 to 1, from how much brighter its surface is lit (after its gloss contour) than a flat surface's brightness `light_z`, negative where it's darker.
/// For PSD interop, the flat brightness stays as it is under a gloss contour, so a curve that moves it shades even flat ground.
pub fn bevel_lighting(brighter: f64, light_z: f64) -> (f64, f64) {
	let highlight = if light_z < 1. { brighter / (1. - light_z) } else { 0. };
	let shadow = if light_z > 0. { -brighter / light_z } else { 0. };
	(highlight.clamp(0., 1.), shadow.clamp(0., 1.))
}

/// For PSD interop, under a gloss contour or a texture a bevel's highlight and shadow beneath the content reach no more than its height before its contour divided by this,
/// so the flat ground they shade ends at the slopes' foot. It's the same as [`GRADIENT_GLOW_FADE`].
pub const BEVEL_FOOT_FADE: f64 = 0.12;

/// For PSD interop, the offsets from each pixel toward its right and lower neighbors at which an antialiased contour samples it, nine in all.
pub const ANTIALIASED_CONTOUR_OFFSETS: [f64; 3] = [0., 1. / 3., 2. / 3.];

/// Where a bevel's height `t` (0 at the foot of its slope and 1 on its top) falls along its contour, given the `range` percentage of the slope the contour spans.
/// For PSD interop, an inner bevel's contour spans the range down from the top, an outer bevel's spans it up from the foot, and an emboss's spans the whole slope.
pub fn slope_contour_input(t: f64, style: BevelStyle, range: f64) -> f64 {
	let range = range.clamp(1., 100.) / 100.;
	match style {
		BevelStyle::InnerBevel => 1. - (1. - t) / range,
		BevelStyle::OuterBevel => t / range,
		BevelStyle::Emboss | BevelStyle::PillowEmboss => t,
	}
}

/// Whether a bevel's contour reshapes its slopes, as any curve does, and a straight one does when it spans part of an inner or outer bevel's slopes.
pub fn slope_contour_reshapes(contour: &TransferCurve, style: BevelStyle, range: f64) -> bool {
	!contour.is_identity() || (range < 100. && matches!(style, BevelStyle::InnerBevel | BevelStyle::OuterBevel))
}

/// The slopes a bevel's contour continues along past its start and its end, for PSD interop, measured across its first and last steps of 1/255 within the curve's 0 to 1.
pub fn contour_end_slopes(contour: &TransferCurveEvaluator) -> (f64, f64) {
	let step = 1. / 255.;
	let at = |x: f64| contour.evaluate(x).clamp(0., 1.);
	((at(step) - at(0.)) / step, (at(1.) - at(1. - step)) / step)
}

/// A bevel's contour at `x`, from 0 to 1 within the curve, and continued past its ends along the `slopes` of [`contour_end_slopes`].
pub fn extended_contour(contour: &TransferCurveEvaluator, (start_slope, end_slope): (f64, f64), x: f64) -> f64 {
	match x {
		x if x < 0. => contour.evaluate(0.).clamp(0., 1.) + start_slope * x,
		x if x > 1. => contour.evaluate(1.).clamp(0., 1.) + end_slope * (x - 1.),
		x => contour.evaluate(x).clamp(0., 1.),
	}
}

/// How much a texture's color sinks a bevel's surface, from its premultiplied gamma-encoded channels: their luma, so a transparent texture leaves it flat.
pub fn texture_gray([red, green, blue, _]: [f32; 4]) -> f32 {
	0.299 * red + 0.587 * green + 0.114 * blue
}

/// Whether a paint is a gradient or image without its own placement, which stretches across the content it paints.
pub fn is_unplaced_paint(paint: &Graphic) -> bool {
	match paint {
		Graphic::Gradient(item) => item.attribute::<DAffine2>(ATTR_TRANSFORM).is_none(),
		Graphic::RasterCPU(item) => item.attribute::<DAffine2>(ATTR_TRANSFORM).is_none(),
		Graphic::RasterGPU(item) => item.attribute::<DAffine2>(ATTR_TRANSFORM).is_none(),
		_ => false,
	}
}

/// An effect's drawn paint with its unit square fitted to `content_bounds`, the extent it fills in the space it's drawn in, if it's a gradient or image without its own placement.
pub fn fitted_paint(paint: &Graphic, content_bounds: [DVec2; 2]) -> Option<Graphic> {
	if !is_unplaced_paint(paint) {
		return None;
	}

	let fit = DAffine2::from_translation(content_bounds[0]) * DAffine2::from_scale(content_bounds[1] - content_bounds[0]);
	let mut paint = paint.clone();
	match &mut paint {
		Graphic::Gradient(item) => item.set_attribute(ATTR_TRANSFORM, fit),
		Graphic::RasterCPU(item) => item.set_attribute(ATTR_TRANSFORM, fit),
		Graphic::RasterGPU(item) => item.set_attribute(ATTR_TRANSFORM, fit),
		_ => {}
	}
	Some(paint)
}

/// The `graphic` expressed in the own space of content placed by `placement`, for an effect with a drawn graphic that follows the content (see [`EffectOperation::drawn_paint_follows_content`]).
/// A gradient or image without its own placement stays as it is, since it stretches across the content wherever that lies.
pub fn linked_to_content(graphic: Graphic, placement: DAffine2) -> Graphic {
	if is_unplaced_paint(&graphic) || placement == DAffine2::IDENTITY || !placement.matrix2.determinant().is_normal() {
		return graphic;
	}
	Graphic::Graphic(Box::new(Item::new_from_element(graphic).with_attribute(ATTR_TRANSFORM, placement.inverse())))
}

/// For a masked radius blur's map that is an image of `size` pixels without its own placement, the transform placing its pixels across `content_bounds`, past which its edge pixels continue.
/// Past its edges a map is transparent and blurs nothing, so an image stretched across the content would otherwise cut its blur off at the content's edges.
pub fn continued_map_pixels(map: &Graphic, size: DVec2, content_bounds: [DVec2; 2]) -> Option<DAffine2> {
	let image = matches!(map, Graphic::RasterCPU(_) | Graphic::RasterGPU(_));
	let has_pixels = size.min_element() > 0.;
	if !image || !is_unplaced_paint(map) || !has_pixels {
		return None;
	}

	Some(DAffine2::from_translation(content_bounds[0]) * DAffine2::from_scale((content_bounds[1] - content_bounds[0]) / size))
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
		operation.visit_parameters(&mut ReadParameters(&|key| self.0.attributes().get_any(key)));
		let operation = operation.with_finite_lengths();

		// A solid paint mixes the same into every pixel, so the renderers carry it out with the other adjustments
		match operation {
			EffectOperation::PaintOverlay {
				paint: Graphic::Color(color),
				blend_mode,
				opacity,
			} => EffectOperation::Adjustment(Adjustment::ColorOverlay {
				color: *color.element(),
				blend_mode,
				opacity,
			}),
			operation => operation,
		}
	}
}

/// An item's ordered list of effects. A pixel filter (like a blur, adjustment, mask, or feather) acts on the result of those before it, shadows and glows included, while shadows and glows are cast from the content and stack in order beneath it (or within it, for an inner shadow).
#[derive(Clone, Debug, Default, PartialEq, CacheHash)]
pub struct Effects(pub List<Effect>);

impl Effects {
	/// Creates the list of rows carrying out the given operations in order, stamped on untransformed content.
	pub fn from_operations(operations: impl IntoIterator<Item = EffectOperation>) -> Self {
		let mut effects = Self::default();
		for operation in operations {
			effects.0.push(Item::new_from_element(Effect::new(&operation)).with_attribute(ATTR_TRANSFORM, DAffine2::IDENTITY));
		}
		effects
	}

	/// The effects in the order they're carried out.
	pub fn effects(&self) -> impl Iterator<Item = &Effect> {
		self.0.iter_element_values()
	}

	/// The effects after the first `count`.
	pub fn after(&self, count: usize) -> Self {
		self.run(count.min(self.len())..self.len()).into_owned()
	}

	/// The effects decoded into the operations renderers carry out, in order.
	pub fn operations(&self) -> impl Iterator<Item = EffectOperation> + '_ {
		self.effects().map(Effect::operation)
	}

	/// Whether carrying out the effects leaves the content as it is, like a blur of no radius, so renderers can skip them.
	pub fn change_nothing(&self) -> bool {
		self.operations().all(|operation| operation.changes_nothing())
	}

	/// The effects of the given range of rows decoded into operations, in order.
	pub fn operations_in(&self, rows: Range<usize>) -> impl Iterator<Item = EffectOperation> + '_ {
		self.effects().skip(rows.start).take(rows.len()).map(Effect::operation)
	}

	/// How far past the content the effects together can paint, in the effect space of a run of them.
	pub fn reach(&self) -> f64 {
		self.operations().map(|operation| operation.painted_reach()).sum()
	}

	/// How far past the content the effects together paint or read from, in the effect space of a run of them, which the region they're worked out over covers.
	pub fn working_reach(&self) -> f64 {
		self.operations().map(|operation| operation.working_reach()).sum()
	}

	/// Splits the rows into runs stamped in the same space, in order, each with the transform its effects were stamped at.
	/// A transform applied between two effects starts a new run, which renders around the result of those before it.
	/// Effects that change nothing join the run they're in, and a run of only those takes the space of the next effect.
	pub fn runs(&self) -> Vec<(DAffine2, Range<usize>)> {
		let mut runs: Vec<(DAffine2, Range<usize>)> = Vec::new();
		let mut run_changes_something = false;
		for (index, effect) in self.effects().enumerate() {
			let stamped_transform: DAffine2 = self.0.attribute_cloned_or_default(ATTR_TRANSFORM, index);
			let changes_something = !effect.operation().changes_nothing();

			if let Some((transform, rows)) = runs.last_mut()
				&& (!changes_something || !run_changes_something || same_linear_part(transform.matrix2, stamped_transform.matrix2))
			{
				if changes_something && !run_changes_something {
					*transform = stamped_transform;
				}
				rows.end = index + 1;
				run_changes_something |= changes_something;
			} else {
				runs.push((stamped_transform, index..index + 1));
				run_changes_something = changes_something;
			}
		}
		runs
	}

	/// Moves the transforms the effects were stamped at onto content whose `baked` transform went into its geometry, and the graphics that follow the content with them.
	pub fn rebase_stamped_transforms(&mut self, baked: DAffine2) {
		if let Some(transforms) = self.0.iter_attribute_values_mut::<DAffine2>(ATTR_TRANSFORM) {
			transforms.for_each(|transform| *transform = rebased_stamped_transform(*transform, baked));
		}

		// A graphic following the content (see `EffectOperation::drawn_paint_follows_content`) lies in its own space, which `baked` now places within the geometry
		if baked == DAffine2::IDENTITY || !baked.matrix2.determinant().is_normal() {
			return;
		}
		for effect in self.0.iter_element_values_mut() {
			let key = match effect.kind() {
				EffectKind::Mask => "mask",
				EffectKind::Blur => "radius_mask",
				_ => continue,
			};
			if let Some(graphic) = effect.0.attribute_mut::<Graphic>(key)
				&& !is_unplaced_paint(graphic)
			{
				*graphic = Graphic::Graphic(Box::new(Item::new_from_element(std::mem::take(graphic)).with_attribute(ATTR_TRANSFORM, baked)));
			}
		}
	}

	/// The mask and feather rows alone, with the transforms they were stamped at, or `None` without any. They shape what clips to the content as they shape the content itself, unlike the other effects.
	pub fn shaping_only(&self) -> Option<Effects> {
		let shapes = |index: &usize| self.0.element(*index).is_some_and(|effect| matches!(effect.kind(), EffectKind::Mask | EffectKind::Feather));
		let shaping: List<Effect> = (0..self.len()).filter(shapes).filter_map(|index| self.0.clone_item(index)).collect();
		(!shaping.is_empty()).then_some(Effects(shaping))
	}

	/// The effects of a run of rows, borrowed when the run is every row.
	pub fn run(&self, rows: Range<usize>) -> Cow<'_, Effects> {
		match rows == (0..self.len()) {
			true => Cow::Borrowed(self),
			false => Cow::Owned(Effects(rows.filter_map(|index| self.0.clone_item(index)).collect())),
		}
	}

	/// How far past the content the effects together can paint along each axis of the space `content_transform` places the content in.
	pub fn reach_along_axes(&self, content_transform: DAffine2) -> DVec2 {
		self.reach_along_axes_by(content_transform, EffectOperation::painted_reach)
	}

	/// Like [`Self::reach_along_axes`], measuring how far each effect reaches in its effect space by `reach`.
	pub fn reach_along_axes_by(&self, content_transform: DAffine2, reach: impl Fn(&EffectOperation) -> f64) -> DVec2 {
		(self.runs().into_iter())
			.map(|(stamped_transform, rows)| self.operations_in(rows).map(|operation| reach(&operation)).sum::<f64>() * spans_along_axes(effect_space(content_transform, stamped_transform).matrix2))
			.sum()
	}

	/// How far a unit of each effect's lengths spans along each axis of the space `content_transform` places the content in, in order.
	pub fn length_spans(&self, content_transform: DAffine2) -> Vec<DVec2> {
		(self.runs().into_iter())
			.flat_map(|(stamped_transform, rows)| std::iter::repeat_n(spans_along_axes(effect_space(content_transform, stamped_transform).matrix2), rows.len()))
			.collect()
	}

	/// How far past the content the effects reading around each pixel (see [`EffectOperation::reads_pixels_around`]) together read, along the axis of the space `content_transform` places the content in that they read furthest along.
	pub fn read_reach(&self, content_transform: DAffine2) -> f64 {
		let reach = |operation: &EffectOperation| if operation.reads_pixels_around() { operation.painted_reach() } else { 0. };
		self.reach_along_axes_by(content_transform, reach).max_element()
	}

	pub fn len(&self) -> usize {
		self.0.len()
	}

	pub fn is_empty(&self) -> bool {
		self.0.is_empty()
	}
}

/// The most that a length in one space spans along each axis of another, given the linear map between them.
pub fn spans_along_axes(linear_map: DMat2) -> DVec2 {
	DVec2::new(linear_map.row(0).length(), linear_map.row(1).length())
}

/// Whether two linear maps agree to within rounding, as two transforms meant to be the same do after a turn and its undoing.
fn same_linear_part(first: DMat2, second: DMat2) -> bool {
	let scale = first.x_axis.abs().max_element().max(first.y_axis.abs().max_element());
	first.abs_diff_eq(second, 1e-9 * scale)
}

/// For PSD interop, an outer glow filled with a gradient fades out once its strength before range and contour falls below this.
pub const GRADIENT_GLOW_FADE: f64 = 0.12;

/// How many evenly spaced samples of a glow's or outline's gradient the renderers look its colors up in, enough to follow it within a level.
pub const GRADIENT_TABLE_SIZE: usize = 256;

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
		// A gradient without stops reads as black
		let gradient = |item: &Item<Gradient>| Self::Gradient {
			evaluator: Box::new(item.element().evaluator(GradientSettings::from_item_attributes(item))),
			visible: item.element().is_empty() || item.element().0.iter_element_values().any(|stop| stop.a() > 0.),
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

/// How far an outline of `weight` reaches outside the silhouette's edge and inside it, by where it's aligned.
pub fn outline_extents(weight: f64, align: StrokeAlign) -> (f64, f64) {
	match align {
		StrokeAlign::Outside => (weight, 0.),
		StrokeAlign::Inside => (0., weight),
		StrokeAlign::Center => (weight / 2., weight / 2.),
	}
}

/// What fills an outline: a solid color tinting its coverage, a gradient run across its width, or a graphic the renderers draw over the same pixels.
pub enum OutlineFill<'a> {
	Solid(Color),
	/// A contour gradient, with its position following the distance across the outline's width rather than its placement (PSD's Shape Burst, for interop).
	ContourGradient(GlowFill),
	Drawn(&'a Graphic),
}

impl<'a> OutlineFill<'a> {
	/// The fill an outline's `paint` makes, or `None` for no paint. A gradient runs across the outline's width if `contour_gradient`, and any other paint ignores it.
	/// A list of several colors or gradients is drawn, which composites them all.
	pub fn of(paint: &'a Graphic, contour_gradient: bool) -> Option<Self> {
		let list_length = match paint {
			Graphic::None(_) | Graphic::NoneList(_) => return None,
			Graphic::ColorList(list) => list.len(),
			Graphic::GradientList(list) => list.len(),
			_ => 1,
		};
		if list_length == 0 {
			return None;
		}

		let single = list_length == 1;
		Some(match paint {
			Graphic::Color(_) | Graphic::ColorList(_) if single => Self::Solid(match GlowFill::of(paint)? {
				GlowFill::Solid(color) => color,
				GlowFill::Gradient { .. } => return None,
			}),
			Graphic::Gradient(_) | Graphic::GradientList(_) if single && contour_gradient => Self::ContourGradient(GlowFill::of(paint)?),
			_ => Self::Drawn(paint),
		})
	}

	/// Whether the fill paints nothing anywhere.
	pub fn is_invisible(&self) -> bool {
		match self {
			Self::Solid(color) => color.a() <= 0.,
			Self::ContourGradient(gradient) => gradient.is_invisible(),
			Self::Drawn(_) => false,
		}
	}
}

/// Where a contour gradient across an outline of `weight` starts, as a distance past the content's edge, and how far inward it spans.
/// A point `distance` past the edge (negative inside it) takes the position `(start - distance) / span`, clamped from 0 to 1 and rounded by [`contour_gradient_position`].
/// For PSD interop, it spans from half a unit outside the outline to half a unit past its inner edge, or a unit and a half for a centered outline.
pub fn contour_gradient_span(weight: f64, align: StrokeAlign) -> (f64, f64) {
	let (outside, _) = outline_extents(weight, align);
	let span = weight + if align == StrokeAlign::Center { 2. } else { 1. };
	(outside + 0.5, span)
}

/// The position from 0 to 1 along a contour gradient that a point `distance` past the content's edge takes, given the `start` and `span` of [`contour_gradient_span`].
/// It's rounded to the nearest of the gradient's [`GRADIENT_TABLE_SIZE`] samples, which for PSD interop are 8-bit levels, so every renderer reads the same sample even at a hard stop.
pub fn contour_gradient_position(distance: f32, start: f32, span: f32) -> f32 {
	let last = (GRADIENT_TABLE_SIZE - 1) as f32;
	(((start - distance) / span).clamp(0., 1.) * last + 0.5).floor() / last
}

/// Appends an effect to the item's list, after those already applied to it, recording the item's transform at that point.
/// Like a stroke's transform, it sets the space the effect's lengths and angles are measured in, so transforms applied later carry and scale the effect.
pub fn stamp_effect<T>(item: &mut Item<T>, effect: Effect) {
	let transform = stamped_transform(item.attribute_cloned_or_default(ATTR_TRANSFORM));
	item.attribute_mut_or_insert_default::<Effects>(ATTR_EFFECTS)
		.0
		.push(Item::new_from_element(effect).with_attribute(ATTR_TRANSFORM, transform));
}

/// The graphic a paint input's wire carries, without the attributes of its envelope but for the effects stamped on it, such as an adjustment applied to the paint as a graphic.
/// Colors and gradients (each row of a list of them) take those effects after their own, staying solid or gradient paints, no paint stays none, and other paints nest as a group carrying them.
pub fn paint_with_envelope_effects(paint: Item<Graphic>) -> Graphic {
	let effects = paint.attribute::<Effects>(ATTR_EFFECTS).filter(|effects| !effects.is_empty()).cloned();
	let paint = paint.into_element();
	let Some(effects) = effects else { return paint };

	let append_to_rows = |rows: std::slice::IterMut<'_, Effects>| rows.for_each(|row| row.0.extend(effects.0.clone()));
	match paint {
		Graphic::None(_) | Graphic::NoneList(_) => paint,
		Graphic::Color(mut item) => {
			item.attribute_mut_or_insert_default::<Effects>(ATTR_EFFECTS).0.extend(effects.0);
			Graphic::Color(item)
		}
		Graphic::Gradient(mut item) => {
			item.attribute_mut_or_insert_default::<Effects>(ATTR_EFFECTS).0.extend(effects.0);
			Graphic::Gradient(item)
		}
		Graphic::ColorList(mut list) => {
			append_to_rows(list.iter_attribute_values_mut_or_default::<Effects>(ATTR_EFFECTS));
			Graphic::ColorList(list)
		}
		Graphic::GradientList(mut list) => {
			append_to_rows(list.iter_attribute_values_mut_or_default::<Effects>(ATTR_EFFECTS));
			Graphic::GradientList(list)
		}
		paint => Graphic::Graphic(Box::new(Item::new_from_element(paint).with_attribute(ATTR_EFFECTS, effects))),
	}
}

/// The part of a content transform a coverage or effect records, its linear part, since the content's position cancels out wherever it's used.
pub fn stamped_transform(content_transform: DAffine2) -> DAffine2 {
	DAffine2::from_mat2(content_transform.matrix2)
}

/// The space effects stamped at `stamped_transform` are carried out in, placed by the content's current `content_transform`.
/// Its origin is the content's, and its lengths are as viewed when stamped, carried and scaled by any transform applied since.
pub fn effect_space(content_transform: DAffine2, stamped_transform: DAffine2) -> DAffine2 {
	let stamped_linear = stamped_transform.matrix2;

	// Content left as it was stamped measures exactly as viewed, which the product with the inverse only approximates
	let since_stamped = if content_transform.matrix2 == stamped_linear || !stamped_linear.determinant().is_normal() {
		DMat2::IDENTITY
	} else {
		content_transform.matrix2 * stamped_linear.inverse()
	};
	DAffine2::from_mat2_translation(since_stamped, content_transform.translation)
}

/// The transform a coverage or effect was stamped at, moved onto content whose `baked` transform went into its geometry, so it keeps measuring in the same space.
pub fn rebased_stamped_transform(stamped_transform: DAffine2, baked: DAffine2) -> DAffine2 {
	match baked.matrix2.determinant().is_normal() {
		true => DAffine2::from_mat2(stamped_transform.matrix2 * baked.matrix2.inverse()),
		false => stamped_transform,
	}
}

/// An effect's parameters decoded from its row, the form that renderers carry it out from. Lengths are in its [`effect_space`].
#[derive(Clone, Debug, PartialEq)]
pub enum EffectOperation {
	/// Blurs the content after what lies beneath it joins it, evenly or by the interpolation between blurs [`crate::masks::masked_radius_blur_interpolation`] defines.
	Blur {
		/// Lies in the content's own space, so it moves with the content, with its luminance setting each point's fraction of the radius.
		/// One that draws nothing, the default, blurs the whole content by the radius.
		radius_mask: Graphic,
		/// The standard deviation of the Gaussian, or the half width of the box, where the radius mask is white.
		radius: f64,
		/// Whether to average a box of pixels rather than weighting them by a Gaussian.
		box_blur: bool,
		/// Whether to blend the colors with their gamma encoding rather than in linear light.
		gamma: bool,
	},
	DropShadow {
		/// With the shadow's opacity.
		color: Color,
		/// How the shadow mixes with what lies beneath the content.
		blend_mode: BlendMode,
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
		/// How the shadow mixes with the content and what shows through it.
		blend_mode: BlendMode,
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
		/// How the glow mixes with what lies beneath the content.
		blend_mode: BlendMode,
		technique: GlowTechnique,
		spread: f64,
		size: f64,
		/// The percentage of the glow's strength it reaches full opacity at.
		range: f64,
		contour: TransferCurve,
	},
	InnerGlow {
		/// A solid color with the glow's opacity, or a gradient running from the content's edge inward (or from its center outward).
		color: Graphic,
		/// How the glow mixes with the content and what shows through it.
		blend_mode: BlendMode,
		technique: GlowTechnique,
		source: GlowSource,
		/// The percentage of the size that hardens into a grow inward from the edges rather than softening.
		choke: f64,
		size: f64,
		range: f64,
		contour: TransferCurve,
	},
	Satin {
		color: Color,
		/// How the shading mixes with the content and what shows through it.
		blend_mode: BlendMode,
		/// The direction each of the two casts of the silhouette is moved along, one each way, in degrees counterclockwise from the right.
		angle: f64,
		distance: f64,
		size: f64,
		invert: bool,
		contour: TransferCurve,
	},
	Outline {
		/// A solid color with the outline's opacity, a gradient, or any graphic, fitted when unplaced to the content and the outline's margin.
		paint: Graphic,
		/// How the outline mixes with what lies beneath it, or with the content for the part inside its edge.
		blend_mode: BlendMode,
		weight: f64,
		align: StrokeAlign,
		/// Whether a gradient paint runs across the outline's width rather than by its own placement.
		contour_gradient: bool,
	},
	BevelAndEmboss {
		style: BevelStyle,
		technique: BevelTechnique,
		/// The percentage that scales how steep the slopes are.
		depth: f64,
		direction: BevelDirection,
		/// How wide the slopes are.
		/// For PSD interop, an importer maps a Smooth inner bevel's Size 0 (a one-pixel step at any depth) to Size 0.5 at Depth 200%.
		size: f64,
		/// How much the highlights and shadows are blurred.
		soften: f64,
		/// The direction the light comes from, in degrees counterclockwise from the right.
		angle: f64,
		/// How high the light is above the surface, in degrees from flat to straight overhead.
		altitude: f64,
		/// With the highlight's opacity.
		highlight_color: Color,
		/// How the highlight mixes with the colors beneath it.
		highlight_blend_mode: BlendMode,
		/// With the shadow's opacity.
		shadow_color: Color,
		/// How the shadow mixes with the colors beneath it.
		shadow_blend_mode: BlendMode,
		/// A graphic with lightness that sinks the surface, or none.
		texture: Graphic,
		/// The percentage of the slopes' width that the texture's lightest parts sink by, raising them when negative.
		texture_depth: f64,
		/// The percentage of the slopes the slope contour spans.
		slope_range: f64,
		/// Whether the gloss contour is sampled across each pixel, smoothing its steep parts.
		gloss_antialiased: bool,
		/// Whether the slope contour is sampled across each pixel, smoothing its steep parts.
		slope_antialiased: bool,
		/// Reshapes how lit the slopes are, from facing away from the light to facing it.
		gloss_contour: TransferCurve,
		/// Reshapes the slopes' profile.
		slope_contour: TransferCurve,
	},
	/// Paints over the content with a paint other than a solid color, since a solid one decodes into the color overlay adjustment instead.
	PaintOverlay {
		/// Placed in the space the content's item is, or fitted to the content's bounds if it's a gradient or image without its own placement.
		paint: Graphic,
		blend_mode: BlendMode,
		/// Percent.
		opacity: f64,
	},
	/// Multiplies the content by how much the mask graphic shows it, after what lies beneath the content joins it.
	Mask {
		/// Lies in the content's own space, so it moves with the content.
		mask: Graphic,
		mode: MaskMode,
		/// The percentage of how much the mask hides that it keeps.
		density: f64,
		/// How much the mask is softened, which [`mask_feather_sigma`] makes a Gaussian's standard deviation.
		feather: f64,
		invert: bool,
	},
	/// Multiplies the content by the strength [`crate::masks::feather_parts`] defines, after what lies beneath the content joins it.
	Feather {
		/// How far in from the edges the fade reaches full strength.
		distance: f64,
	},
	Adjustment(Adjustment),
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
			Self::InnerGlow { .. } => EffectKind::InnerGlow,
			Self::Satin { .. } => EffectKind::Satin,
			Self::Outline { .. } => EffectKind::Outline,
			Self::BevelAndEmboss { .. } => EffectKind::BevelAndEmboss,
			Self::PaintOverlay { .. } => EffectKind::PaintOverlay,
			Self::Mask { .. } => EffectKind::Mask,
			Self::Feather { .. } => EffectKind::Feather,
			Self::Adjustment(adjustment) => adjustment.kind(),
		}
	}

	/// Whether this operation leaves the content as it is, like a blur of no radius or a shadow of no opacity.
	pub fn changes_nothing(&self) -> bool {
		match self {
			Self::Blur { radius, .. } => *radius <= 0.,
			Self::DropShadow { color, .. } | Self::InnerShadow { color, .. } | Self::Satin { color, .. } => color.a() <= 0.,
			Self::OuterGlow { color, .. } | Self::InnerGlow { color, .. } => GlowFill::of(color).is_none_or(|fill| fill.is_invisible()),
			Self::Outline { paint, weight, contour_gradient, .. } => *weight <= 0. || OutlineFill::of(paint, *contour_gradient).is_none_or(|fill| fill.is_invisible()),
			// A gloss contour shades even flat ground, so a bevel without slopes still draws it
			Self::BevelAndEmboss {
				highlight_color,
				shadow_color,
				size,
				gloss_contour,
				..
			} => (highlight_color.a() <= 0. && shadow_color.a() <= 0.) || (*size <= 0. && gloss_contour.is_identity()),
			Self::PaintOverlay { opacity, .. } => *opacity <= 0.,
			Self::Adjustment(Adjustment::ColorOverlay { color, opacity, .. }) => color.a() <= 0. || *opacity <= 0.,
			Self::Mask { density, .. } => *density <= 0.,
			Self::Feather { distance } => *distance <= 0.,
			Self::Adjustment(_) => false,
		}
	}

	/// Whether this is a blur varying its radius by a radius mask that draws something, rather than blurring the whole content evenly.
	pub fn varies_blur_radius(&self) -> bool {
		matches!(self, Self::Blur { radius_mask, .. } if !radius_mask.draws_nothing())
	}

	/// Whether this operation reads the pixels around each one of the image the effects before it leave, like a blur, rather than casting from the content's shape or changing each pixel alone.
	pub fn reads_pixels_around(&self) -> bool {
		match self {
			Self::Blur { .. } => true,
			// Cast from the content's shape, reading a mask's own image or a feather's map, or changing each pixel alone
			Self::DropShadow { .. }
			| Self::InnerShadow { .. }
			| Self::OuterGlow { .. }
			| Self::InnerGlow { .. }
			| Self::Satin { .. }
			| Self::Outline { .. }
			| Self::BevelAndEmboss { .. }
			| Self::Feather { .. } => false,
			Self::PaintOverlay { .. } | Self::Mask { .. } | Self::Adjustment(_) => false,
		}
	}

	/// How far past the content this operation paints, which is nowhere for an operation that changes nothing, a mask, or a feather.
	pub fn painted_reach(&self) -> f64 {
		if self.changes_nothing() || matches!(self, Self::Mask { .. } | Self::Feather { .. }) {
			0.
		} else {
			self.reach()
		}
	}

	/// How far past the content this operation paints or reads from, which the region it's worked out over covers, and nowhere for an operation that changes nothing.
	pub fn working_reach(&self) -> f64 {
		if self.changes_nothing() { 0. } else { self.reach() }
	}

	/// How far past the content this operation can paint, or reads from, in its effect space.
	pub fn reach(&self) -> f64 {
		match self {
			// A Gaussian is negligible past three standard deviations
			Self::Blur { radius, .. } => 3. * radius.max(0.),
			// An inner shadow paints within the content but reads this far out to find the edges it casts from.
			// Shadows and glows end within their size, past which a precise glow fades over one more unit.
			Self::DropShadow { distance, size, .. } | Self::InnerShadow { distance, size, .. } => distance.abs() + size.max(0.) + 1.,
			Self::OuterGlow { size, .. } => size.max(0.) + 1.,
			Self::InnerGlow { size, .. } => size.max(0.) + 1.,
			Self::Satin { distance, size, .. } => distance.abs() + size.max(0.) + 1.,
			Self::Outline { weight, .. } => weight.max(0.) + 1.,
			Self::BevelAndEmboss { size, soften, .. } => size.max(0.) + soften.max(0.) + 2.,
			// A mask paints nothing past the content, but its feather reads the mask this far past it
			Self::Mask { feather, .. } => 3. * mask_feather_sigma(*feather),
			// A feather paints nothing past the content, but finds edges up to this far past it, which a render cut short there would miss
			Self::Feather { distance } => distance.max(0.) + 1.,
			Self::PaintOverlay { .. } | Self::Adjustment(_) => 0.,
		}
	}

	/// The graphic this operation paints with that renderers draw over the content's pixels on their own: a paint overlay's paint, an outline's drawn paint, a bevel's texture, a mask's graphic, or a blur's radius mask.
	/// An operation that changes nothing draws none, since renderers skip it.
	pub fn drawn_paint(&self) -> Option<&Graphic> {
		if self.changes_nothing() {
			return None;
		}

		match self {
			Self::PaintOverlay { paint, .. } => Some(paint),
			Self::Outline { paint, contour_gradient, .. } => match OutlineFill::of(paint, *contour_gradient)? {
				OutlineFill::Drawn(paint) => Some(paint),
				_ => None,
			},
			Self::BevelAndEmboss { texture, .. } => (!matches!(texture, Graphic::None(_) | Graphic::NoneList(_))).then_some(texture),
			Self::Mask { mask, .. } => Some(mask),
			Self::Blur { radius_mask, .. } => self.varies_blur_radius().then_some(radius_mask),
			_ => None,
		}
	}

	/// Whether the graphic this operation draws with is placed in the content's own space, so it moves with the content, rather than in the space the content's item is placed in.
	/// The nodes stamping such operations express the graphic relative to the content's placement at that time with [`linked_to_content`].
	pub fn drawn_paint_follows_content(&self) -> bool {
		matches!(self, Self::Mask { .. } | Self::Blur { .. })
	}

	/// The bounds a drawn paint without its own placement is fitted to, given the content's bounds and the length a document pixel spans along each of their axes.
	/// For PSD interop, an outline's paint fits the content grown by the whole pixels the outline reaches outside it, at least one.
	pub fn drawn_paint_bounds(&self, content_bounds: [DVec2; 2], pixel_spans: DVec2) -> [DVec2; 2] {
		match self {
			Self::Outline { weight, align, .. } => {
				// Interpolated between the whole weights PSD stores
				let margin_at = |weight: f64| outline_extents(weight, *align).0.ceil().max(1.);
				let weight = weight.max(0.);
				let below = weight.floor();
				let margin = margin_at(below) + (margin_at(below + 1.) - margin_at(below)) * (weight - below);

				[content_bounds[0] - margin * pixel_spans, content_bounds[1] + margin * pixel_spans]
			}
			_ => content_bounds,
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
			Self::InnerGlow { choke, size, range, .. } => [choke, size, range].into_iter().for_each(finite),
			Self::Satin { angle, distance, size, .. } => [angle, distance, size].into_iter().for_each(finite),
			Self::Outline { weight, .. } => finite(weight),
			Self::BevelAndEmboss {
				depth,
				size,
				soften,
				angle,
				altitude,
				slope_range,
				texture_depth,
				..
			} => [depth, size, soften, angle, altitude, slope_range, texture_depth].into_iter().for_each(finite),
			Self::Mask { density, feather, .. } => [density, feather].into_iter().for_each(finite),
			Self::Feather { distance } => finite(distance),
			Self::PaintOverlay { .. } | Self::Adjustment(_) => {}
		}
		self
	}

	/// An operation of this kind with arbitrary parameters, for a visitor to fill in.
	fn placeholder(kind: EffectKind) -> Self {
		match kind {
			EffectKind::Blur => Self::Blur {
				radius_mask: Graphic::default(),
				radius: 0.,
				box_blur: false,
				gamma: false,
			},
			EffectKind::DropShadow => Self::DropShadow {
				color: Color::TRANSPARENT,
				blend_mode: BlendMode::default(),
				angle: 0.,
				distance: 0.,
				spread: 0.,
				size: 0.,
				contour: TransferCurve::default(),
				knockout: false,
			},
			EffectKind::InnerShadow => Self::InnerShadow {
				color: Color::TRANSPARENT,
				blend_mode: BlendMode::default(),
				angle: 0.,
				distance: 0.,
				choke: 0.,
				size: 0.,
				contour: TransferCurve::default(),
			},
			EffectKind::OuterGlow => Self::OuterGlow {
				color: Graphic::default(),
				blend_mode: BlendMode::default(),
				technique: GlowTechnique::default(),
				spread: 0.,
				size: 0.,
				range: 0.,
				contour: TransferCurve::default(),
			},
			EffectKind::InnerGlow => Self::InnerGlow {
				color: Graphic::default(),
				blend_mode: BlendMode::default(),
				technique: GlowTechnique::default(),
				source: GlowSource::default(),
				choke: 0.,
				size: 0.,
				range: 0.,
				contour: TransferCurve::default(),
			},
			EffectKind::Satin => Self::Satin {
				color: Color::TRANSPARENT,
				blend_mode: BlendMode::default(),
				angle: 0.,
				distance: 0.,
				size: 0.,
				invert: false,
				contour: TransferCurve::default(),
			},
			EffectKind::Outline => Self::Outline {
				paint: Graphic::default(),
				blend_mode: BlendMode::default(),
				weight: 0.,
				align: StrokeAlign::default(),
				contour_gradient: false,
			},
			EffectKind::BevelAndEmboss => Self::BevelAndEmboss {
				style: BevelStyle::default(),
				technique: BevelTechnique::default(),
				depth: 0.,
				direction: BevelDirection::default(),
				size: 0.,
				soften: 0.,
				angle: 0.,
				altitude: 0.,
				highlight_color: Color::TRANSPARENT,
				highlight_blend_mode: BlendMode::default(),
				shadow_color: Color::TRANSPARENT,
				shadow_blend_mode: BlendMode::default(),
				texture: Graphic::default(),
				texture_depth: 0.,
				slope_range: 0.,
				gloss_antialiased: false,
				slope_antialiased: false,
				gloss_contour: TransferCurve::default(),
				slope_contour: TransferCurve::default(),
			},
			EffectKind::PaintOverlay => Self::PaintOverlay {
				paint: Graphic::default(),
				blend_mode: BlendMode::default(),
				opacity: 0.,
			},
			EffectKind::Mask => Self::Mask {
				mask: Graphic::default(),
				mode: MaskMode::default(),
				density: 0.,
				feather: 0.,
				invert: false,
			},
			EffectKind::Feather => Self::Feather { distance: 0. },
			EffectKind::Desaturate => Self::Adjustment(Adjustment::Desaturate { method: Default::default() }),
			EffectKind::GammaCorrection => Self::Adjustment(Adjustment::GammaCorrection { gamma: 0., inverse: false }),
			EffectKind::MakeOpaque => Self::Adjustment(Adjustment::MakeOpaque),
			EffectKind::BrightnessContrast => Self::Adjustment(Adjustment::BrightnessContrast {
				brightness: 0.,
				contrast: 0.,
				use_classic: false,
				classic_pivot: 0.,
			}),
			EffectKind::Levels => Self::Adjustment(Adjustment::Levels(Default::default())),
			EffectKind::Curves => Self::Adjustment(Adjustment::Curves(Default::default())),
			EffectKind::BlackAndWhite => Self::Adjustment(Adjustment::BlackAndWhite(Default::default())),
			EffectKind::HueSaturation => Self::Adjustment(Adjustment::HueSaturation(Default::default())),
			EffectKind::Invert => Self::Adjustment(Adjustment::Invert),
			EffectKind::Threshold => Self::Adjustment(Adjustment::Threshold { min_luminance: 0., max_luminance: 0. }),
			EffectKind::GradientMap => Self::Adjustment(Adjustment::GradientMap {
				gradient: Default::default(),
				reverse: false,
			}),
			EffectKind::Vibrance => Self::Adjustment(Adjustment::Vibrance { vibrance: 0., saturation: 0. }),
			EffectKind::ChannelMixer => Self::Adjustment(Adjustment::ChannelMixer(Default::default())),
			EffectKind::SelectiveColor => Self::Adjustment(Adjustment::SelectiveColor(Default::default())),
			EffectKind::Posterize => Self::Adjustment(Adjustment::Posterize { levels: 0 }),
			EffectKind::Exposure => Self::Adjustment(Adjustment::Exposure {
				exposure: 0.,
				offset: 0.,
				gamma_correction: 0.,
			}),
			EffectKind::ColorBalance => Self::Adjustment(Adjustment::ColorBalance(Default::default())),
			EffectKind::PhotoFilter => Self::Adjustment(Adjustment::PhotoFilter {
				color: Color::TRANSPARENT,
				density: 0.,
				preserve_luminosity: false,
			}),
			EffectKind::ColorLookup => Self::Adjustment(Adjustment::ColorLookup { lut_file: Default::default() }),
		}
	}

	/// Visits each parameter with the attribute it rides and its implicit default, the one listing that writing and reading rows share.
	/// For PSD interop, the defaults are those of layer styles, so imported ones that leave a setting out read the same.
	fn visit_parameters(&mut self, visitor: &mut impl ParameterVisitor) {
		let shadow_color = Color::from(SRGBA8::new(0, 0, 0, 0xbf));
		let glow_color = Color::from(SRGBA8::new(0xff, 0xff, 0xbe, 0xbf));

		match self {
			Self::Blur { radius_mask, radius, box_blur, gamma } => {
				visitor.parameter("radius", radius, 0.);
				visitor.parameter("radius_mask", radius_mask, Graphic::default());
				visitor.parameter("box_blur", box_blur, false);
				visitor.parameter("gamma", gamma, false);
			}
			Self::DropShadow {
				color,
				blend_mode,
				angle,
				distance,
				spread,
				size,
				contour,
				knockout,
			} => {
				visitor.parameter("color", color, shadow_color);
				visitor.parameter("blend_mode", blend_mode, BlendMode::Multiply);
				visitor.parameter("angle", angle, 120.);
				visitor.parameter("distance", distance, 5.);
				visitor.parameter("spread", spread, 0.);
				visitor.parameter("size", size, 5.);
				visitor.parameter("contour", contour, TransferCurve::default());
				visitor.parameter("knockout", knockout, true);
			}
			Self::InnerShadow {
				color,
				blend_mode,
				angle,
				distance,
				choke,
				size,
				contour,
			} => {
				visitor.parameter("color", color, shadow_color);
				visitor.parameter("blend_mode", blend_mode, BlendMode::Multiply);
				visitor.parameter("angle", angle, 120.);
				visitor.parameter("distance", distance, 5.);
				visitor.parameter("choke", choke, 0.);
				visitor.parameter("size", size, 5.);
				visitor.parameter("contour", contour, TransferCurve::default());
			}
			Self::OuterGlow {
				color,
				blend_mode,
				technique,
				spread,
				size,
				range,
				contour,
			} => {
				visitor.parameter("color", color, Graphic::from(glow_color));
				visitor.parameter("blend_mode", blend_mode, BlendMode::Screen);
				visitor.parameter("technique", technique, GlowTechnique::default());
				visitor.parameter("spread", spread, 0.);
				visitor.parameter("size", size, 5.);
				visitor.parameter("range", range, 50.);
				visitor.parameter("contour", contour, TransferCurve::default());
			}
			Self::InnerGlow {
				color,
				blend_mode,
				technique,
				source,
				choke,
				size,
				range,
				contour,
			} => {
				visitor.parameter("color", color, Graphic::from(glow_color));
				visitor.parameter("blend_mode", blend_mode, BlendMode::Screen);
				visitor.parameter("technique", technique, GlowTechnique::default());
				visitor.parameter("source", source, GlowSource::default());
				visitor.parameter("choke", choke, 0.);
				visitor.parameter("size", size, 5.);
				visitor.parameter("range", range, 50.);
				visitor.parameter("contour", contour, TransferCurve::default());
			}
			Self::Satin {
				color,
				blend_mode,
				angle,
				distance,
				size,
				invert,
				contour,
			} => {
				visitor.parameter("color", color, Color::from(SRGBA8::new(0, 0, 0, 0x80)));
				visitor.parameter("blend_mode", blend_mode, BlendMode::Multiply);
				visitor.parameter("angle", angle, 19.);
				visitor.parameter("distance", distance, 11.);
				visitor.parameter("size", size, 14.);
				visitor.parameter("invert", invert, true);
				visitor.parameter("contour", contour, TransferCurve::default());
			}
			Self::Outline {
				paint,
				blend_mode,
				weight,
				align,
				contour_gradient,
			} => {
				visitor.parameter("paint", paint, Graphic::from(Color::BLACK));
				visitor.parameter("blend_mode", blend_mode, BlendMode::Normal);
				visitor.parameter("weight", weight, 3.);
				visitor.parameter("align", align, StrokeAlign::Outside);
				visitor.parameter("contour_gradient", contour_gradient, false);
			}
			Self::BevelAndEmboss {
				style,
				technique,
				depth,
				direction,
				size,
				soften,
				angle,
				altitude,
				highlight_color,
				highlight_blend_mode,
				shadow_color,
				shadow_blend_mode,
				texture,
				texture_depth,
				slope_range,
				gloss_antialiased,
				slope_antialiased,
				gloss_contour,
				slope_contour,
			} => {
				visitor.parameter("style", style, BevelStyle::default());
				visitor.parameter("technique", technique, BevelTechnique::default());
				visitor.parameter("depth", depth, 100.);
				visitor.parameter("direction", direction, BevelDirection::default());
				visitor.parameter("size", size, 5.);
				visitor.parameter("soften", soften, 0.);
				visitor.parameter("angle", angle, 120.);
				visitor.parameter("altitude", altitude, 30.);
				visitor.parameter("highlight_color", highlight_color, Color::from(SRGBA8::new(0xff, 0xff, 0xff, 0xbf)));
				visitor.parameter("highlight_blend_mode", highlight_blend_mode, BlendMode::Screen);
				visitor.parameter("shadow_color", shadow_color, Color::from(SRGBA8::new(0, 0, 0, 0xbf)));
				visitor.parameter("shadow_blend_mode", shadow_blend_mode, BlendMode::Multiply);
				visitor.parameter("texture", texture, Graphic::default());
				visitor.parameter("texture_depth", texture_depth, 100.);
				visitor.parameter("slope_range", slope_range, 100.);
				visitor.parameter("gloss_antialiased", gloss_antialiased, false);
				visitor.parameter("slope_antialiased", slope_antialiased, false);
				visitor.parameter("gloss_contour", gloss_contour, TransferCurve::default());
				visitor.parameter("slope_contour", slope_contour, TransferCurve::default());
			}
			Self::PaintOverlay { paint, blend_mode, opacity } => {
				visitor.parameter("paint", paint, Graphic::from(Color::BLACK));
				visitor.parameter("blend_mode", blend_mode, BlendMode::default());
				visitor.parameter("opacity", opacity, 100.);
			}
			Self::Mask { mask, mode, density, feather, invert } => {
				visitor.parameter("mask", mask, Graphic::default());
				visitor.parameter("mode", mode, MaskMode::default());
				visitor.parameter("density", density, 100.);
				visitor.parameter("feather", feather, 0.);
				visitor.parameter("invert", invert, false);
			}
			Self::Feather { distance } => visitor.parameter("distance", distance, 10.),
			Self::Adjustment(adjustment) => adjustment.visit_parameters(visitor),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use vector_types::{Gradient, GradientStop};

	#[test]
	fn rows_round_trip_their_operations_eliding_defaults() {
		let operations = [
			EffectOperation::Blur {
				radius_mask: Graphic::default(),
				radius: 6.,
				box_blur: true,
				gamma: false,
			},
			EffectOperation::default_for(EffectKind::DropShadow),
			EffectOperation::InnerShadow {
				color: Color::RED,
				blend_mode: BlendMode::Normal,
				angle: 30.,
				distance: 3.,
				choke: 25.,
				size: 6.,
				contour: TransferCurve::new(vec![DVec2::ZERO, DVec2::new(0.5, 1.), DVec2::ONE]),
			},
			EffectOperation::OuterGlow {
				color: Graphic::from(Color::BLUE),
				blend_mode: BlendMode::Lighten,
				technique: GlowTechnique::Precise,
				spread: 10.,
				size: 8.,
				range: 75.,
				contour: TransferCurve::default(),
			},
			EffectOperation::InnerGlow {
				color: Graphic::from(Color::GREEN),
				blend_mode: BlendMode::Screen,
				technique: GlowTechnique::Precise,
				source: GlowSource::Center,
				choke: 20.,
				size: 3.,
				range: 50.,
				contour: TransferCurve::default(),
			},
			EffectOperation::Satin {
				color: Color::BLACK,
				blend_mode: BlendMode::ColorBurn,
				angle: 45.,
				distance: 6.,
				size: 2.,
				invert: false,
				contour: TransferCurve::new(vec![DVec2::ZERO, DVec2::new(0.5, 1.), DVec2::ONE]),
			},
			EffectOperation::Outline {
				paint: Graphic::from(Color::RED),
				blend_mode: BlendMode::Difference,
				weight: 7.,
				align: StrokeAlign::Center,
				contour_gradient: false,
			},
			EffectOperation::Outline {
				paint: Graphic::from(Gradient::new([0., 1.].map(|position| GradientStop {
					position,
					midpoint: 0.5,
					color: Color::from_rgbaf32_unchecked(1., position as f32, 0., 1.),
				}))),
				blend_mode: BlendMode::Multiply,
				weight: 4.,
				align: StrokeAlign::Inside,
				contour_gradient: true,
			},
			EffectOperation::BevelAndEmboss {
				style: BevelStyle::PillowEmboss,
				technique: BevelTechnique::ChiselHard,
				depth: 250.,
				direction: BevelDirection::Down,
				size: 12.,
				soften: 2.,
				angle: -45.,
				altitude: 60.,
				highlight_color: Color::YELLOW,
				highlight_blend_mode: BlendMode::Overlay,
				shadow_color: Color::BLUE,
				shadow_blend_mode: BlendMode::Normal,
				gloss_contour: TransferCurve::new(vec![DVec2::ZERO, DVec2::new(0.5, 1.), DVec2::X]),
				gloss_antialiased: true,
				slope_contour: TransferCurve::new(vec![DVec2::ZERO, DVec2::new(0.5, 0.25), DVec2::ONE]),
				slope_antialiased: true,
				slope_range: 50.,
				texture: Graphic::from(Gradient::default()),
				texture_depth: -250.,
			},
			EffectOperation::PaintOverlay {
				paint: Graphic::from(Gradient::new([0., 1.].map(|position| GradientStop {
					position,
					midpoint: 0.5,
					color: Color::from_rgbaf32_unchecked(position as f32, 0., 1., 1.),
				}))),
				blend_mode: BlendMode::Multiply,
				opacity: 60.,
			},
			EffectOperation::Mask {
				mask: Graphic::from(Color::WHITE),
				mode: MaskMode::Alpha,
				density: 40.,
				feather: 3.5,
				invert: true,
			},
			EffectOperation::Feather { distance: 4.5 },
			EffectOperation::Blur {
				radius_mask: Graphic::from(Color::WHITE),
				radius: 7.5,
				box_blur: true,
				gamma: true,
			},
			EffectOperation::Adjustment(Adjustment::ColorOverlay {
				color: Color::GREEN,
				blend_mode: BlendMode::Screen,
				opacity: 100.,
			}),
		];

		for operation in operations {
			assert_eq!(Effect::new(&operation).operation(), operation);
		}

		// A solid paint decodes into the color overlay adjustment, which the Paint Overlay row also carries
		let solid = EffectOperation::PaintOverlay {
			paint: Graphic::from(Color::RED),
			blend_mode: BlendMode::Normal,
			opacity: 50.,
		};
		let solid_row = Effect::new(&solid);
		assert_eq!(solid_row.kind(), EffectKind::PaintOverlay);
		assert_eq!(
			solid_row.operation(),
			EffectOperation::Adjustment(Adjustment::ColorOverlay {
				color: Color::RED,
				blend_mode: BlendMode::Normal,
				opacity: 50.,
			})
		);

		let default_row = Effect::new(&EffectOperation::default_for(EffectKind::DropShadow));
		assert_eq!(default_row.0.attributes().iter_any().count(), 0, "a default operation stamps no parameters");
	}

	#[test]
	fn a_blur_row_without_a_radius_mask_decodes_as_an_even_blur() {
		let blur = |radius_mask: Graphic| EffectOperation::Blur {
			radius_mask,
			radius: 5.,
			box_blur: false,
			gamma: false,
		};

		// An empty radius mask is elided from the row, which reads back as empty
		let row = Effect::new(&blur(Graphic::default()));
		assert!(row.0.attribute::<Graphic>("radius_mask").is_none());
		assert_eq!(row.operation(), blur(Graphic::default()));

		// So the row decodes the same as one carrying an empty radius mask, or one that draws nothing, all blurring evenly with nothing drawn
		let mut carrying_empty = row.clone();
		carrying_empty.0.set_attribute("radius_mask", Graphic::default());
		let mut carrying_nothing = row.clone();
		carrying_nothing.0.set_attribute("radius_mask", Graphic::GraphicList(List::new()));
		for row in [row, carrying_empty, carrying_nothing] {
			let operation = row.operation();
			assert!(!operation.varies_blur_radius() && operation.drawn_paint().is_none(), "{operation:?}");
		}

		// A radius mask drawing something varies the blur and draws as its paint
		let masked = blur(Graphic::from(Color::WHITE));
		assert!(Effect::new(&masked).0.attribute::<Graphic>("radius_mask").is_some());
		assert!(masked.varies_blur_radius() && masked.drawn_paint() == Some(&Graphic::from(Color::WHITE)));
	}

	#[test]
	fn lengths_that_arent_finite_decode_as_zero() {
		let row = Effect::new(&EffectOperation::DropShadow {
			color: Color::BLACK,
			blend_mode: BlendMode::Multiply,
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
	fn outline_paints_fit_the_whole_pixels_the_outline_reaches_outside() {
		let margin = |weight, align| {
			let outline = EffectOperation::Outline {
				paint: Graphic::from(Gradient::default()),
				blend_mode: BlendMode::Normal,
				weight,
				align,
				contour_gradient: false,
			};
			outline.drawn_paint_bounds([DVec2::ZERO, DVec2::splat(10.)], DVec2::ONE)[1].x - 10.
		};

		assert_eq!(margin(8., StrokeAlign::Outside), 8.);
		// A centered outline of odd weight reaches into one more pixel, and one inside still fits a pixel past the content
		assert_eq!(margin(7., StrokeAlign::Center), 4.);
		assert_eq!(margin(40., StrokeAlign::Center), 20.);
		assert_eq!(margin(8., StrokeAlign::Inside), 1.);
		// Between whole weights, the margin is interpolated
		assert_eq!(margin(6.5, StrokeAlign::Center), 3.5);

		// In a space a document pixel spans unevenly, the margin is measured along each axis
		let outline = EffectOperation::Outline {
			paint: Graphic::from(Gradient::default()),
			blend_mode: BlendMode::Normal,
			weight: 4.,
			align: StrokeAlign::Outside,
			contour_gradient: false,
		};
		assert_eq!(
			outline.drawn_paint_bounds([DVec2::ZERO, DVec2::splat(10.)], DVec2::new(2., 0.5)),
			[DVec2::new(-8., -2.), DVec2::new(18., 12.)]
		);
	}

	#[test]
	fn bevel_contours_continue_straight_past_their_ends() {
		// Inner and outer bevels map the range down from the top and up from the foot, while an emboss maps its whole slope
		assert_eq!(slope_contour_input(0.75, BevelStyle::InnerBevel, 50.), 0.5);
		assert_eq!(slope_contour_input(0.25, BevelStyle::OuterBevel, 50.), 0.5);
		assert_eq!(slope_contour_input(0.3, BevelStyle::Emboss, 25.), 0.3);

		// A curve dipping below 0 near its start is held flat there, and so continues flat before it
		let dipping = TransferCurve::new(vec![DVec2::ZERO, DVec2::new(0.75, 0.), DVec2::ONE]).evaluator();
		let slopes = contour_end_slopes(&dipping);
		assert_eq!(slopes.0, 0.);
		assert_eq!(extended_contour(&dipping, slopes, -0.5), 0.);

		// A straight contour continues along its slope either way, and meets its ends there
		let straight = TransferCurve::default().evaluator();
		let slopes = contour_end_slopes(&straight);
		for x in [-0.5, 0., 1., 1.5] {
			assert!((extended_contour(&straight, slopes, x) - x).abs() < 1e-6, "the straight contour at {x}");
		}
	}

	#[test]
	fn contour_gradient_positions_land_on_the_gradients_samples() {
		// An outline of 3 aligned outside spans from 3.5 past the edge to 0.5 inside it, so its middle pixel lies halfway
		let (start, span) = contour_gradient_span(3., StrokeAlign::Outside);
		assert_eq!((start, span), (3.5, 4.));
		assert_eq!(contour_gradient_position(1.5, start as f32, span as f32), 128. / 255.);
		assert_eq!(contour_gradient_position(-5., start as f32, span as f32), 1.);

		// A centered outline spans a unit more
		assert_eq!(contour_gradient_span(8., StrokeAlign::Center), (4.5, 10.));
	}

	#[test]
	fn only_effects_that_show_change_something() {
		assert!(Effects::from_operations([EffectOperation::default_for(EffectKind::Blur)]).change_nothing());
		assert!(!Effects::from_operations([EffectOperation::default_for(EffectKind::DropShadow)]).change_nothing());
	}

	#[test]
	fn effects_on_content_left_as_stamped_measure_exactly_as_viewed() {
		// An image's pixel scale times its inverse rounds away from the identity, which would otherwise set up an effect space of its own
		let content = DAffine2::from_scale_angle_translation(DVec2::new(4000., 3000.), 10_f64.to_radians(), DVec2::new(250., 100.));
		let space = effect_space(content, stamped_transform(content));
		assert_eq!(space.matrix2, DMat2::IDENTITY);
		assert_eq!(space.translation, content.translation);

		// A transform applied since stamping carries the space along
		let scaled = DAffine2::from_scale(DVec2::splat(0.5)) * content;
		let space = effect_space(scaled, stamped_transform(content));
		assert!(space.matrix2.abs_diff_eq(DMat2::from_diagonal(DVec2::splat(0.5)), 1e-12));
	}

	#[test]
	fn runs_join_effects_stamped_rounding_apart_and_those_changing_nothing() {
		let blur = |radius: f64| EffectOperation::Blur {
			radius_mask: Graphic::default(),
			radius,
			box_blur: false,
			gamma: false,
		};
		let mut item = Item::new_from_element(());
		let mut stamp_at = |transform: DAffine2, operation: EffectOperation| {
			item.set_attribute(ATTR_TRANSFORM, transform);
			stamp_effect(&mut item, Effect::new(&operation));
		};

		// A turn and its undoing land back where they started only to within rounding, and a blur of no radius changes nothing wherever it's stamped
		let turned_back = DAffine2::from_angle(30_f64.to_radians()) * DAffine2::from_angle(-30_f64.to_radians());
		stamp_at(DAffine2::IDENTITY, blur(2.));
		stamp_at(turned_back, blur(3.));
		stamp_at(DAffine2::from_scale(DVec2::splat(2.)), blur(0.));
		stamp_at(DAffine2::from_scale(DVec2::splat(2.)), blur(4.));

		let Some(effects) = item.attribute::<Effects>(ATTR_EFFECTS) else {
			panic!("stamping should add the effects attribute");
		};
		let runs: Vec<_> = effects.runs().into_iter().map(|(_, rows)| rows).collect();
		assert_eq!(runs, vec![0..3, 3..4]);
	}

	#[test]
	fn adjustment_rows_elide_their_defaults_and_name_grouped_parameters() {
		use EffectKind::*;
		let kinds = [
			Desaturate,
			GammaCorrection,
			MakeOpaque,
			BrightnessContrast,
			Levels,
			Curves,
			BlackAndWhite,
			HueSaturation,
			Invert,
			Threshold,
			GradientMap,
			Vibrance,
			ChannelMixer,
			SelectiveColor,
			Posterize,
			Exposure,
			ColorBalance,
			PhotoFilter,
			ColorLookup,
			PaintOverlay,
		];
		for kind in kinds {
			let operation = EffectOperation::default_for(kind);
			assert_eq!(operation.kind(), kind);
			assert_eq!(Effect::new(&operation).0.attributes().iter_any().count(), 0, "{kind} stamps no defaults");
		}

		// A parameter in one of the node's groups rides the attribute named with the group's prefix
		let mut levels = EffectOperation::default_for(Levels);
		if let EffectOperation::Adjustment(Adjustment::Levels(settings)) = &mut levels {
			settings.blue.midtones = 1.5;
		}
		let row = Effect::new(&levels);
		assert_eq!(row.0.attribute::<f64>("blue_midtones"), Some(&1.5));
		assert_eq!(row.operation(), levels);
	}

	#[test]
	fn paints_keep_the_effects_stamped_on_their_wire_envelope() {
		let invert = Effect::new(&EffectOperation::Adjustment(Adjustment::Invert));
		let desaturate = Effect::new(&EffectOperation::Adjustment(Adjustment::Desaturate { method: Default::default() }));
		let operations = |item_effects: Option<&Effects>| item_effects.map(|effects| effects.operations().collect::<Vec<_>>()).unwrap_or_default();

		// A color keeps its own effects first, then takes the envelope's
		let mut color = Item::new_from_element(Color::RED);
		stamp_effect(&mut color, desaturate.clone());
		let mut envelope = Item::new_from_element(Graphic::Color(color));
		stamp_effect(&mut envelope, invert.clone());
		let Graphic::Color(color) = paint_with_envelope_effects(envelope) else {
			panic!("the color paint should stay a color")
		};
		assert_eq!(operations(color.attribute::<Effects>(ATTR_EFFECTS)), [desaturate.operation(), invert.operation()]);

		// Each row of a list of colors takes them, and other paints nest as a group carrying them
		let mut envelope = Item::new_from_element(Graphic::ColorList(List::new_from_element(Color::RED)));
		stamp_effect(&mut envelope, invert.clone());
		let Graphic::ColorList(list) = paint_with_envelope_effects(envelope) else {
			panic!("the list of colors should stay a list of colors")
		};
		assert_eq!(operations(list.clone_item(0).as_ref().and_then(|row| row.attribute::<Effects>(ATTR_EFFECTS))), [invert.operation()]);
		let mut envelope = Item::new_from_element(Graphic::VectorList(List::new_from_element(crate::Vector::default())));
		stamp_effect(&mut envelope, invert.clone());
		let Graphic::Graphic(group) = paint_with_envelope_effects(envelope) else {
			panic!("the vector paint should nest as a group")
		};
		assert_eq!(operations(group.attribute::<Effects>(ATTR_EFFECTS)), [invert.operation()]);

		// Without effects, the paint is the element alone, and no paint stays none
		let envelope = Item::new_from_element(Graphic::Color(Item::new_from_element(Color::RED)));
		assert!(matches!(paint_with_envelope_effects(envelope), Graphic::Color(color) if color.attribute::<Effects>(ATTR_EFFECTS).is_none()));
		let mut envelope = Item::new_from_element(Graphic::default());
		stamp_effect(&mut envelope, invert);
		assert!(matches!(paint_with_envelope_effects(envelope), Graphic::None(_)));
	}

	#[test]
	fn a_mask_paints_nothing_past_the_content_but_its_feather_reads_past_it() {
		let mask = EffectOperation::Mask {
			mask: Graphic::Color(Item::new_from_element(Color::WHITE)),
			mode: MaskMode::default(),
			density: 100.,
			feather: 4.,
			invert: false,
		};
		assert_eq!(mask.painted_reach(), 0.);
		assert_eq!(mask.working_reach(), 3. * mask_feather_sigma(4.));

		let effects = Effects::from_operations([mask]);
		assert_eq!((effects.reach(), effects.working_reach()), (0., 3. * mask_feather_sigma(4.)));
	}

	#[test]
	fn baking_the_content_placement_into_its_geometry_keeps_its_mask_in_place() {
		fn nested_placement(graphic: &Graphic) -> DAffine2 {
			match graphic {
				Graphic::Graphic(item) => item.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM) * nested_placement(item.element()),
				_ => DAffine2::IDENTITY,
			}
		}
		let masked_by = |mask: Graphic| {
			Effects::from_operations([EffectOperation::Mask {
				mask,
				mode: MaskMode::default(),
				density: 100.,
				feather: 0.,
				invert: false,
			}])
		};
		let mask_of = |effects: &Effects| match effects.operations().next() {
			Some(EffectOperation::Mask { mask, .. }) => mask,
			_ => panic!("the row should stay a mask"),
		};

		// A mask drawn at (5, 7) beside content placed by `placement`, which then goes into the content's geometry, leaving it unplaced
		let placement = DAffine2::from_scale_angle_translation(DVec2::splat(2.), 0.5, DVec2::new(200., 100.));
		let drawn_at = DAffine2::from_translation(DVec2::new(5., 7.));
		let mask = Graphic::Graphic(Box::new(Item::new_from_element(Graphic::default()).with_attribute(ATTR_TRANSFORM, drawn_at)));
		let mut effects = masked_by(linked_to_content(mask.clone(), placement));
		effects.rebase_stamped_transforms(placement);
		assert!(nested_placement(&mask_of(&effects)).abs_diff_eq(drawn_at, 1e-9));

		// A blur's radius mask follows the content the same way
		let blur = EffectOperation::Blur {
			radius_mask: linked_to_content(mask, placement),
			radius: 4.,
			box_blur: false,
			gamma: false,
		};
		let mut effects = Effects::from_operations([blur]);
		effects.rebase_stamped_transforms(placement);
		let Some(EffectOperation::Blur { radius_mask, .. }) = effects.operations().next() else {
			panic!("the row should stay a blur")
		};
		assert!(nested_placement(&radius_mask).abs_diff_eq(drawn_at, 1e-9));

		// A gradient without its own placement stretches across the content wherever it lies, so it's left as it is
		let unplaced = Graphic::Gradient(Item::new_from_element(Gradient::default()));
		let mut effects = masked_by(unplaced.clone());
		effects.rebase_stamped_transforms(placement);
		assert_eq!(mask_of(&effects), unplaced);
	}
}
