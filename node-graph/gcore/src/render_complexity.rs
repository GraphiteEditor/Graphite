use crate::gradient::{Gradient, GradientStops};
use crate::instances::Instances;
use crate::raster_types::{CPU, GPU, Raster};
use crate::vector::VectorData;
use crate::{Artboard, Color, GraphicElement};
use glam::DVec2;

pub trait RenderComplexity {
	fn render_complexity(&self) -> usize {
		0
	}
}

impl<T: RenderComplexity> RenderComplexity for Instances<T> {
	fn render_complexity(&self) -> usize {
		self.instance_ref_iter().map(|instance| instance.instance.render_complexity()).fold(0, usize::saturating_add)
	}
}

impl RenderComplexity for Artboard {
	fn render_complexity(&self) -> usize {
		self.graphic_group.render_complexity()
	}
}

impl RenderComplexity for GraphicElement {
	fn render_complexity(&self) -> usize {
		match self {
			Self::GraphicGroup(instances) => instances.render_complexity(),
			Self::VectorData(instances) => instances.render_complexity(),
			Self::RasterDataCPU(instances) => instances.render_complexity(),
			Self::RasterDataGPU(instances) => instances.render_complexity(),
		}
	}
}

impl RenderComplexity for VectorData {
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

impl RenderComplexity for String {}
impl RenderComplexity for bool {}
impl RenderComplexity for u32 {}
impl RenderComplexity for f64 {}
impl RenderComplexity for DVec2 {}
impl RenderComplexity for Option<Color> {}
impl RenderComplexity for Vec<Color> {}
impl RenderComplexity for GradientStops {}
impl RenderComplexity for Gradient {}
