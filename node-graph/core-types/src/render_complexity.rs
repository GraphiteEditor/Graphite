// Raster types moved to raster-types crate
use crate::Color;
use crate::table::Table;

pub trait RenderComplexity {
	fn render_complexity(&self) -> usize {
		0
	}
}

impl<T: RenderComplexity> RenderComplexity for Table<T> {
	fn render_complexity(&self) -> usize {
		self.iter().map(|row| row.element.render_complexity()).fold(0, usize::saturating_add)
	}
}

impl RenderComplexity for Color {
	fn render_complexity(&self) -> usize {
		1
	}
}
