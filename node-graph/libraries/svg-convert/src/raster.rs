use core_types::ATTR_TRANSFORM;
use core_types::list::{Item, List};
use glam::{DAffine2, DVec2};
use graphic_types::Graphic;
use graphic_types::raster_types::{Image, Raster};

pub fn raster_item(encoded: &[u8], size: usvg::Size) -> Option<Item<Graphic>> {
	let mut raster = Item::new_from_element(Raster::new_cpu(Image::from_encoded(encoded).ok()?));
	let transform = DAffine2::from_scale(DVec2::new(size.width().into(), size.height().into()));
	if transform != DAffine2::IDENTITY {
		raster.set_attribute(ATTR_TRANSFORM, transform);
	}
	Some(Item::new_from_element(Graphic::RasterCPUList(List::new_from_item(raster))))
}
