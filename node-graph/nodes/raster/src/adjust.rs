use core_types::color::Color;
use core_types::list::Item;
use graphic_types::Adjustment;
use raster_kernels::adjustments::{PreparedAdjustment, adjust_color, adjust_colors};
use raster_kernels::evaluate_raster_effects;
use raster_types::{CPU, Raster};
use vector_types::Gradient;

pub trait Adjust: Sized {
	fn adjust(&mut self, adjustment: &Adjustment);

	/// The item with the effects stamped on it carried out, where the element holds the pixels they act on, so an adjustment after them applies to their result.
	fn carry_out_effects(item: Item<Self>) -> Item<Self> {
		item
	}
}

/// Applies the adjustment to the item's element, after the effects stamped on it before.
pub fn adjust_item<T: Adjust>(item: Item<T>, adjustment: &Adjustment) -> Item<T> {
	let mut item = T::carry_out_effects(item);
	item.element_mut().adjust(adjustment);
	item
}

impl Adjust for Color {
	fn adjust(&mut self, adjustment: &Adjustment) {
		*self = adjust_color(adjustment, *self);
	}
}

impl Adjust for Raster<CPU> {
	fn adjust(&mut self, adjustment: &Adjustment) {
		adjust_colors(adjustment, &mut self.data_mut().data);
	}

	fn carry_out_effects(item: Item<Self>) -> Item<Self> {
		evaluate_raster_effects(item)
	}
}

impl Adjust for Gradient {
	fn adjust(&mut self, adjustment: &Adjustment) {
		let adjustment = PreparedAdjustment::new(adjustment);
		*self = self.map_colors(|color| adjustment.adjust(*color));
	}
}
