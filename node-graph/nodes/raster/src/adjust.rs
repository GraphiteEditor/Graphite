use core_types::list::Item;
use graphic_types::{Adjustment, Effect, EffectOperation, Graphic, Vector, stamp_effect};
use no_std_types::color::Color;
use raster_kernels::adjustments::{adjust_color, adjust_colors};
use raster_types::{CPU, GPU, Raster};
use vector_types::Gradient;

pub trait Adjust: Sized {
	/// The item with the adjustment applied after everything done to it so far.
	fn adjust(item: Item<Self>, adjustment: &Adjustment) -> Item<Self>;
}

impl Adjust for Color {
	fn adjust(item: Item<Self>, adjustment: &Adjustment) -> Item<Self> {
		let mut item = item;
		*item.element_mut() = adjust_color(adjustment, *item.element());
		item
	}
}

impl Adjust for Raster<CPU> {
	fn adjust(item: Item<Self>, adjustment: &Adjustment) -> Item<Self> {
		let mut item = item;
		adjust_colors(adjustment, &mut item.element_mut().data_mut().data);
		item
	}
}

impl Adjust for Gradient {
	fn adjust(item: Item<Self>, adjustment: &Adjustment) -> Item<Self> {
		let mut item = item;
		let adjusted = item.element().map_colors(|color| adjust_color(adjustment, *color));
		*item.element_mut() = adjusted;
		item
	}
}

/// Content that the renderer draws carries the adjustment as an effect, which each render carries out at its own resolution.
macro_rules! adjust_by_stamping {
	($($content:ty),*) => {$(
		impl Adjust for $content {
			fn adjust(item: Item<Self>, adjustment: &Adjustment) -> Item<Self> {
				let mut item = item;
				stamp_effect(&mut item, Effect::new(&EffectOperation::Adjustment(adjustment.clone())));
				item
			}
		}
	)*};
}
adjust_by_stamping!(Graphic, Vector, Raster<GPU>, String);
