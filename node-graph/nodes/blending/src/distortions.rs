use core_types::list::Item;
use core_types::{Color, Ctx};
use glam::DVec2;
use graphic_types::raster_types::{CPU, GPU, Raster};
use graphic_types::{Effect, EffectOperation, Generator, Graphic, RadialBlurMethod, Vector, stamp_effect};
use vector_types::Gradient;

/// Smears the input graphics along a line, as if they moved while being captured.
#[node_macro::node(category("Raster: Filter"))]
fn motion_blur<T>(
	_: impl Ctx,
	/// The content to smear.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// The direction of the line each point is smeared along, counterclockwise from the right. Opposite directions smear alike.
	#[unit("°")]
	#[range]
	#[soft(-90..90)]
	angle: Item<f64>,
	/// How long the line each point is smeared along is.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..250)]
	#[default(10.)]
	distance: Item<f64>,
	/// Opt to incorrectly blend the colors in gamma space for compatibility with the results from other software.
	gamma: Item<bool>,
) -> Item<T> {
	let mut content = content;
	stamp_effect(
		&mut content,
		Effect::new(&EffectOperation::MotionBlur {
			angle: *angle.element(),
			distance: *distance.element(),
			gamma: *gamma.element(),
		}),
	);
	content
}

/// Smears the input graphics along circles around a center, or along lines through it, placed relative to their bounds.
#[node_macro::node(category("Raster: Filter"))]
fn radial_blur<T>(
	_: impl Ctx,
	/// The content to smear.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// Whether the content spins around the center or zooms toward it.
	method: Item<RadialBlurMethod>,
	/// How far the content smears: the degrees each point spins through, or for a zoom, twice the percentage of its distance from the center that it smears across.
	#[range]
	#[hard(0..360)]
	#[soft(..100)]
	#[default(10.)]
	amount: Item<f64>,
	/// Where the center lies, as a percentage of the content's width and height away from its middle, so (-50, -50) is its top left corner.
	#[unit("%")]
	center: Item<DVec2>,
	/// Opt to incorrectly blend the colors in gamma space for compatibility with the results from other software.
	gamma: Item<bool>,
) -> Item<T> {
	let mut content = content;
	stamp_effect(
		&mut content,
		Effect::new(&EffectOperation::RadialBlur {
			method: *method.element(),
			amount: *amount.element(),
			center: *center.element(),
			gamma: *gamma.element(),
		}),
	);
	content
}
