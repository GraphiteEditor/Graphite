use core_types::blending::BlendMode;
use core_types::list::Item;
use core_types::{Color, Ctx};
use glam::DVec2;
use graphic_types::raster_types::{CPU, GPU, Raster};
use graphic_types::{Effect, EffectOperation, Generator, Graphic, Vector, stamp_effect};
use raster_kernels::evaluate_color_effects;
use vector_types::Gradient;

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

/// Paints over the input graphics within their shape, mixing the paint with their colors by a blend mode.
#[node_macro::node(category("Raster: Effect"), icon("NodeColorOverlay"))]
fn paint_overlay<T>(
	_: impl Ctx,
	/// The content to paint over.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// The color, gradient, image, or other graphic to paint with. A gradient or image without its own placement stretches across the content.
	#[default(Color::BLACK)]
	paint: Item<Graphic>,
	/// How the paint mixes with the content's colors.
	blend_mode: Item<BlendMode>,
	/// How strongly the paint shows.
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(100.)]
	opacity: Item<f64>,
) -> Item<T> {
	// The paint is the element alone, since the wire envelope's attributes would nest it as a group
	let paint = match paint.into_element() {
		Graphic::None(_) | Graphic::NoneList(_) => return content,
		Graphic::Color(color) => Graphic::from(evaluate_color_effects(color).into_element()),
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

		// Multiplying by green keeps only the green channel, and the content keeps its own alpha
		assert_eq!(result.data[0], Color::from_rgbaf32_unchecked(0., image_color.g(), 0., image_color.a()));
	}
}
