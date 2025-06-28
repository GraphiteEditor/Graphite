use crate::math::quad::Quad;
use crate::math::rect::Rect;
use bezier_rs::Bezier;

pub trait QuadExt {
	/// Get all the edges in the rect as linear bezier curves
	fn bezier_lines(&self) -> impl Iterator<Item = Bezier> + '_;
}

impl QuadExt for Quad {
	fn bezier_lines(&self) -> impl Iterator<Item = Bezier> + '_ {
		self.all_edges().into_iter().map(|[start, end]| Bezier::from_linear_dvec2(start, end))
	}
}

pub trait RectExt {
	/// Get all the edges in the quad as linear bezier curves
	fn bezier_lines(&self) -> impl Iterator<Item = Bezier> + '_;
}

impl RectExt for Rect {
	fn bezier_lines(&self) -> impl Iterator<Item = Bezier> + '_ {
		self.edges().into_iter().map(|[start, end]| Bezier::from_linear_dvec2(start, end))
	}
}
