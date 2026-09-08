//! Not immediately shader compatible due to needing [`Gradient`] as a param, which needs [`Vec`]

use crate::adjust::Adjust;
use core_types::{Color, Ctx};
use raster_types::{CPU, Raster};
use vector_types::markers::{
	GradientCyclic as GradientCyclicAttr, GradientHueDirection as GradientHueDirectionAttr, GradientInterpolation as GradientInterpolationAttr, GradientSpace as GradientSpaceAttr,
	GradientSpread as GradientSpreadAttr,
};
use vector_types::{Gradient, GradientSettings};

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27grdm%27%20%3D%20Gradient%20Map
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=Gradient%20settings%20(Photoshop%206.0)
#[node_macro::node(category("Raster: Adjustment"))]
fn gradient_map<T: Adjust<Color> + Clone + Send + Sync + core_types::CacheHash + 'static>(
	_: impl Ctx,
	#[implementations(
		Raster<CPU>,
		Color,
		Gradient,
	)]
	mut image: T,
	#[default(Color::BLACK, Color::WHITE)] gradient: IList<Gradient>,
	reverse: bool,
) -> T {
	if gradient.is_empty() {
		return image;
	}
	// Master reads the whole-ramp settings off the item; ours ride the gradient's own lane.
	let lane = gradient.lane(0);
	let settings = GradientSettings {
		spread: lane.attr::<GradientSpreadAttr>(),
		cyclic: lane.attr::<GradientCyclicAttr>(),
		space: lane.attr::<GradientSpaceAttr>(),
		hue_direction: lane.attr::<GradientHueDirectionAttr>(),
		interpolation: lane.attr::<GradientInterpolationAttr>(),
	};
	let evaluator = gradient.element_ref(0).evaluator(settings);

	image.adjust(|color| {
		let intensity = color.luminance_rec_709();
		let intensity = if reverse { 1. - intensity } else { intensity };
		evaluator.evaluate(intensity as f64)
	});

	image
}
