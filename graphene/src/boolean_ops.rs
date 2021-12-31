use crate::{
   layers::{
      simple_shape::Shape,
      style::{PathStyle, Stroke, Fill},
   },
   color::Color,
   intersection::{Intersect, F64PRECISION, Origin, intersections, line_intersect_point},
};
use kurbo::{BezPath, Point, PathEl, Rect, PathSeg, ParamCurve, ParamCurveArclen, ParamCurveExtrema, ParamCurveDeriv, Line, QuadBez, CubicBez};
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

// "!" operator reverses direction
#[derive(PartialEq, Eq, Clone,)]
enum Direction {
   CCW = 1,
   CW = 0,
}

/// TODO: computing a cycle direction and cycle length is expensive, find ways to optimize/avoid/
/// TODO: test edge cases of direction algorithm
/// Behavior: Intersection and Union cases are distinuguished between by cycle length
///   - This only effects shapes whose intersection is a single shape, and the intersection is similalarly sized to the union
///   - can be solved by first computing at low accuracy, and if the values are close recomputing.
struct Cycle {
   vertices: Vec<usize>,
   dir: Option<Direction>,
   ex: [Option<(f64, usize)>; 4], //lowest, leftest, highest, rightest
   time: usize, //(most recent, prior)
   length: Option<f64>,
}

impl Cycle {
   pub fn new(start: usize) -> Self {
      Cycle{
         vertices: vec![start],
         dir: None,
         ex: [None, None, None, None],
         time: 1,
         length: None,
      }
   }

   pub fn new_measured(start: usize) -> Self {
      let mut cycle = Cycle::new(start);
      cycle.length = 0.0;
      cycle
   }

   /// returns true when the cycle is complete, a cycle is complete when it revisits its first vertex
   /// for purposes of computing direction this function assumes vertices are traversed in order
   pub fn extend(&mut self, graph: &PathGraph, vertex: usize, edge_curve: &BezPath) -> bool {
      if vertex == self.vertices[0] { return true; }
      self.vertices.push(vertex);
      self.update_ex(bounding_box(edge_curve));
      if let Some(ref mut cycle_length) = self.length {
         //TODO: the accuracy value below should be replaced with a more carefully chosen one
         //    - could dynamically adjust value by looking at the manhatten distance between the segments end points,
         //    or maybe the bounding rectangle area,
         *cycle_length += path_length(edge_curve, Some(0.0001));
      }
      return false;
   }

   pub fn vertices(&self) -> &Vec<usize> {
      &self.vertices
   }

   fn update_ex(&mut self, bound: Rect) {
      self.update_extreme(0, bound.y0, |a, b| a <= b);
      self.update_extreme(1, bound.x0, |a, b| a <= b);
      self.update_extreme(2, bound.y1, |a, b| a >= b);
      self.update_extreme(3, bound.x1, |a, b| a >= b);
   }

   fn update_extreme<F: Fn(f64, f64) -> bool>(&mut self, old: usize, new: f64, comp: F) {
      if let Some((val, _)) = self.ex[old] {
         if comp(new, val) {
            self.ex[old] = Some((new, self.time));
            self.time += 1;
         }
      }
      else {
         self.ex[old] = Some((new, self.time));
         self.time += 1;
      }
   }

   fn compute_direction(& self) -> Result<Direction, ()> {
      let mut cw = true;
      let mut ccw = true;
      let mut prev_time = 0;

      self.ex.iter()
         .map(|o| o.unwrap()) //All values should be Some()
         .skip_while(|(_, time)| *time != self.time - 1) //find most recent
         .cycle()
         .take_while(|(_, time)| {
            if *time == self.time - 1 { return false; }
            if prev_time != 0 && *time > prev_time { ccw = false; }
            if prev_time != 0 && *time < prev_time { cw = false; }
            prev_time = *time;
            return true;
         });

      match (ccw, cw) {
         (true, false) => Ok(Direction::CCW),
         (false, true) => Ok(Direction::CW),
         _ => Err(()),
      }
   }

   /// - if the path has multiple sub paths the function Errs
   /// - if the path crosses itself the computed direction may be (probably will be) wrong
   /// - the path does not need to end in a ClosePath, however if it doesn't, the final vertex must compare exactly equal to the start vertex.
   ///   Which, with floating point precision, is unlikely.
   pub fn compute_direction_for_path(path: &BezPath) -> Result<Direction, ()> {
      let mut cycle = None;
      let mut start = Point::new(0.0, 0.0);
      let mut current = Point::new(0.0, 0.0); //this value should never be read
      for element in path.iter(){
         match element {
            PathEl::MoveTo(p0) => {
               if let Some(_) = cycle { return Err(()); }
               else {cycle = Some(Cycle::new(0)); current = p0; start = p0;}
            }
            PathEl::LineTo(p0) | PathEl::QuadTo(_, p0) | PathEl::CurveTo(.., p0) => {
               // cycle should never be None as all valid paths begin with a MoveTo
               cycle.unwrap().update_ex(Rect::from((current, p0)));
               current = p0;
            }
            PathEl::ClosePath => {
               cycle.unwrap().update_ex(Rect::from((current, start)));
               current = start;
            }
         }
      }
      if current != start { return Err(()); }
      if let Some(c) = cycle { return c.compute_direction(); }
      Err(())
   }
}

/// Optimization: store computed segment bounding boxes, or even edge bounding boxes to prevent recomputation
pub struct PathGraph{
   vertices: Vec<Vertex>,
}

/// Boolean Operation Algorithm
///   - Behavior: Has somewhat undefined behavior when shapes have self intersections
#[allow(dead_code)] //<---- remove this @ release
impl PathGraph{
   pub fn from_paths(alpha: & BezPath, beta: & BezPath, reverse: bool) -> Option<PathGraph> {
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
         seg_idx += 1;
      }
      current.append(&mut beginning); // we assume, but do not check for, a closed path
      self.add_edge(origin, cstart.unwrap(), start_idx.unwrap(), current, reverse);
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
      self.vertices.iter().enumerate().find_map(|(idx, vertex)| {
         if vertex.intersect.seg_idx(origin) == idx {
            Some((idx, vertex.intersect.t_val(origin)))
         }
         else { None }
      })
   }

   // return number of vertices in graph, this is equivalent to the number of intersections
   pub fn size(&self) -> usize {
      return self.vertices.len();

   }

   pub fn vertex(&self, idx: usize) -> &Vertex {
      & self.vertices[idx]
   }

   pub fn edge(&self, from: usize, to: usize) -> Option<&Edge> {
      // with a data strucutre restructure, or a hashmap, the find here could be avoided
      // but it probably has a miniaml performance impact
      self.vertex(from).edges.iter().find(|edge| edge.destination == to)
   }

   // return reference to intersect associated with the vertex at idx
   pub fn intersect(&self, idx: usize) -> &Intersect {
      & self.vertices[idx].intersect
   }

   /// where a valid cycle alternates edge Origin
   /// cvert: the current vertex, or the last vertex added to cycle
   /// state: the Origin of the last edge
   fn get_cycle(&self, cycle: &mut Cycle, cvert: usize, state: Origin, measure: bool) {
      // a properly constructed path graph will have exactly one edge at each vertex of each Origin
      let next_edge = self.vertex(cvert).edges.iter().find(|edge| edge.from != state).unwrap();
      if !cycle.extend(self, next_edge.destination, &next_edge.curve) {
         return self.get_cycle(cycle, next_edge.destination, !state, measure);
      }
   }

   pub fn get_cycles(&self, measure: bool) -> Vec<Cycle> {
      let cycles = Vec::new();
      self.vertices.iter().enumerate()
         .for_each(|(vertex_idx, _vertex)| {
            let mut temp = if measure {Cycle::new_measured(vertex_idx)} else {Cycle::new(vertex_idx)};
            self.get_cycle(&mut temp, vertex_idx, Origin::Alpha, measure);
            cycles.push(temp);
            temp = if measure {Cycle::new_measured(vertex_idx)} else {Cycle::new(vertex_idx)};
            self.get_cycle(&mut temp, vertex_idx, Origin::Beta, measure);
            cycles.push(temp);
         });
      cycles
   }

   pub fn get_shape(&self, cycle: &Cycle) -> Shape {
      let mut path = BezPath::new();
      let vertices = cycle.vertices();
      for idx in 1..vertices.len() {
         // we expect the cycle to be valid, this should not panic
         concat_paths(&mut path, &self.edge(vertices[idx - 1], vertices[idx]).unwrap().curve);
      }
      Shape::from_bez_path(path, PathStyle::new(Some(Stroke::new(Color::BLACK, 1.0)), Some(Fill::none())), false)
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

/// TODO: when a boolean operation fails that should be reported with a specific message in the returned result
///   - Several function which return Err(()) should return a more specific error code
///   - The error message should then be displayed to the user.
///   - there are situations where it may not be obvious why the operation failed, a path that looks closed but actually isn't for example
/// TODO: For the Union and intersection operations, what should the new Fill and Stroke be?
pub fn boolean_operation(select: BooleanOperation, alpha: Shape, beta: Shape) -> Result<Vec<Shape>, ()> {
   let alpha = alpha.path;
   let beta = beta.path;
   if alpha.is_empty() || beta.is_empty() { return Err(()); }
   let alpha_dir = Cycle::compute_direction_for_path(&alpha)?;
   let beta_dir = Cycle::compute_direction_for_path(&beta)?;
   match select {
      BooleanOperation::Union => {
         let graph = PathGraph::from_paths(&alpha, &beta, alpha_dir != beta_dir).ok_or(())?;
         collect_shapes(&graph, graph.get_cycles(true), alpha_dir)
      }
      BooleanOperation::Difference => {
         let graph = PathGraph::from_paths(&alpha, &beta, alpha_dir == beta_dir).ok_or(())?;
         Err(()) //not yet implemented
      }
      BooleanOperation::Intersection => {
         let graph = PathGraph::from_paths(&alpha, &beta, alpha_dir != beta_dir).ok_or(())?;
         Err(()) //not yet implemented
      }
      BooleanOperation::SubBack => {
         let graph = PathGraph::from_paths(&alpha, &beta, alpha_dir == beta_dir).ok_or(())?;
         collect_shapes(&graph, graph.get_cycles(false), alpha_dir)
      }
      BooleanOperation::SubFront => {
         let graph = PathGraph::from_paths(&alpha, &beta, alpha_dir == beta_dir).ok_or(())?;
         collect_shapes(&graph, graph.get_cycles(false), beta_dir)
      }
   }
}

/// panics if the curve has no PathSeg's
pub fn bounding_box(curve: &BezPath) -> Rect {
   curve.segments().map(|seg| <PathSeg as ParamCurveExtrema>::bounding_box(&seg) ).reduce(|bounds, rect| bounds.union(rect)).unwrap()
}

pub fn collect_shapes(graph: &PathGraph, cycles: Vec<Cycle>, direction: Direction) -> Result<Vec<Shape>, ()> {
   let mut shapes = Vec::new();
   for ref cycle in cycles {
      if let Ok(dir) = cycle.compute_direction() {
         if dir == direction { shapes.push(graph.get_shape(cycle)); }
         else { return Err(()); }
      }
      else { return Err(()); }
   }
   Ok(shapes)
}

pub fn is_closed(curve: &BezPath) -> bool {
   curve.iter().last() == Some(PathEl::ClosePath)
}

pub fn concat_paths(a: &mut BezPath, b: &BezPath) {
   b.iter().for_each(|element| a.push(element));
}

pub fn path_length(a: &BezPath, accuracy: Option<f64>) -> f64 {
   let mut sum = 0.0;
   //computing arclen with F64PRECISION accuracy is probably ridiculous
   match accuracy {
      Some(val) => a.segments().for_each(|seg| sum += seg.arclen(val)),
      None => a.segments().for_each(|seg| sum += seg.arclen(F64PRECISION)),
   }
   sum
}
