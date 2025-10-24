use crate::math::quad::Quad;
use crate::math::rect::Rect;
use crate::subpath::Bezier;
use crate::vector::misc::dvec2_to_point;
use kurbo::{Line, PathSeg};

pub trait QuadExt {
	/// Get all the edges in the rect as linear bezier curves
	fn bezier_lines(&self) -> impl Iterator<Item = Bezier> + '_;
	fn to_lines(&self) -> impl Iterator<Item = PathSeg>;
}

impl QuadExt for Quad {
	fn bezier_lines(&self) -> impl Iterator<Item = Bezier> + '_ {
		self.all_edges().into_iter().map(|[start, end]| Bezier::from_linear_dvec2(start, end))
	}

	fn to_lines(&self) -> impl Iterator<Item = PathSeg> {
		self.all_edges().into_iter().map(|[start, end]| PathSeg::Line(Line::new(dvec2_to_point(start), dvec2_to_point(end))))
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
