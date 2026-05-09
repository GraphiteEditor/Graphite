// Raster types moved to raster-types crate
use crate::Color;
use crate::list::List;

pub trait RenderComplexity {
	fn render_complexity(&self) -> usize {
		0
	}
}

impl<T: RenderComplexity> RenderComplexity for List<T> {
	fn render_complexity(&self) -> usize {
		self.iter_element_values().map(|element| element.render_complexity()).fold(0, usize::saturating_add)
	}
}

impl RenderComplexity for Color {
	fn render_complexity(&self) -> usize {
		1
	}
}
