use core_types::blending::BlendMode;
use core_types::list::Item;
use core_types::transfer_curve::TransferCurve;
use core_types::{ATTR_OPACITY, ATTR_OPACITY_FILL, Color, Ctx};
use graphic_types::raster_types::{CPU, GPU, Raster};
use graphic_types::{BevelDirection, BevelStyle, BevelTechnique, Effect, EffectOperation, Generator, GlowSource, GlowTechnique, Graphic, Vector, paint_with_envelope_effects, stamp_effect};
use raster_kernels::{evaluate_color_effects, evaluate_gradient_effects};
use vector_types::Gradient;
use vector_types::vector::style::StrokeAlign;

/// Blurs the input graphics, whether vector shapes, images, or text.
#[node_macro::node(category("Raster: Filter"), icon("NodeBlur"), version(2))]
fn blur<T>(
	_: impl Ctx,
	/// The content to blur.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// How far the blur spreads each point of the content.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..100)]
	radius: Item<f64>,
	/// Use a box kernel instead of a circular Gaussian kernel, which averages evenly and leaves boxy artifacts.
	box_blur: Item<bool>,
	/// Opt to incorrectly blend the colors in gamma space for compatibility with the results from other software.
	gamma: Item<bool>,
) -> Item<T> {
	let mut content = content;
	stamp_effect(
		&mut content,
		Effect::new(&EffectOperation::Blur {
			radius: *radius.element(),
			box_blur: *box_blur.element(),
			gamma: *gamma.element(),
		}),
	);
	content
}

/// Casts a soft shadow of the input graphics' silhouette beneath them.
#[node_macro::node(category("Raster: Effect"), icon("NodeDropShadow"), version(2))]
fn drop_shadow<T>(
	_: impl Ctx,
	/// The content casting the shadow.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// The shadow's color, with its opacity.
	#[default("#000000bf")]
	color: Item<Color>,
	/// How the shadow mixes with the colors beneath it.
	#[default(BlendMode::Multiply)]
	blend_mode: Item<BlendMode>,
	/// The direction the light comes from, which the shadow falls away from.
	#[unit("°")]
	#[range]
	#[soft(-180..180)]
	#[default(120.)]
	angle: Item<f64>,
	/// How far the shadow falls from the content.
	#[unit(" px")]
	#[hard(0..)]
	#[default(5.)]
	distance: Item<f64>,
	/// How much of the size hardens the shadow by growing its edge outward, rather than softening it.
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	spread: Item<f64>,
	/// How far the shadow reaches past the content's silhouette.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..250)]
	#[default(5.)]
	size: Item<f64>,
	/// Reshapes how the shadow's strength falls off across its size.
	contour: Item<TransferCurve>,
	/// Hides the shadow behind the content even where the content's fill is faded.
	#[default(true)]
	knockout: Item<bool>,
) -> Item<T> {
	let mut content = content;
	stamp_effect(
		&mut content,
		Effect::new(&EffectOperation::DropShadow {
			color: evaluate_color_effects(color).into_element(),
			blend_mode: *blend_mode.element(),
			angle: *angle.element(),
			distance: *distance.element(),
			spread: *spread.element(),
			size: *size.element(),
			contour: contour.into_element(),
			knockout: *knockout.element(),
		}),
	);
	content
}

/// Casts a soft shadow inward from the edges of the input graphics, as if they were cut out of the surface.
#[node_macro::node(category("Raster: Effect"), icon("NodeInnerShadow"), version(2))]
fn inner_shadow<T>(
	_: impl Ctx,
	/// The content the shadow falls inside.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// The shadow's color, with its opacity.
	#[default("#000000bf")]
	color: Item<Color>,
	/// How the shadow mixes with the content's colors, and with what shows through where the content is faded.
	#[default(BlendMode::Multiply)]
	blend_mode: Item<BlendMode>,
	/// The direction the light comes from, which the edges facing it cast the shadow away from.
	#[unit("°")]
	#[range]
	#[soft(-180..180)]
	#[default(120.)]
	angle: Item<f64>,
	/// How far the shadow falls from the edges casting it.
	#[unit(" px")]
	#[hard(0..)]
	#[default(5.)]
	distance: Item<f64>,
	/// How much of the size hardens the shadow by growing its edge inward, rather than softening it.
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	choke: Item<f64>,
	/// How far the shadow reaches in from the content's edges.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..250)]
	#[default(5.)]
	size: Item<f64>,
	/// Reshapes how the shadow's strength falls off across its size.
	contour: Item<TransferCurve>,
) -> Item<T> {
	let mut content = content;
	stamp_effect(
		&mut content,
		Effect::new(&EffectOperation::InnerShadow {
			color: evaluate_color_effects(color).into_element(),
			blend_mode: *blend_mode.element(),
			angle: *angle.element(),
			distance: *distance.element(),
			choke: *choke.element(),
			size: *size.element(),
			contour: contour.into_element(),
		}),
	);
	content
}

/// A glow's, outline's, or bevel texture's paint with its own effects carried out, if it's a color or gradient.
fn glow_paint(paint: Item<Graphic>) -> Graphic {
	match paint_with_envelope_effects(paint) {
		Graphic::Color(color) => Graphic::Color(evaluate_color_effects(color)),
		Graphic::Gradient(gradient) => Graphic::Gradient(evaluate_gradient_effects(gradient)),
		Graphic::ColorList(colors) => Graphic::ColorList(colors.into_iter().map(evaluate_color_effects).collect()),
		Graphic::GradientList(gradients) => Graphic::GradientList(gradients.into_iter().map(evaluate_gradient_effects).collect()),
		paint => paint,
	}
}

/// Surrounds the input graphics with a soft glow of their silhouette.
#[node_macro::node(category("Raster: Effect"), icon("NodeOuterGlow"), version(2))]
fn outer_glow<T>(
	_: impl Ctx,
	/// The content to glow around.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// The glow's color with its opacity, or a gradient that runs from the content's edge outward across the glow.
	#[default("#ffffbebf")]
	color: Item<Graphic>,
	/// How the glow mixes with the colors beneath it.
	#[default(BlendMode::Screen)]
	blend_mode: Item<BlendMode>,
	/// Whether the glow fades like a blur or evenly with the distance from the content.
	technique: Item<GlowTechnique>,
	/// How much of the size hardens the glow by growing its edge outward, rather than softening it.
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	spread: Item<f64>,
	/// How far the glow reaches past the content's silhouette.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..250)]
	#[default(5.)]
	size: Item<f64>,
	/// The share of the glow's strength at which it reaches full opacity, so a lower range makes it denser.
	#[unit("%")]
	#[range]
	#[hard(1..100)]
	#[default(50.)]
	range: Item<f64>,
	/// Reshapes how the glow's strength falls off across its size.
	contour: Item<TransferCurve>,
) -> Item<T> {
	let mut content = content;
	stamp_effect(
		&mut content,
		Effect::new(&EffectOperation::OuterGlow {
			color: glow_paint(color),
			blend_mode: *blend_mode.element(),
			technique: *technique.element(),
			spread: *spread.element(),
			size: *size.element(),
			range: *range.element(),
			contour: contour.into_element(),
		}),
	);
	content
}

/// Glows inward from the edges of the input graphics, or outward from their center.
#[node_macro::node(category("Raster: Effect"), icon("NodeInnerGlow"), version(2))]
fn inner_glow<T>(
	_: impl Ctx,
	/// The content to glow within.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// The glow's color with its opacity, or a gradient that runs from the content's edges inward (or from its center outward) across the glow.
	#[default("#ffffbebf")]
	color: Item<Graphic>,
	/// How the glow mixes with the content's colors, and with what shows through where the content is faded.
	#[default(BlendMode::Screen)]
	blend_mode: Item<BlendMode>,
	/// Whether the glow fades like a blur or evenly with the distance from the content's edges.
	technique: Item<GlowTechnique>,
	/// Whether the glow starts from the edges or the center.
	source: Item<GlowSource>,
	/// How much of the size hardens the glow by growing its edge inward, rather than softening it.
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	choke: Item<f64>,
	/// How far the glow reaches in from the content's edges.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..250)]
	#[default(5.)]
	size: Item<f64>,
	/// The share of the glow's strength at which it reaches full opacity, so a lower range makes it denser.
	#[unit("%")]
	#[range]
	#[hard(1..100)]
	#[default(50.)]
	range: Item<f64>,
	/// Reshapes how the glow's strength falls off across its size.
	contour: Item<TransferCurve>,
) -> Item<T> {
	let mut content = content;
	stamp_effect(
		&mut content,
		Effect::new(&EffectOperation::InnerGlow {
			color: glow_paint(color),
			blend_mode: *blend_mode.element(),
			technique: *technique.element(),
			source: *source.element(),
			choke: *choke.element(),
			size: *size.element(),
			range: *range.element(),
			contour: contour.into_element(),
		}),
	);
	content
}

/// Shades the inside of the input graphics with soft, glossy bands that follow their shape.
#[node_macro::node(category("Raster: Effect"), icon("NodeSatin"), version(2))]
fn satin<T>(
	_: impl Ctx,
	/// The content to shade.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// The shading's color, with its opacity.
	#[default("#00000080")]
	color: Item<Color>,
	/// How the shading mixes with the content's colors, and with what shows through where the content is faded.
	#[default(BlendMode::Multiply)]
	blend_mode: Item<BlendMode>,
	/// The direction the two casts of the content's silhouette are moved along, one each way, which the bands run across.
	#[unit("°")]
	#[range]
	#[soft(-180..180)]
	#[default(19.)]
	angle: Item<f64>,
	/// How far each cast of the silhouette is moved, which sets how far apart the bands are.
	#[unit(" px")]
	#[hard(0..)]
	#[default(11.)]
	distance: Item<f64>,
	/// How soft the bands are.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..250)]
	#[default(14.)]
	size: Item<f64>,
	/// Reshapes the shading's strength across the bands.
	contour: Item<TransferCurve>,
	/// Shade where the bands overlap rather than where they don't.
	#[default(true)]
	invert: Item<bool>,
) -> Item<T> {
	let mut content = content;
	stamp_effect(
		&mut content,
		Effect::new(&EffectOperation::Satin {
			color: evaluate_color_effects(color).into_element(),
			blend_mode: *blend_mode.element(),
			angle: *angle.element(),
			distance: *distance.element(),
			size: *size.element(),
			invert: *invert.element(),
			contour: contour.into_element(),
		}),
	);
	content
}

/// Paints a band along the edge of the input graphics, outside, inside, or centered on it.
#[node_macro::node(category("Raster: Effect"), icon("NodeOutline"), version(3))]
fn outline<T>(
	_: impl Ctx,
	/// The content to outline.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// The color, gradient, image, or other graphic to paint the outline with. A gradient or image without its own placement stretches across the content and its outline.
	#[default(Color::BLACK)]
	paint: Item<Graphic>,
	/// How the outline mixes with the colors beneath it, or with the content's colors inside its edge.
	blend_mode: Item<BlendMode>,
	/// How thick the outline is.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..100)]
	#[default(3.)]
	weight: Item<f64>,
	/// Whether the outline is drawn outside the edge, inside it, or centered on it.
	#[default(StrokeAlign::Outside)]
	align: Item<StrokeAlign>,
	/// Runs a gradient paint across the outline's width, from its outer edge to its inner edge, rather than by the gradient's own placement.
	contour_gradient: Item<bool>,
) -> Item<T> {
	// Stamped even with no paint, which draws nothing, so whether the content has effects doesn't depend on a value
	let mut content = content;
	stamp_effect(
		&mut content,
		Effect::new(&EffectOperation::Outline {
			paint: glow_paint(paint),
			blend_mode: *blend_mode.element(),
			weight: *weight.element(),
			align: *align.element(),
			contour_gradient: *contour_gradient.element(),
		}),
	);
	content
}

/// Raises the input graphics into a lit relief, with highlights on the slopes facing the light and shadows on those facing away.
#[node_macro::node(name("Bevel & Emboss"), category("Raster: Effect"), icon("NodeBevelAndEmboss"), version(2))]
fn bevel_and_emboss<T>(
	_: impl Ctx,
	/// The content to raise.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// Where the slope lies: inside the edge, outside it, across it, or across it with the edge pressed in.
	style: Item<BevelStyle>,
	/// Whether the slope is rounded like a blur, or straight and following the content's details.
	technique: Item<BevelTechnique>,
	/// How steep the slope is.
	#[unit("%")]
	#[range]
	#[hard(0..)]
	#[soft(..1000)]
	#[default(100.)]
	depth: Item<f64>,
	/// Whether the slope rises out of the surface or sinks into it.
	direction: Item<BevelDirection>,
	/// How wide the slope is.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..250)]
	#[default(5.)]
	size: Item<f64>,
	/// How much the shading is blurred.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..16)]
	soften: Item<f64>,
	/// The direction the light comes from.
	#[widget(ParsedWidgetOverride::Custom = "wrapping_range_slider")]
	#[unit("°")]
	#[range]
	#[soft(-180..180)]
	#[default(120.)]
	angle: Item<f64>,
	/// How high above the surface the light is, from the horizon to straight overhead.
	#[unit("°")]
	#[range]
	#[hard(0..90)]
	#[default(30.)]
	altitude: Item<f64>,
	/// The highlight's color, with its opacity.
	#[default("#ffffffbf")]
	highlight_color: Item<Color>,
	/// How the highlight mixes with the colors beneath it.
	#[default(BlendMode::Screen)]
	highlight_blend_mode: Item<BlendMode>,
	/// The shadow's color, with its opacity.
	#[default("#000000bf")]
	shadow_color: Item<Color>,
	/// How the shadow mixes with the colors beneath it.
	#[default(BlendMode::Multiply)]
	shadow_blend_mode: Item<BlendMode>,
	/// A graphic with lightness that sinks the surface into a relief, like a pattern pressed into it. A gradient or image without its own placement stretches across the content.
	texture: Item<Graphic>,
	/// How deep the texture's lightest parts sink, as a share of the slope's width, or how high they rise when negative.
	#[unit("%")]
	#[range]
	#[soft(-1000..1000)]
	#[default(100.)]
	texture_depth: Item<f64>,
	/// How much of the slope the slope contour spans: down from the top of an inner bevel, or up from the foot of an outer one. An emboss's always spans its whole slope. At 100%, a straight one leaves the slope as it is.
	#[unit("%")]
	#[range]
	#[hard(1..100)]
	#[default(100.)]
	slope_range: Item<f64>,
	/// Samples the gloss contour across each pixel, smoothing its steep parts.
	gloss_antialiased: Item<bool>,
	/// Samples the slope contour across each pixel, smoothing its steep parts.
	slope_antialiased: Item<bool>,
	/// Reshapes how lit the slope is, from where it faces away from the light to where it faces it, for glossy or metallic sheens.
	gloss_contour: Item<TransferCurve>,
	/// Reshapes the slope's profile, for ridges, grooves, and rounded or stepped edges.
	slope_contour: Item<TransferCurve>,
) -> Item<T> {
	let mut content = content;
	stamp_effect(
		&mut content,
		Effect::new(&EffectOperation::BevelAndEmboss {
			style: *style.element(),
			technique: *technique.element(),
			depth: *depth.element(),
			direction: *direction.element(),
			size: *size.element(),
			soften: *soften.element(),
			angle: *angle.element(),
			altitude: *altitude.element(),
			highlight_color: evaluate_color_effects(highlight_color).into_element(),
			highlight_blend_mode: *highlight_blend_mode.element(),
			shadow_color: evaluate_color_effects(shadow_color).into_element(),
			shadow_blend_mode: *shadow_blend_mode.element(),
			texture: glow_paint(texture),
			texture_depth: *texture_depth.element(),
			slope_range: *slope_range.element(),
			gloss_antialiased: *gloss_antialiased.element(),
			slope_antialiased: *slope_antialiased.element(),
			gloss_contour: gloss_contour.into_element(),
			slope_contour: slope_contour.into_element(),
		}),
	);
	content
}

/// Paints over the input graphics within their shape, mixing the paint with their colors by a blend mode.
#[node_macro::node(category("Raster: Effect"), icon("NodeColorOverlay"), version(2))]
fn paint_overlay<T>(
	_: impl Ctx,
	/// The content to paint over.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// The color, gradient, image, or other graphic to paint with. A gradient or image without its own placement stretches across the content.
	#[default(Color::BLACK)]
	paint: Item<Graphic>,
	/// How the paint mixes with the content's colors, and with what shows through where the content is faded.
	blend_mode: Item<BlendMode>,
	/// How strongly the paint shows.
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(100.)]
	opacity: Item<f64>,
) -> Item<T> {
	// Only the wire envelope's effects carry over, since its other attributes would nest the paint as a group
	let paint = match paint_with_envelope_effects(paint) {
		Graphic::None(_) | Graphic::NoneList(_) => return content,
		// A solid paint becomes a bare color, so its own opacity fades the color's alpha
		Graphic::Color(color) => {
			let opacity = color.attribute_cloned_or::<f64>(ATTR_OPACITY, 1.) * color.attribute_cloned_or::<f64>(ATTR_OPACITY_FILL, 1.);
			let color = evaluate_color_effects(color).into_element();
			Graphic::from(color.with_alpha(color.a() * opacity as f32))
		}
		paint => paint,
	};

	let mut content = content;
	stamp_effect(
		&mut content,
		Effect::new(&EffectOperation::PaintOverlay {
			paint,
			blend_mode: blend_mode.into_element(),
			opacity: opacity.into_element(),
		}),
	);
	content
}

#[cfg(test)]
mod test {
	use core_types::Color;
	use core_types::blending::BlendMode;
	use core_types::list::Item;
	use graphic_types::Graphic;
	use graphic_types::raster_types::{Image, Raster};

	#[test]
	fn paint_overlay_multiplies_a_color_into_the_content() {
		let image_color = Color::from_rgbaf32_unchecked(0.7, 0.6, 0.5, 0.4);
		let image = Image::new(1, 1, image_color);

		let result = super::paint_overlay(
			(),
			Item::new_from_element(Raster::new_cpu(image)),
			Item::new_from_element(Graphic::from(Color::GREEN)),
			BlendMode::Multiply.into(),
			100_f64.into(),
		);
		let result = raster_kernels::evaluate_raster_effects(result).into_element();

		// Multiplying by green keeps only the green channel. It blends within the content's share, so the translucent content keeps its own alpha.
		let expected = Color::from_rgbaf32_unchecked(0., image_color.g(), 0., image_color.a());
		assert!((result.data[0].to_vec4() - expected.to_vec4()).abs().max_element() < 1e-4, "{:?} against {expected:?}", result.data[0]);
	}

	#[test]
	fn paint_overlay_fades_a_color_paint_by_its_own_opacity() {
		use core_types::ATTR_OPACITY;

		let paint = Graphic::Color(Item::new_from_element(Color::BLACK).with_attribute(ATTR_OPACITY, 0.5_f64));
		let result = super::paint_overlay(
			(),
			Item::new_from_element(Raster::new_cpu(Image::new(1, 1, Color::WHITE))),
			Item::new_from_element(paint),
			BlendMode::Normal.into(),
			100_f64.into(),
		);
		let color = raster_kernels::evaluate_raster_effects(result).into_element().data[0];

		assert!(color.r() > 0.05 && color.r() < 0.95, "half the black paint leaves {color:?}");
	}
}
