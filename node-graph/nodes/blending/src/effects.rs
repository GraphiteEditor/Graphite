use core_types::list::Item;
use core_types::transfer_curve::TransferCurve;
use core_types::{Color, Ctx};
use graphic_types::raster_types::{CPU, GPU, Raster};
use graphic_types::{Effect, EffectOperation, Generator, GlowTechnique, Graphic, Vector, paint_with_envelope_effects, stamp_effect};
use raster_kernels::{evaluate_color_effects, evaluate_gradient_effects};
use vector_types::Gradient;

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
#[node_macro::node(category("Raster: Effect"), icon("NodeDropShadow"))]
fn drop_shadow<T>(
	_: impl Ctx,
	/// The content casting the shadow.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// The shadow's color, with its opacity.
	#[default("#000000bf")]
	color: Item<Color>,
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
#[node_macro::node(category("Raster: Effect"), icon("NodeInnerShadow"))]
fn inner_shadow<T>(
	_: impl Ctx,
	/// The content the shadow falls inside.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// The shadow's color, with its opacity.
	#[default("#000000bf")]
	color: Item<Color>,
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
			angle: *angle.element(),
			distance: *distance.element(),
			choke: *choke.element(),
			size: *size.element(),
			contour: contour.into_element(),
		}),
	);
	content
}

/// A glow's color or gradient paint with its own effects carried out.
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
#[node_macro::node(category("Raster: Effect"), icon("NodeOuterGlow"))]
fn outer_glow<T>(
	_: impl Ctx,
	/// The content to glow around.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// The glow's color with its opacity, or a gradient that runs from the content's edge outward across the glow.
	#[default("#ffffbebf")]
	color: Item<Graphic>,
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
			technique: *technique.element(),
			spread: *spread.element(),
			size: *size.element(),
			range: *range.element(),
			contour: contour.into_element(),
		}),
	);
	content
}
