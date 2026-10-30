//! An item's effects: an ordered list of image operations on the pixels the item renders to, stored in the `ATTR_EFFECTS` attribute.
//! Each row is an [`EffectKind`] whose parameters ride its item as attributes, read as their defaults when absent.
//! Each renderer decodes the rows into [`EffectOperation`]s and carries them out its own way, at the resolution of each render.

use crate::Graphic;
use crate::adjustment::Adjustment;
use core_types::ATTR_TRANSFORM;
use core_types::Color;
use core_types::blending::BlendMode;
use core_types::color::SRGBA8;
use core_types::graphene_hash::CacheHash;
use core_types::list::{ATTR_EFFECTS, Item, List};
use dyn_any::DynAny;
use glam::{DAffine2, DMat2, DVec2, DVec3};
use std::fmt::Debug;
use vector_types::vector::style::StrokeAlign;

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
	/// Glows inward from the content's edges, or outward from its center.
	InnerGlow,
	/// Shades the content's interior with soft bands from its silhouette cast two ways.
	Satin,
	/// Draws a band of color along the content's silhouette.
	Outline,
	/// Lights the content as if raised from the surface or pressed into it, with highlights and shadows along its edges.
	BevelAndEmboss,
	/// Paints over the content within its silhouette, mixing with its colors by a blend mode. A solid color paint is the color overlay adjustment.
	PaintOverlay,
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

/// The deviation of the blur that rounds a bevel's slopes of `size`, in the same units.
pub fn bevel_smoothing(size: f64) -> f64 {
	size.max(0.) / 4.
}

/// The unit vector toward a bevel's light from its `angle` and `altitude` in degrees, with `lengths_to_target` mapping document directions into the y-down space it's lit in.
pub fn bevel_light(angle: f64, altitude: f64, lengths_to_target: DMat2) -> DVec3 {
	let (angle, altitude) = (angle.to_radians(), altitude.clamp(0., 90.).to_radians());
	let across = (lengths_to_target * DVec2::new(angle.cos(), -angle.sin())).try_normalize().unwrap_or(DVec2::X);
	(across * altitude.cos()).extend(altitude.sin())
}

/// An overlay's paint with its unit square fitted to `content_bounds`, the content's extent in the space it's drawn in, if it's a gradient or image without its own placement.
pub fn fitted_paint(paint: &Graphic, content_bounds: [DVec2; 2]) -> Option<Graphic> {
	let unplaced = match paint {
		Graphic::Gradient(item) => item.attribute::<DAffine2>(ATTR_TRANSFORM).is_none(),
		Graphic::RasterCPU(item) => item.attribute::<DAffine2>(ATTR_TRANSFORM).is_none(),
		Graphic::RasterGPU(item) => item.attribute::<DAffine2>(ATTR_TRANSFORM).is_none(),
		_ => false,
	};
	if !unplaced {
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

	/// Decodes the parameters into an [`EffectOperation`], falling back to the default for any absent attribute.
	pub fn operation(&self) -> EffectOperation {
		let mut operation = EffectOperation::placeholder(self.kind());
		operation.visit_parameters(&mut ReadParameters(&self.0));

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

	/// The effects after the first `count`.
	pub fn after(&self, count: usize) -> Self {
		let mut remaining = Self::default();
		for effect in self.effects().skip(count) {
			remaining.0.push(Item::new_from_element(effect.clone()));
		}
		remaining
	}

	/// The effects decoded into the operations renderers carry out, in order.
	pub fn operations(&self) -> impl Iterator<Item = EffectOperation> + '_ {
		self.effects().map(Effect::operation)
	}

	/// How far past the content the effects together can paint, in document pixels.
	pub fn reach(&self) -> f64 {
		self.operations().map(|operation| operation.reach()).sum()
	}

	pub fn len(&self) -> usize {
		self.0.len()
	}

	pub fn is_empty(&self) -> bool {
		self.0.is_empty()
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
		color: Color,
		offset: DVec2,
		blur: f64,
		spread: f64,
	},
	InnerShadow {
		color: Color,
		offset: DVec2,
		blur: f64,
		spread: f64,
	},
	OuterGlow {
		color: Color,
		blur: f64,
		spread: f64,
	},
	InnerGlow {
		color: Color,
		blur: f64,
		/// How far the glow grows into the content before it softens.
		choke: f64,
		source: GlowSource,
	},
	Satin {
		color: Color,
		/// How far each of the two casts of the silhouette is moved, in opposite directions.
		offset: DVec2,
		blur: f64,
		invert: bool,
	},
	Outline {
		color: Color,
		weight: f64,
		align: StrokeAlign,
	},
	BevelAndEmboss {
		style: BevelStyle,
		/// How steep the slopes are, as a percentage.
		depth: f64,
		direction: BevelDirection,
		/// How wide the slopes are.
		size: f64,
		/// How much the highlights and shadows are blurred.
		soften: f64,
		/// The direction the light comes from, in degrees counterclockwise from the right.
		angle: f64,
		/// How high the light is above the surface, in degrees from flat to straight overhead.
		altitude: f64,
		highlight_color: Color,
		shadow_color: Color,
	},
	/// Paints over the content with a paint other than a solid color, since a solid one decodes into the color overlay adjustment instead.
	PaintOverlay {
		/// Placed in the space the content's item is, or fitted to the content's bounds if it's a gradient or image without its own placement.
		paint: Graphic,
		blend_mode: BlendMode,
		/// Percent.
		opacity: f64,
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
			Self::Adjustment(adjustment) => adjustment.kind(),
		}
	}

	/// How far past the content this operation can paint, or reads from, in document pixels.
	pub fn reach(&self) -> f64 {
		// A Gaussian is negligible past three standard deviations
		match self {
			Self::Blur { radius, .. } => 3. * radius.max(0.),
			Self::DropShadow { offset, blur, spread, .. } => offset.length() + spread.max(0.) + 3. * blur.max(0.),
			Self::InnerShadow { offset, blur, .. } => offset.length() + 3. * blur.max(0.),
			Self::OuterGlow { blur, spread, .. } => spread.max(0.) + 3. * blur.max(0.),
			Self::InnerGlow { blur, choke, .. } => choke.max(0.) + 3. * blur.max(0.),
			Self::Satin { offset, blur, .. } => offset.length() + 3. * blur.max(0.),
			Self::Outline { weight, .. } => weight.max(0.),
			Self::BevelAndEmboss { size, soften, .. } => size.max(0.) + 3. * (soften.max(0.) + bevel_smoothing(*size)),
			Self::PaintOverlay { .. } | Self::Adjustment(_) => 0.,
		}
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
				offset: DVec2::ZERO,
				blur: 0.,
				spread: 0.,
			},
			EffectKind::InnerShadow => Self::InnerShadow {
				color: Color::TRANSPARENT,
				offset: DVec2::ZERO,
				blur: 0.,
				spread: 0.,
			},
			EffectKind::OuterGlow => Self::OuterGlow {
				color: Color::TRANSPARENT,
				blur: 0.,
				spread: 0.,
			},
			EffectKind::InnerGlow => Self::InnerGlow {
				color: Color::TRANSPARENT,
				blur: 0.,
				choke: 0.,
				source: GlowSource::default(),
			},
			EffectKind::Satin => Self::Satin {
				color: Color::TRANSPARENT,
				offset: DVec2::ZERO,
				blur: 0.,
				invert: false,
			},
			EffectKind::Outline => Self::Outline {
				color: Color::TRANSPARENT,
				weight: 0.,
				align: StrokeAlign::default(),
			},
			EffectKind::BevelAndEmboss => Self::BevelAndEmboss {
				style: BevelStyle::default(),
				depth: 0.,
				direction: BevelDirection::default(),
				size: 0.,
				soften: 0.,
				angle: 0.,
				altitude: 0.,
				highlight_color: Color::TRANSPARENT,
				shadow_color: Color::TRANSPARENT,
			},
			EffectKind::PaintOverlay => Self::PaintOverlay {
				paint: Graphic::default(),
				blend_mode: BlendMode::default(),
				opacity: 0.,
			},
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
	fn visit_parameters(&mut self, visitor: &mut impl ParameterVisitor) {
		match self {
			Self::Blur { radius, box_blur, gamma } => {
				visitor.parameter("radius", radius, 0.);
				visitor.parameter("box_blur", box_blur, false);
				visitor.parameter("gamma", gamma, false);
			}
			Self::DropShadow { color, offset, blur, spread } | Self::InnerShadow { color, offset, blur, spread } => {
				visitor.parameter("color", color, Color::from(SRGBA8::new(0, 0, 0, 0x80)));
				visitor.parameter("offset", offset, DVec2::splat(4.));
				visitor.parameter("blur", blur, 4.);
				visitor.parameter("spread", spread, 0.);
			}
			Self::OuterGlow { color, blur, spread } => {
				visitor.parameter("color", color, Color::from(SRGBA8::new(0xff, 0xff, 0xbe, 0xbf)));
				visitor.parameter("blur", blur, 8.);
				visitor.parameter("spread", spread, 0.);
			}
			Self::InnerGlow { color, blur, choke, source } => {
				visitor.parameter("color", color, Color::from(SRGBA8::new(0xff, 0xff, 0xbe, 0xbf)));
				visitor.parameter("blur", blur, 8.);
				visitor.parameter("choke", choke, 0.);
				visitor.parameter("source", source, GlowSource::default());
			}
			Self::Satin { color, offset, blur, invert } => {
				visitor.parameter("color", color, Color::from(SRGBA8::new(0, 0, 0, 0x80)));
				visitor.parameter("offset", offset, DVec2::new(10., 4.));
				visitor.parameter("blur", blur, 14.);
				visitor.parameter("invert", invert, true);
			}
			Self::Outline { color, weight, align } => {
				visitor.parameter("color", color, Color::BLACK);
				visitor.parameter("weight", weight, 3.);
				visitor.parameter("align", align, StrokeAlign::Outside);
			}
			Self::BevelAndEmboss {
				style,
				depth,
				direction,
				size,
				soften,
				angle,
				altitude,
				highlight_color,
				shadow_color,
			} => {
				visitor.parameter("style", style, BevelStyle::default());
				visitor.parameter("depth", depth, 100.);
				visitor.parameter("direction", direction, BevelDirection::default());
				visitor.parameter("size", size, 5.);
				visitor.parameter("soften", soften, 0.);
				visitor.parameter("angle", angle, 120.);
				visitor.parameter("altitude", altitude, 30.);
				visitor.parameter("highlight_color", highlight_color, Color::from(SRGBA8::new(0xff, 0xff, 0xff, 0xbf)));
				visitor.parameter("shadow_color", shadow_color, Color::from(SRGBA8::new(0, 0, 0, 0xbf)));
			}
			Self::PaintOverlay { paint, blend_mode, opacity } => {
				visitor.parameter("paint", paint, Graphic::from(Color::BLACK));
				visitor.parameter("blend_mode", blend_mode, BlendMode::default());
				visitor.parameter("opacity", opacity, 100.);
			}
			Self::Adjustment(adjustment) => adjustment.visit_parameters(visitor),
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
	use vector_types::{Gradient, GradientStop};

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
				offset: DVec2::new(-2., 3.),
				blur: 1.,
				spread: 5.,
			},
			EffectOperation::OuterGlow {
				color: Color::BLUE,
				blur: 8.,
				spread: 2.,
			},
			EffectOperation::InnerGlow {
				color: Color::GREEN,
				blur: 3.,
				choke: 1.,
				source: GlowSource::Center,
			},
			EffectOperation::Satin {
				color: Color::BLACK,
				offset: DVec2::new(5., -6.),
				blur: 2.,
				invert: false,
			},
			EffectOperation::Outline {
				color: Color::RED,
				weight: 7.,
				align: StrokeAlign::Center,
			},
			EffectOperation::BevelAndEmboss {
				style: BevelStyle::PillowEmboss,
				depth: 250.,
				direction: BevelDirection::Down,
				size: 12.,
				soften: 2.,
				angle: -45.,
				altitude: 60.,
				highlight_color: Color::YELLOW,
				shadow_color: Color::BLUE,
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
}
