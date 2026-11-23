use crate::chains::chain_links;
use crate::contour::{ColumnPool, Contour};
use crate::forest::Forest;
use crate::per_node::PerNode;
use crate::{CHAIN_WIDTH, Graph, NodeIndex, NodeKind, Rect};
use glam::IVec2;
use std::mem;

const GAP: IVec2 = IVec2::new(CHAIN_WIDTH - 5, 0);

pub(crate) fn elongations(graph: &Graph, forest: &Forest) -> PerNode<IVec2> {
	let mut fitter = Fitter {
		graph,
		forest,
		chain_links: chain_links(graph, forest),
		carries_held: carriers_of_held(graph, forest),
		contours: PerNode::from_fn(graph, |_| Contour::default()),
		elongations: PerNode::filled(graph, IVec2::ZERO),
		pool: ColumnPool::default(),
	};
	forest.order.iter().rev().for_each(|node| fitter.fit_upstream(*node));
	fitter.elongations
}

fn carriers_of_held(graph: &Graph, forest: &Forest) -> PerNode<bool> {
	let mut carries_held = PerNode::from_fn(graph, |node| graph[node].held);
	forest.order.iter().rev().for_each(|node| {
		if carries_held[*node]
			&& let Some(owner) = forest.owner(*node)
		{
			carries_held[owner] = true;
		}
	});
	carries_held
}

struct Fitter<'a> {
	graph: &'a Graph,
	forest: &'a Forest,
	chain_links: PerNode<bool>,
	carries_held: PerNode<bool>,
	contours: PerNode<Contour>,
	elongations: PerNode<IVec2>,
	pool: ColumnPool,
}

impl Fitter<'_> {
	fn stays_in_place(&self, node: NodeIndex) -> bool {
		let owner_held = self.forest.owner(node).is_some_and(|owner| self.graph[owner].held);
		self.graph[node].kind == NodeKind::Waypoint || self.chain_links[node] || self.carries_held[node] || owner_held
	}

	fn fit_upstream(&mut self, node: NodeIndex) {
		let forest = self.forest;
		let outline = self.graph[node].outline();
		let mut placed = forest
			.children_from_the_top
			.of(node)
			.iter()
			.fold(Contour::default(), |placed, child| self.fit_child(*child, outline, placed));

		if let Some(outline) = outline {
			placed.cover_rect(outline, &mut self.pool);
		}
		self.contours[node] = placed;
	}

	fn fit_child(&mut self, child: NodeIndex, owner_outline: Option<Rect>, placed: Contour) -> Contour {
		let contour = mem::take(&mut self.contours[child]);
		if self.forest.detached[child] {
			contour.release(&mut self.pool);
			return placed;
		}

		let contour = contour.moved(self.forest.offsets[child]);
		let (contour, elongation) = if self.stays_in_place(child) {
			(contour, IVec2::ZERO)
		} else {
			cleared(contour, &placed, owner_outline, self.forest.feeds_stack(self.graph, child))
		};
		self.elongations[child] = elongation;
		placed.union(contour, &mut self.pool)
	}
}

fn cleared(contour: Contour, placed: &Contour, owner_outline: Option<Rect>, feeds_stack: bool) -> (Contour, IVec2) {
	let from_owner = owner_outline.map_or(IVec2::ZERO, |outline| {
		if feeds_stack {
			IVec2::new(0, contour.clearance_below_rect(outline, GAP))
		} else {
			IVec2::new(-contour.clearance_left_of_rect(outline, GAP), 0)
		}
	});
	let contour = contour.moved(from_owner);
	let from_siblings = IVec2::new(0, contour.clearance_below(placed, GAP));
	(contour.moved(from_siblings), from_owner + from_siblings)
}
