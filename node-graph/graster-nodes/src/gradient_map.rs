//! Not immediately shader compatible due to needing [`GradientStops`] as a param, which needs [`Vec`]

use crate::adjust::Adjust;
use graphene_core::gradient::GradientStops;
use graphene_core::raster_types::{CPU, Raster};
use graphene_core::table::Table;
use graphene_core::{Color, Ctx};

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27grdm%27%20%3D%20Gradient%20Map
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=Gradient%20settings%20(Photoshop%206.0)
#[node_macro::node(category("Raster: Adjustment"))]
async fn gradient_map<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(
		Color,
		Table<Raster<CPU>>,
		GradientStops,
	)]
	mut image: T,
	gradient: GradientStops,
	reverse: bool,
) -> T {
	image.adjust(|color| {
		let intensity = color.luminance_srgb();
		let intensity = if reverse { 1. - intensity } else { intensity };
		gradient.evaluate(intensity as f64).to_linear_srgb()
	});

	image
}
