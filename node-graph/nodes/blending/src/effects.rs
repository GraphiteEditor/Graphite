use core_types::list::Item;
use core_types::{Color, Ctx};
use glam::DVec2;
use graphic_types::raster_types::{CPU, GPU, Raster};
use graphic_types::{BevelDirection, BevelStyle, Effect, EffectOperation, Generator, GlowSource, Graphic, Vector, stamp_effect};
use raster_kernels::evaluate_color_effects;
use vector_types::Gradient;
use vector_types::vector::style::StrokeAlign;

/// Blurs the input graphics, whether vector shapes, images, or text.
#[node_macro::node(category("Raster: Filter"), icon("NodeBlur"))]
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
#[node_macro::node(category("Raster: Effect"), icon("NodeBlur"))]
fn drop_shadow<T>(
	_: impl Ctx,
	/// The content casting the shadow.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// The shadow's color, with its opacity.
	#[default("#00000080")]
	color: Item<Color>,
	/// How far the shadow sits from the content.
	#[unit(" px")]
	#[default(4., 4.)]
	offset: Item<DVec2>,
	/// How soft the shadow's edges are.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..100)]
	#[default(4.)]
	blur: Item<f64>,
	/// How far the shadow grows past the content's silhouette before it softens.
	#[unit(" px")]
	#[hard(0..)]
	spread: Item<f64>,
) -> Item<T> {
	let mut content = content;
	stamp_effect(
		&mut content,
		Effect::new(&EffectOperation::DropShadow {
			color: evaluate_color_effects(color).into_element(),
			offset: *offset.element(),
			blur: *blur.element(),
			spread: *spread.element(),
		}),
	);
	content
}

/// Casts a soft shadow inward from the edges of the input graphics, as if they were cut out of the surface.
#[node_macro::node(category("Raster: Effect"), icon("NodeBlur"))]
fn inner_shadow<T>(
	_: impl Ctx,
	/// The content the shadow falls inside.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// The shadow's color, with its opacity.
	#[default("#00000080")]
	color: Item<Color>,
	/// How far the shadow sits from the edges it falls from.
	#[unit(" px")]
	#[default(4., 4.)]
	offset: Item<DVec2>,
	/// How soft the shadow's edges are.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..100)]
	#[default(4.)]
	blur: Item<f64>,
	/// How far the shadow reaches in from the content's edges before it softens.
	#[unit(" px")]
	#[hard(0..)]
	spread: Item<f64>,
) -> Item<T> {
	let mut content = content;
	stamp_effect(
		&mut content,
		Effect::new(&EffectOperation::InnerShadow {
			color: evaluate_color_effects(color).into_element(),
			offset: *offset.element(),
			blur: *blur.element(),
			spread: *spread.element(),
		}),
	);
	content
}

/// Surrounds the input graphics with a soft glow of their silhouette.
#[node_macro::node(category("Raster: Effect"), icon("NodeBlur"))]
fn outer_glow<T>(
	_: impl Ctx,
	/// The content to glow around.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// The glow's color, with its opacity.
	#[default("#ffffbebf")]
	color: Item<Color>,
	/// How soft the glow's edges are.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..100)]
	#[default(8.)]
	blur: Item<f64>,
	/// How far the glow grows past the content's silhouette before it softens.
	#[unit(" px")]
	#[hard(0..)]
	spread: Item<f64>,
) -> Item<T> {
	let mut content = content;
	stamp_effect(
		&mut content,
		Effect::new(&EffectOperation::OuterGlow {
			color: evaluate_color_effects(color).into_element(),
			blur: *blur.element(),
			spread: *spread.element(),
		}),
	);
	content
}

/// Glows inward from the edges of the input graphics, or outward from their center.
#[node_macro::node(category("Raster: Effect"), icon("NodeBlur"))]
fn inner_glow<T>(
	_: impl Ctx,
	/// The content to glow within.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// The glow's color, with its opacity.
	#[default("#ffffbebf")]
	color: Item<Color>,
	/// How soft the glow's edges are.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..100)]
	#[default(8.)]
	blur: Item<f64>,
	/// How far the glow grows into the content before it softens.
	#[unit(" px")]
	#[hard(0..)]
	choke: Item<f64>,
	/// Whether the glow starts from the edges or the center.
	source: Item<GlowSource>,
) -> Item<T> {
	let mut content = content;
	stamp_effect(
		&mut content,
		Effect::new(&EffectOperation::InnerGlow {
			color: evaluate_color_effects(color).into_element(),
			blur: *blur.element(),
			choke: *choke.element(),
			source: *source.element(),
		}),
	);
	content
}

/// Shades the inside of the input graphics with soft, glossy bands that follow their shape.
#[node_macro::node(category("Raster: Effect"), icon("NodeBlur"))]
fn satin<T>(
	_: impl Ctx,
	/// The content to shade.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// The shading's color, with its opacity.
	#[default("#00000080")]
	color: Item<Color>,
	/// How far apart the bands are drawn, and in which direction.
	#[unit(" px")]
	#[default(10., 4.)]
	offset: Item<DVec2>,
	/// How soft the bands are.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..100)]
	#[default(14.)]
	blur: Item<f64>,
	/// Shade where the bands overlap rather than where they don't.
	#[default(true)]
	invert: Item<bool>,
) -> Item<T> {
	let mut content = content;
	stamp_effect(
		&mut content,
		Effect::new(&EffectOperation::Satin {
			color: evaluate_color_effects(color).into_element(),
			offset: *offset.element(),
			blur: *blur.element(),
			invert: *invert.element(),
		}),
	);
	content
}

/// Draws a band of color along the edge of the input graphics, outside, inside, or centered on it.
#[node_macro::node(category("Raster: Effect"), icon("NodeBlur"))]
fn outline<T>(
	_: impl Ctx,
	/// The content to outline.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// The outline's color, with its opacity.
	#[default(Color::BLACK)]
	color: Item<Color>,
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
) -> Item<T> {
	let mut content = content;
	stamp_effect(
		&mut content,
		Effect::new(&EffectOperation::Outline {
			color: evaluate_color_effects(color).into_element(),
			weight: *weight.element(),
			align: *align.element(),
		}),
	);
	content
}

/// Raises the input graphics into a lit relief, with highlights on the slopes facing the light and shadows on those facing away.
#[node_macro::node(name("Bevel & Emboss"), category("Raster: Effect"), icon("NodeBlur"))]
fn bevel_and_emboss<T>(
	_: impl Ctx,
	/// The content to raise.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// Where the slope lies: inside the edge, outside it, across it, or across it with the edge pressed in.
	style: Item<BevelStyle>,
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
	/// The shadow's color, with its opacity.
	#[default("#000000bf")]
	shadow_color: Item<Color>,
) -> Item<T> {
	let mut content = content;
	stamp_effect(
		&mut content,
		Effect::new(&EffectOperation::BevelAndEmboss {
			style: *style.element(),
			depth: *depth.element(),
			direction: *direction.element(),
			size: *size.element(),
			soften: *soften.element(),
			angle: *angle.element(),
			altitude: *altitude.element(),
			highlight_color: evaluate_color_effects(highlight_color).into_element(),
			shadow_color: evaluate_color_effects(shadow_color).into_element(),
		}),
	);
	content
}
