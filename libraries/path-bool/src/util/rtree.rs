use glam::DVec2;
use std::cmp::Ordering;

use crate::aabb::{bounding_boxes_overlap, merge_bounding_boxes, Aabb};

pub(crate) struct RTreeNode {
	bbox: Aabb,
	children: Vec<RTreeNode>,
	entries: Vec<(Aabb, usize)>,
	is_leaf: bool,
}

impl RTreeNode {
	fn new(is_leaf: bool) -> Self {
		RTreeNode {
			bbox: Aabb::default(),
			children: Vec::new(),
			entries: Vec::new(),
			is_leaf,
		}
	}

	fn insert(&mut self, bbox: Aabb, index: usize, max_entries: usize) {
		if self.is_leaf {
			self.entries.push((bbox, index));
			self.update_bbox(&bbox);
			if self.entries.len() > max_entries {
				self.split(max_entries);
			}
		} else {
			let best_child = self.choose_subtree(&bbox);
			self.children[best_child].insert(bbox, index, max_entries);
			self.update_bbox(&self.children[best_child].bbox.clone());
		}
	}

	fn choose_subtree(&self, bbox: &Aabb) -> usize {
		self.children
			.iter()
			.enumerate()
			.min_by(|(_, a), (_, b)| {
				let area_increase_a = area(&merge_bounding_boxes(&a.bbox, bbox)) - area(&a.bbox);
				let area_increase_b = area(&merge_bounding_boxes(&b.bbox, bbox)) - area(&b.bbox);
				area_increase_a.partial_cmp(&area_increase_b).unwrap_or(Ordering::Equal)
			})
			.map(|(index, _)| index)
			.unwrap_or(0)
	}

	fn update_bbox(&mut self, bbox: &Aabb) {
		self.bbox = merge_bounding_boxes(&self.bbox, bbox);
	}

	fn split(&mut self, max_entries: usize) {
		if !self.is_leaf {
			return; // We only split leaf nodes in this implementation
		}

		let mut new_node = RTreeNode::new(true);
		let (seed1, seed2) = self.pick_seeds();

		new_node.entries.push(self.entries[seed2].clone());
		new_node.update_bbox(&new_node.entries[0].0.clone());

		self.bbox = self.entries[seed1].0;

		let mut group1 = vec![self.entries[seed1].clone()];
		let mut group2 = vec![self.entries[seed2].clone()];

		let remaining_entries: Vec<_> = self.entries.iter().enumerate().filter(|&(i, _)| i != seed1 && i != seed2).map(|(_, e)| e.clone()).collect();

		for entry in remaining_entries {
			if group1.len() >= max_entries / 2 {
				group2.push(entry);
			} else if group2.len() >= max_entries / 2 {
				group1.push(entry);
			} else {
				let increase1 = area(&merge_bounding_boxes(&self.bbox, &entry.0)) - area(&self.bbox);
				let increase2 = area(&merge_bounding_boxes(&new_node.bbox, &entry.0)) - area(&new_node.bbox);
				if increase1 < increase2 {
					group1.push(entry);
					self.update_bbox(&entry.0);
				} else {
					group2.push(entry);
					new_node.update_bbox(&entry.0);
				}
			}
		}

		self.entries = group1;
		new_node.entries = group2;

		// Create a new parent if this was the root
		if self.children.is_empty() {
			let mut new_parent = RTreeNode::new(false);
			new_parent.children.push(std::mem::replace(self, RTreeNode::new(true)));
			new_parent.children.push(new_node);
			new_parent.update_bbox(&new_parent.children[0].bbox.clone());
			new_parent.update_bbox(&new_parent.children[1].bbox.clone());
			*self = new_parent;
		} else {
			self.children.push(new_node);
		}
	}

	fn pick_seeds(&self) -> (usize, usize) {
		let mut max_waste = f64::NEG_INFINITY;
		let mut seeds = (0, 1);

		for i in 0..self.entries.len() {
			for j in (i + 1)..self.entries.len() {
				let combined_bbox = merge_bounding_boxes(&self.entries[i].0, &self.entries[j].0);
				let waste = area(&combined_bbox) - area(&self.entries[i].0) - area(&self.entries[j].0);

				if waste > max_waste {
					max_waste = waste;
					seeds = (i, j);
				}
			}
		}

		seeds
	}

	fn pick_next(&self, other: &RTreeNode) -> usize {
		self.entries
			.iter()
			.enumerate()
			.max_by(|(_, a), (_, b)| {
				let diff_a = area(&merge_bounding_boxes(&self.bbox, &a.0)) - area(&merge_bounding_boxes(&other.bbox, &a.0));
				let diff_b = area(&merge_bounding_boxes(&self.bbox, &b.0)) - area(&merge_bounding_boxes(&other.bbox, &b.0));
				diff_a.partial_cmp(&diff_b).unwrap_or(Ordering::Equal)
			})
			.map(|(index, _)| index)
			.unwrap_or(0)
	}

	fn assign_entry(&mut self, index: usize, other: &mut RTreeNode) {
		let entry = self.entries.remove(index);
		other.entries.push(entry);
		other.update_bbox(&entry.0);
	}

	fn query(&self, search_bbox: &Aabb, results: &mut Vec<usize>) {
		if !bounding_boxes_overlap(&self.bbox, search_bbox) {
			return;
		}

		if self.is_leaf {
			for (bbox, index) in &self.entries {
				if bounding_boxes_overlap(bbox, search_bbox) {
					results.push(*index);
				}
			}
		} else {
			for child in &self.children {
				child.query(search_bbox, results);
			}
		}
	}
}

pub(crate) struct RTree {
	root: RTreeNode,
	max_entries: usize,
}

impl RTree {
	pub(crate) fn new(max_entries: usize) -> Self {
		RTree {
			root: RTreeNode::new(true),
			max_entries,
		}
	}

	pub(crate) fn insert(&mut self, bbox: Aabb, index: usize) {
		self.root.insert(bbox, index, self.max_entries);
	}

	pub(crate) fn query(&self, search_bbox: &Aabb) -> Vec<usize> {
		let mut results = Vec::new();
		self.root.query(search_bbox, &mut results);
		results
	}
}

// Helper function to calculate the area of an Aabb
fn area(bbox: &Aabb) -> f64 {
	(bbox.right() - bbox.left()) * (bbox.bottom() - bbox.top())
}
