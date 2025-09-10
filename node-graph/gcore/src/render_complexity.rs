use crate::gradient::GradientStops;
use crate::raster_types::{CPU, GPU, Raster};
use crate::table::Table;
use crate::vector::Vector;
use crate::{Artboard, Color, Graphic};

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

impl RenderComplexity for Artboard {
	fn render_complexity(&self) -> usize {
		self.content.render_complexity()
	}
}

impl RenderComplexity for Graphic {
	fn render_complexity(&self) -> usize {
		match self {
			Self::Graphic(table) => table.render_complexity(),
			Self::Vector(table) => table.render_complexity(),
			Self::RasterCPU(table) => table.render_complexity(),
			Self::RasterGPU(table) => table.render_complexity(),
			Self::Color(table) => table.render_complexity(),
			Self::Gradient(table) => table.render_complexity(),
		}
	}
}

impl RenderComplexity for Vector {
	fn render_complexity(&self) -> usize {
		self.segment_domain.ids().len()
	}
}

impl RenderComplexity for Raster<CPU> {
	fn render_complexity(&self) -> usize {
		(self.width * self.height / 500) as usize
	}
}

impl RenderComplexity for Raster<GPU> {
	fn render_complexity(&self) -> usize {
		// GPU textures currently can't have a thumbnail
		usize::MAX
	}
}

impl RenderComplexity for Color {
	fn render_complexity(&self) -> usize {
		1
	}
}

impl RenderComplexity for GradientStops {
	fn render_complexity(&self) -> usize {
		1
	}
}
