use crate::{
   layers::{
      simple_shape::Shape,
   },
   intersection::{Intersect, Origin, intersections, line_intersect_point},
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

// represents a directional graph with edges "colored" by Origin
// each edge also represents a portion of a visible shape
struct Edge {
   pub from: Origin,
   pub destination: usize,
   pub curve: BezPath,
}

struct Vertex {
   pub intersect: Intersect,
   pub edges: Vec<Edge>,
}

enum Direction {
   CCW,
   CW
}

/// computing a cycle winding direction will be expensive
struct Cycle {
   vertices: Vec<usize>,
   dir: Option<Direction>,
   // tri_ex: [Point; 3] //lower left, upper left, right
}

impl Cycle {
   pub fn new() -> Self {
      Cycle{
         vertices: Vec::new(),
         dir: None,
      }
   }

   /// for purposes of computing direction this function assumes vertices are traversed in order
   pub fn add_vertex(&mut self, graph: &PathGraph, idx: usize){
      self.vertices.push(idx);
      let new_point = graph.intersect(idx).point;
   }
}

/// Optimization: store computed segment bounding boxes, or even edge bounding boxes to prevent recomputation
pub struct PathGraph{
   vertices: Vec<Vertex>,
}

#[allow(dead_code)] //<---- remove this @ release
impl PathGraph{
   pub fn from_paths(alpha: & BezPath, beta: & BezPath, reverse: bool) -> Option<PathGraph>{
      //TODO: check for closed paths somewhere, maybe here?
      let mut new = PathGraph{ vertices: intersections(alpha, beta).into_iter().map(|i| Vertex{intersect: i, edges: Vec::new()}).collect()};
      // we only consider graphs with even numbers of intersections.
      // An odd number of intersections occurrs when either
      //    1. There exists a tangential intersection (which shouldn't effect boolean operations)
      //    2. The algorithm has found an extra intersection or missed an intersection
      if new.size() == 0 || new.size() % 2 != 0 {return None;}
      new.add_edges_from_path(alpha, Origin::Alpha, reverse);
      new.add_edges_from_path(beta, Origin::Beta, reverse);
      Some(new)
   }

   /// Behavior: path should be split at intersection point, not intersection t value, in case of discrepancy between paths
   ///   - implementing this behavior may not be feasible, instead reduce discrepancies
   fn add_edges_from_path(&mut self, path: &BezPath, origin: Origin, reverse: bool) {
      let mut seg_idx = 0;
      //cstart holds the idx of the vertex the current edge is starting from
      let mut cstart = None;
      let mut current = Vec::new();
      // in order to iterate through once, store information for incomplete first edge
      let mut beginning = Vec::new();
      let mut start_idx = None;

      for seg in path.segments(){
         if let Some((next_idx, time)) = self.intersect_at_idx(seg_idx, origin) {
            let (seg1, seg2) = split_path_seg(&seg, time);
            match cstart {
               Some(idx) => {
                  current.push(seg1);
                  self.add_edge(origin, idx, next_idx, current, reverse);
                  cstart = Some(next_idx);
                  current = Vec::new();
                  current.push(seg2);
               }
               None => {
                  cstart = Some(next_idx);
                  start_idx = Some(next_idx);
                  beginning.push(seg1);
                  current.push(seg2);
               }
            }
         }
         else { match cstart {
            Some(_) => current.push(seg),
            None => beginning.push(seg),
         }}
      }
      current.append(&mut beginning); // we assume, but do not check for, a closed path
      self.add_edge(origin, cstart.unwrap(), start_idx.unwrap(), current, reverse);
      seg_idx += 1;
   }

   fn add_edge(&mut self, origin: Origin, vertex: usize, destination: usize, curve: Vec<PathSeg>, reverse: bool){
      let mut new_edge = Edge{from: origin, destination: destination, curve: BezPath::from_path_segments(curve.into_iter())};
      if reverse {
         new_edge.destination = vertex;
         self.vertices[destination].edges.push(new_edge);
      }
      else { self.vertices[vertex].edges.push(new_edge); }
   }

   fn intersect_at_idx(&self, idx: usize, origin: Origin) -> Option<(usize, f64)> {
      self.vertices.iter().enumerate().find_map(|idx, isct| {
         if isct.seg_idx(origin) == idx {
            Some((idx, isct.t_val(origin)))
         }
         else { None }
      })
   }

   // return number of vertices in graph, this is equivalent to the number of intersections
   pub fn size(&self) -> usize {
      return self.vertices.len();

   }

   // return reference to intersect associated with the vertex at idx
   pub fn intersect(&self, idx: usize) -> &Intersect {
      & self.vertices[idx].intersect
   }

   fn get_cycle(&self, cvert: usize, corigin: Origin) -> Cycle {

   }

   pub fn get_cycles(&self) -> Vec<Cycle> {

   }

}

/// This functions assumes t in [0,1], behavior is undefined otherwise
/// Fix: function currently panics when line_intersection returns None, this happens when the quad is flat like a line
/// Check: function may panic in other avoidable scenarios
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
