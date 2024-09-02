// Copyright 2024 Adam Platkevič <rflashster@gmail.com>
//
// SPDX-License-Identifier: MIT

use crate::aabb::AaBb;
use std::collections::HashSet;

pub struct QuadTree<T> {
	bounding_box: AaBb,
	depth: usize,
	inner_node_capacity: usize,
	subtrees: Option<Box<[QuadTree<T>; 4]>>,
	pairs: Vec<(AaBb, T)>,
}

impl<T: Clone> QuadTree<T> {
	pub fn new(bounding_box: AaBb, depth: usize, inner_node_capacity: usize) -> Self {
		QuadTree {
			bounding_box,
			depth,
			inner_node_capacity,
			subtrees: None,
			pairs: Vec::new(),
		}
	}

	pub fn insert(&mut self, bounding_box: AaBb, value: T) -> bool {
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

	pub fn find(&self, bounding_box: &AaBb) -> HashSet<T>
	where
		T: Eq + std::hash::Hash + Clone,
	{
		let mut set = HashSet::new();
		self.find_internal(bounding_box, &mut set);
		set
	}

	fn find_internal(&self, bounding_box: &AaBb, set: &mut HashSet<T>)
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

		let midx = (self.bounding_box.left + self.bounding_box.right) / 2.0;
		let midy = (self.bounding_box.top + self.bounding_box.bottom) / 2.0;

		self.subtrees = Some(Box::new([
			QuadTree::new(
				AaBb {
					top: self.bounding_box.top,
					right: midx,
					bottom: midy,
					left: self.bounding_box.left,
				},
				self.depth - 1,
				self.inner_node_capacity,
			),
			QuadTree::new(
				AaBb {
					top: self.bounding_box.top,
					right: self.bounding_box.right,
					bottom: midy,
					left: midx,
				},
				self.depth - 1,
				self.inner_node_capacity,
			),
			QuadTree::new(
				AaBb {
					top: midy,
					right: midx,
					bottom: self.bounding_box.bottom,
					left: self.bounding_box.left,
				},
				self.depth - 1,
				self.inner_node_capacity,
			),
			QuadTree::new(
				AaBb {
					top: midy,
					right: self.bounding_box.right,
					bottom: self.bounding_box.bottom,
					left: midx,
				},
				self.depth - 1,
				self.inner_node_capacity,
			),
		]));
	}
}
