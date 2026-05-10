//! Not immediately shader compatible due to needing [`GradientStops`] as a param, which needs [`Vec`]

use crate::adjust::Adjust;
use core_types::list::{Item, List};
use core_types::{Color, Ctx};
use raster_types::{CPU, Raster};
use vector_types::GradientStops;

// Aims for interoperable compatibility with:
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=%27grdm%27%20%3D%20Gradient%20Map
// https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/#:~:text=Gradient%20settings%20(Photoshop%206.0)
#[node_macro::node(category("Raster: Adjustment"))]
async fn gradient_map<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(
		Item<List<Raster<CPU>>>,
		Item<List<Color>>,
		Item<List<GradientStops>>,
	)]
	image: Item<T>,
	gradient: Item<List<GradientStops>>,
	reverse: Item<bool>,
) -> Item<T> {
	let mut image = image.into_element();
	let gradient = gradient.into_element();
	let reverse = reverse.into_element();

	let Some(gradient) = gradient.element(0) else {
		return Item::new_from_element(image);
	};

	image.adjust(|color| {
		let intensity = color.luminance_srgb();
		let intensity = if reverse { 1. - intensity } else { intensity };
		gradient.evaluate(intensity as f64).to_linear_srgb()
	});

	Item::new_from_element(image)
}
