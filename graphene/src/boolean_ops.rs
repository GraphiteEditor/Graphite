use crate::{
   layers::{
      simple_shape::Shape,
   },
   intersection::{get_intersections, Intersect},
};
use kurbo::{BezPath, Point, PathEl, PathSeg};

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

impl PathGraph{
   pub fn from_paths(alpha: & BezPath, beta: & BezPath, reverse: bool) -> Option<PathGraph>{
      let mut new = PathGraph{edges: Vec::new(), graph: Vec::new()};
      let mut a_seg_idx = 0;
      let mut b_seg_idx = 0;
      let mut sects = get_intersections(alpha, beta);
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

pub fn split_path_seg(p: &PathSeg, t: f64){

}

pub fn boolean_operation(select: BooleanOperation, shapes: Vec<Shape>){}
