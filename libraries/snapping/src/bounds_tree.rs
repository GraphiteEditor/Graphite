use crate::bounds::Bounds;

/// The most items a leaf holds before it is split.
const LEAF_SIZE: usize = 8;

/// A tree of nested boxes over a fixed set of items, for finding the ones that overlap an area without visiting them all.
#[derive(Clone, Debug, Default)]
pub(crate) struct BoundsTree {
	nodes: Vec<Node>,
	/// Item indices, ordered so that every node covers one contiguous run of them.
	items: Vec<u32>,
	item_bounds: Vec<Bounds>,
}

#[derive(Clone, Copy, Debug)]
struct Node {
	bounds: Bounds,
	/// For a leaf, where its run of items begins. For a branch, the index of its second child, the first being the next node.
	start_or_second_child: u32,
	/// How many items a leaf holds, or zero for a branch.
	count: u32,
}

impl BoundsTree {
	/// Builds a tree over items given by their boxes, which must not be NaN, with [`Bounds::EMPTY`] for an item never to be found. O(n log n).
	pub(crate) fn build(item_bounds: &[Bounds]) -> Self {
		let mut tree = Self {
			nodes: Vec::with_capacity(item_bounds.len() / LEAF_SIZE * 2 + 1),
			items: (0..item_bounds.len() as u32).collect(),
			item_bounds: item_bounds.to_vec(),
		};

		if !item_bounds.is_empty() {
			tree.build_node(item_bounds, 0, item_bounds.len());
		}
		tree
	}

	fn build_node(&mut self, item_bounds: &[Bounds], start: usize, end: usize) -> u32 {
		let run = &mut self.items[start..end];
		let bounds = run.iter().fold(Bounds::EMPTY, |bounds, &item| bounds.union(&item_bounds[item as usize]));
		let centers = run
			.iter()
			.map(|&item| item_bounds[item as usize].center())
			.filter(|center| center.is_finite())
			.fold(Bounds::EMPTY, |centers, center| centers.including(center));

		let node_index = self.nodes.len() as u32;
		self.nodes.push(Node {
			bounds,
			start_or_second_child: start as u32,
			count: (end - start) as u32,
		});

		// Items with coinciding centers cannot be told apart by splitting, so they stay together in one leaf
		let spread = centers.size();
		let can_be_split = spread.max_element() > 0.;
		if end - start <= LEAF_SIZE || !can_be_split {
			return node_index;
		}

		// Halve the run about the median center along the axis the centers spread furthest on
		let axis = if spread.x >= spread.y { 0 } else { 1 };
		let middle = (end - start) / 2;
		run.select_nth_unstable_by(middle, |&a, &b| item_bounds[a as usize].center()[axis].total_cmp(&item_bounds[b as usize].center()[axis]));

		self.build_node(item_bounds, start, start + middle);
		let second_child = self.build_node(item_bounds, start + middle, end);

		self.nodes[node_index as usize].start_or_second_child = second_child;
		self.nodes[node_index as usize].count = 0;
		node_index
	}

	/// Visits the index of every item with a box overlapping an area. O(log n + matches) for areas that are small beside the whole.
	pub(crate) fn overlapping(&self, area: &Bounds, mut visit: impl FnMut(u32)) {
		if !self.nodes.is_empty() {
			self.overlapping_below(0, area, &mut visit);
		}
	}

	/// Building halves every run, which keeps the tree and so this recursion to a few dozen levels at most.
	fn overlapping_below(&self, node_index: u32, area: &Bounds, visit: &mut impl FnMut(u32)) {
		let node = self.nodes[node_index as usize];
		if !node.bounds.intersects(area) {
			return;
		}

		if node.count > 0 {
			let run = node.start_or_second_child as usize..(node.start_or_second_child + node.count) as usize;
			let overlapping = self.items[run].iter().filter(|&&item| self.item_bounds[item as usize].intersects(area));
			overlapping.for_each(|&item| visit(item));
		} else {
			self.overlapping_below(node_index + 1, area, visit);
			self.overlapping_below(node.start_or_second_child, area, visit);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use glam::DVec2;

	/// A small deterministic generator of numbers from 0 up to 1.
	fn pseudo_random(state: &mut u64) -> f64 {
		*state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
		(*state >> 11) as f64 / (1_u64 << 53) as f64
	}

	fn overlapping(tree: &BoundsTree, area: &Bounds) -> Vec<u32> {
		let mut found = Vec::new();
		tree.overlapping(area, |item| found.push(item));
		found.sort();
		found
	}

	#[test]
	fn an_empty_tree_finds_nothing() {
		let tree = BoundsTree::build(&[]);
		assert!(overlapping(&tree, &Bounds::EVERYTHING).is_empty());
	}

	#[test]
	fn queries_match_a_check_of_every_item() {
		let mut state = 7;
		let mut random = || pseudo_random(&mut state);

		let item_bounds: Vec<Bounds> = (0..3000)
			.map(|_| {
				let corner = DVec2::new(random(), random()) * 1000.;
				let size = DVec2::new(random(), random()) * 20.;
				Bounds::from_points([corner, corner + size])
			})
			.collect();
		let tree = BoundsTree::build(&item_bounds);

		for _ in 0..200 {
			let corner = DVec2::new(random(), random()) * 1000.;
			let area = Bounds::from_points([corner, corner + DVec2::new(random(), random()) * 100.]);

			let expected: Vec<u32> = (0..item_bounds.len() as u32).filter(|&item| item_bounds[item as usize].intersects(&area)).collect();
			assert_eq!(overlapping(&tree, &area), expected);
		}

		assert_eq!(overlapping(&tree, &Bounds::EVERYTHING).len(), item_bounds.len());
	}

	#[test]
	fn items_in_the_same_place_are_all_found() {
		let item_bounds = vec![Bounds::from_point(DVec2::new(5., 5.)); 100];
		let tree = BoundsTree::build(&item_bounds);

		assert_eq!(overlapping(&tree, &Bounds::from_points([DVec2::ZERO, DVec2::splat(10.)])).len(), 100);
		assert!(overlapping(&tree, &Bounds::from_points([DVec2::splat(6.), DVec2::splat(10.)])).is_empty());
	}

	#[test]
	fn items_with_empty_boxes_are_never_found_and_leave_the_rest_split() {
		let item_bounds: Vec<Bounds> = (0..300)
			.map(|index| {
				if index % 3 == 0 || index == 299 {
					Bounds::EMPTY
				} else {
					Bounds::from_point(DVec2::new(index as f64, 0.))
				}
			})
			.collect();
		let tree = BoundsTree::build(&item_bounds);

		assert!(tree.nodes.len() > 1);
		let area = Bounds::from_points([DVec2::new(10., -1.), DVec2::new(20., 1.)]);
		assert_eq!(overlapping(&tree, &area), vec![10, 11, 13, 14, 16, 17, 19, 20]);
		assert_eq!(overlapping(&tree, &Bounds::from_points([DVec2::splat(-1e9), DVec2::splat(1e9)])).len(), 199);
	}

	#[test]
	fn a_small_area_in_a_large_lattice_of_points_finds_just_the_points_inside() {
		let side = 300;
		let item_bounds: Vec<Bounds> = (0..side * side).map(|index| Bounds::from_point(DVec2::new((index % side) as f64, (index / side) as f64))).collect();
		let tree = BoundsTree::build(&item_bounds);

		let area = Bounds::from_points([DVec2::new(100.2, 100.2), DVec2::new(102.8, 102.8)]);
		assert_eq!(overlapping(&tree, &area).len(), 4);
	}
}
