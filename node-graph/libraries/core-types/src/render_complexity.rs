// Raster types moved to raster-types crate
use crate::Color;
use crate::list::{Item, List};

pub trait RenderComplexity {
	fn render_complexity(&self) -> usize {
		0
	}

	/// The complexity of an item's element. A type with content that partly rides its item's attributes overrides
	/// this to read them, and the rest defer to the element alone.
	fn render_complexity_in_item(item: &Item<Self>) -> usize
	where
		Self: Sized,
	{
		item.element().render_complexity()
	}

	/// As [`Self::render_complexity_in_item`], for the element at one index of a list.
	fn render_complexity_in_list(list: &List<Self>, index: usize) -> usize
	where
		Self: Sized,
	{
		list.element(index).map_or(0, Self::render_complexity)
	}
}

impl<T: RenderComplexity> RenderComplexity for Item<T> {
	fn render_complexity(&self) -> usize {
		T::render_complexity_in_item(self)
	}
}

impl<T: RenderComplexity> RenderComplexity for List<T> {
	fn render_complexity(&self) -> usize {
		(0..self.len()).map(|index| T::render_complexity_in_list(self, index)).fold(0, usize::saturating_add)
	}
}

impl RenderComplexity for Color {
	fn render_complexity(&self) -> usize {
		1
	}
}

impl RenderComplexity for String {
	fn render_complexity(&self) -> usize {
		self.chars().count()
	}
}
