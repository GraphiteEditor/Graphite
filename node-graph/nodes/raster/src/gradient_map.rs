//! Not immediately shader compatible due to needing [`GradientStops`] as a param, which needs [`Vec`]

use crate::adjust::Adjust;
use core_types::{Color, Ctx, ExtractIndex, InjectIndex};
use raster_types::{CPU, Raster};
use vector_types::GradientStops;

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27grdm%27%20%3D%20Gradient%20Map
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=Gradient%20settings%20(Photoshop%206.0)
#[node_macro::node(category("Raster: Adjustment"))]
fn gradient_map<T: Adjust<Color> + Clone + Send + Sync + core_types::CacheHash + 'static>(
	_: impl Ctx + ExtractIndex + InjectIndex + Copy,
	#[implementations(
		Raster<CPU>,
		Color,
		GradientStops,
	)]
	mut image: T,
	gradient: IList<GradientStops>,
	reverse: bool,
) -> T {
	if gradient.is_empty() {
		return image;
	}
	let gradient = gradient.element_ref(0);

	image.adjust(|color| {
		let intensity = color.luminance_rec_709();
		let intensity = if reverse { 1. - intensity } else { intensity };
		gradient.evaluate(intensity as f64)
	});

	image
}
