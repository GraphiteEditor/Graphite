use crate::{
   layers::{
      simple_shape::Shape,
   },
   intersection::{intersections, line_intersect_point},
};
use kurbo::{BezPath, Point, PathEl, PathSeg, ParamCurve, ParamCurveDeriv, Line, QuadBez, CubicBez};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum BooleanOperation{
   Union,
   Difference,
   Intersection,
   SubFront,
   SubBack,
}

pub struct Vertex{
   location: Point,
   edge_id: Vec<usize>,
}
impl Vertex{
   pub fn new(p: Point) -> Self{
      Self{location: p, edge_id: Vec::new()}
   }
}

pub struct PathGraph{
   pub edges: Vec<BezPath>,
   pub graph: Vec<Vertex>,
}

#[allow(dead_code)] //<---- remove this @ release
impl PathGraph{
   pub fn from_paths(alpha: & BezPath, beta: & BezPath, reverse: bool) -> Option<PathGraph>{
      let mut new = PathGraph{edges: Vec::new(), graph: Vec::new()};
      let mut a_seg_idx = 0;
      let mut b_seg_idx = 0;
      let mut sects = intersections(alpha, beta);
      if sects.len() == 0 {return None;}
      let mut current = sects.first().unwrap().point;
      sects.first_mut().unwrap().mark += 1;
      for element in alpha.elements(){
         match element{
            PathEl::MoveTo(p) => {
               current = p.clone();

            }
            PathEl::LineTo(p) => {

            }
            PathEl::QuadTo(p1, p2) => {

            }
            PathEl::CurveTo(p1, p2, p3) => {

            }
            PathEl::ClosePath => {

            }
         }
      };
      None
   }
}

/// This functions assumes t in [0,1], behavior is undefined otherwise
/// FIX: function currently panics when line_intersection returns None, this happens when the quad is flat like a line
pub fn split_path_seg(p: &PathSeg, t: f64) -> (PathSeg, PathSeg) {
   match p{
      PathSeg::Cubic(cubic) => {
         let split = cubic.eval(t);
         let handle = cubic.deriv().eval(t).to_vec2();
         (PathSeg::Cubic(CubicBez{p0: cubic.p0, p1: cubic.p1, p2: split - handle, p3: split}),
          PathSeg::Cubic(CubicBez{p0: split, p1: split + handle, p2: cubic.p2, p3: cubic.p3}))
      }
      PathSeg::Quad(quad) => {
         let split = quad.eval(t);
         let handle = quad.deriv().eval(t).to_vec2();
         let mid1 = line_intersect_point(&Line::new(quad.p0, quad.p1), &Line::new(split, split + handle)).unwrap();
         let mid2 = line_intersect_point(&Line::new(quad.p2, quad.p1), &Line::new(split, split + handle)).unwrap();
         (PathSeg::Quad(QuadBez{p0: quad.p0, p1: mid1, p2: split}), PathSeg::Quad(QuadBez{p0: split, p1: mid2, p2: quad.p2}))
      }
      PathSeg::Line(line) => {
         let split = line.eval(t);
         (PathSeg::Line(Line{p0: line.p0, p1: split}), PathSeg::Line(Line{p0: split, p1: line.p1}))
      }
   }
}

pub fn boolean_operation(select: BooleanOperation, shapes: Vec<Shape>) {}
