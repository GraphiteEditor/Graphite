// Copyright 2024 Adam Platkevič <rflashster@gmail.com>
//
// SPDX-License-Identifier: MIT

use crate::aabb::AABB;
use crate::line_segment_aabb::line_segment_aabb_intersect;
use crate::vector::Vector;
use std::collections::HashSet;

pub struct QuadTree<T> {
    bounding_box: AABB,
    depth: usize,
    inner_node_capacity: usize,
    subtrees: Option<Box<[QuadTree<T>; 4]>>,
    pairs: Vec<(AABB, T)>,
}

impl<T: Clone> QuadTree<T> {
    pub fn from_pairs(pairs: &[(AABB, T)], depth: usize, inner_node_capacity: usize) -> Self {
        if pairs.is_empty() {
            panic!("QuadTree::from_pairs: at least one pair needed.");
        }

        let mut bounding_box = pairs[0].0;
        for (key, _) in pairs.iter().skip(1) {
            bounding_box = crate::aabb::merge_bounding_boxes(Some(bounding_box), key);
        }

        let mut tree = QuadTree::new(bounding_box, depth, inner_node_capacity);

        for (key, value) in pairs {
            tree.insert(*key, value.clone());
        }

        tree
    }

    pub fn new(bounding_box: AABB, depth: usize, inner_node_capacity: usize) -> Self {
        QuadTree {
            bounding_box,
            depth,
            inner_node_capacity,
            subtrees: None,
            pairs: Vec::new(),
        }
    }

    pub fn insert(&mut self, bounding_box: AABB, value: T) -> bool {
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

    pub fn find(&self, bounding_box: &AABB) -> HashSet<T>
    where
        T: Eq + std::hash::Hash + Clone,
    {
        let mut set = HashSet::new();
        self.find_internal(bounding_box, &mut set);
        set
    }

    fn find_internal(&self, bounding_box: &AABB, set: &mut HashSet<T>)
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

    pub fn find_on_line_segment(&self, seg: [Vector; 2]) -> HashSet<T>
    where
        T: Eq + std::hash::Hash + Clone,
    {
        let mut set = HashSet::new();
        self.find_on_line_segment_internal(seg, &mut set);
        set
    }

    fn find_on_line_segment_internal(&self, seg: [Vector; 2], set: &mut HashSet<T>)
    where
        T: Eq + std::hash::Hash + Clone,
    {
        if !line_segment_aabb_intersect(seg, &self.bounding_box) {
            return;
        }

        for (key, value) in &self.pairs {
            if line_segment_aabb_intersect(seg, key) {
                set.insert(value.clone());
            }
        }

        if let Some(subtrees) = &self.subtrees {
            for tree in subtrees.iter() {
                tree.find_on_line_segment_internal(seg, set);
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
                AABB {
                    top: self.bounding_box.top,
                    right: midx,
                    bottom: midy,
                    left: self.bounding_box.left,
                },
                self.depth - 1,
                self.inner_node_capacity,
            ),
            QuadTree::new(
                AABB {
                    top: self.bounding_box.top,
                    right: self.bounding_box.right,
                    bottom: midy,
                    left: midx,
                },
                self.depth - 1,
                self.inner_node_capacity,
            ),
            QuadTree::new(
                AABB {
                    top: midy,
                    right: midx,
                    bottom: self.bounding_box.bottom,
                    left: self.bounding_box.left,
                },
                self.depth - 1,
                self.inner_node_capacity,
            ),
            QuadTree::new(
                AABB {
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
