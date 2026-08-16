use crate::vector::misc::dvec2_to_point;
use core_types::math::quad::Quad;
use kurbo::{Line, PathSeg};

pub trait QuadExt {
	fn to_lines(&self) -> impl Iterator<Item = PathSeg>;
}

impl QuadExt for Quad {
	fn to_lines(&self) -> impl Iterator<Item = PathSeg> {
		self.all_edges().into_iter().map(|[start, end]| PathSeg::Line(Line::new(dvec2_to_point(start), dvec2_to_point(end))))
	}
}
