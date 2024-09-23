use crate::aabb::Aabb;
use std::collections::HashSet;

pub struct QuadTree<T> {
	bounding_box: Aabb,
	depth: usize,
	inner_node_capacity: usize,
	subtrees: Option<Box<[QuadTree<T>; 4]>>,
	pairs: Vec<(Aabb, T)>,
}

impl<T: Clone> QuadTree<T> {
	pub fn new(bounding_box: Aabb, depth: usize, inner_node_capacity: usize) -> Self {
		QuadTree {
			bounding_box,
			depth,
			inner_node_capacity,
			subtrees: None,
			pairs: Vec::new(),
		}
	}

	pub fn insert(&mut self, bounding_box: Aabb, value: T) -> bool {
		if !crate::aabb::bounding_boxes_overlap(&bounding_box, &self.bounding_box) {
			return false;
		}

		if self.depth > 0 && self.pairs.len() >= self.inner_node_capacity {
			self.ensure_subtrees();
			for tree in self.subtrees.as_mut().unwrap().iter_mut() {
				tree.insert(bounding_box, value.clone());
			}
		} else {
			self.pairs.push((bounding_box, value));
		}

		true
	}

	pub fn find(&self, bounding_box: &Aabb) -> HashSet<T>
	where
		T: Eq + std::hash::Hash + Clone,
	{
		let mut set = HashSet::new();
		self.find_internal(bounding_box, &mut set);
		set
	}

	fn find_internal(&self, bounding_box: &Aabb, set: &mut HashSet<T>)
	where
		T: Eq + std::hash::Hash + Clone,
	{
		if !crate::aabb::bounding_boxes_overlap(bounding_box, &self.bounding_box) {
			return;
		}

		for (key, value) in &self.pairs {
			if crate::aabb::bounding_boxes_overlap(bounding_box, key) {
				set.insert(value.clone());
			}
		}

		if let Some(subtrees) = &self.subtrees {
			for tree in subtrees.iter() {
				tree.find_internal(bounding_box, set);
			}
		}
	}

	fn ensure_subtrees(&mut self) {
		if self.subtrees.is_some() {
			return;
		}

		let mid_x = (self.bounding_box.left + self.bounding_box.right) / 2.;
		let mid_y = (self.bounding_box.top + self.bounding_box.bottom) / 2.;

		self.subtrees = Some(Box::new([
			QuadTree::new(
				Aabb {
					top: self.bounding_box.top,
					right: mid_x,
					bottom: mid_y,
					left: self.bounding_box.left,
				},
				self.depth - 1,
				self.inner_node_capacity,
			),
			QuadTree::new(
				Aabb {
					top: self.bounding_box.top,
					right: self.bounding_box.right,
					bottom: mid_y,
					left: mid_x,
				},
				self.depth - 1,
				self.inner_node_capacity,
			),
			QuadTree::new(
				Aabb {
					top: mid_y,
					right: mid_x,
					bottom: self.bounding_box.bottom,
					left: self.bounding_box.left,
				},
				self.depth - 1,
				self.inner_node_capacity,
			),
			QuadTree::new(
				Aabb {
					top: mid_y,
					right: self.bounding_box.right,
					bottom: self.bounding_box.bottom,
					left: mid_x,
				},
				self.depth - 1,
				self.inner_node_capacity,
			),
		]));
	}
}
