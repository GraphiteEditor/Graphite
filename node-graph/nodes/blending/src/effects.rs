use core_types::list::Item;
use core_types::{Color, Ctx};
use glam::DVec2;
use graphic_types::raster_types::{CPU, GPU, Raster};
use graphic_types::{Effect, EffectOperation, Graphic, Vector, stamp_effect};
use vector_types::Gradient;

/// Blurs the input graphics, whether vector shapes, images, or text.
#[node_macro::node(category("Effect"), icon("NodeBlur"))]
fn blur<T>(
	_: impl Ctx,
	/// The content to blur.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String)]
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
#[node_macro::node(category("Effect"), icon("NodeBlur"))]
fn drop_shadow<T>(
	_: impl Ctx,
	/// The content casting the shadow.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String)]
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
			color: *color.element(),
			offset: *offset.element(),
			blur: *blur.element(),
			spread: *spread.element(),
		}),
	);
	content
}

/// Casts a soft shadow inward from the edges of the input graphics, as if they were cut out of the surface.
#[node_macro::node(category("Effect"), icon("NodeBlur"))]
fn inner_shadow<T>(
	_: impl Ctx,
	/// The content the shadow falls inside.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String)]
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
			color: *color.element(),
			offset: *offset.element(),
			blur: *blur.element(),
			spread: *spread.element(),
		}),
	);
	content
}

/// Surrounds the input graphics with a soft glow of their silhouette.
#[node_macro::node(category("Effect"), icon("NodeBlur"))]
fn outer_glow<T>(
	_: impl Ctx,
	/// The content to glow around.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String)]
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
			color: *color.element(),
			blur: *blur.element(),
			spread: *spread.element(),
		}),
	);
	content
}
