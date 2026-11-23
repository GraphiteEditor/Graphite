use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use glam::IVec2;
use graph_layout::{Drag, Graph, Layout, Node, NodeIndex, NodeKind, Placement, Wire};
use std::hint::black_box;

const SIZES: [u32; 3] = [1_000, 10_000, 100_000];
const BESIDE: IVec2 = IVec2::new(-7, 0);
const STACKED: IVec2 = IVec2::new(0, 3);

type GraphShape = fn(u32) -> Graph;

struct Random(u64);

impl Random {
	fn below(&mut self, bound: u32) -> u32 {
		self.0 ^= self.0 << 13;
		self.0 ^= self.0 >> 7;
		self.0 ^= self.0 << 17;
		u32::try_from(self.0 % u64::from(bound)).unwrap_or_default()
	}
}

fn node(placement: Placement) -> Node {
	Node {
		placement,
		kind: NodeKind::Node,
		size: IVec2::new(5, 3),
		chainable: true,
		held: false,
	}
}

fn layer(placement: Placement) -> Node {
	Node {
		kind: NodeKind::Layer,
		size: IVec2::new(5, 2),
		..node(placement)
	}
}

fn wire(source: u32, target: u32, input: u32) -> Wire {
	Wire {
		source: source.into(),
		output: 0,
		target: target.into(),
		input,
	}
}

fn line(node_count: u32) -> Graph {
	Graph {
		nodes: (0..node_count)
			.map(|index| node(if index == 0 { Placement::Absolute(IVec2::ZERO) } else { Placement::Relative(BESIDE) }))
			.collect(),
		wires: (1..node_count).map(|index| wire(index, index - 1, 0)).collect(),
	}
}

fn fan_in(node_count: u32) -> Graph {
	let roots = node_count / 2;
	let root_nodes = (0..roots).map(|root| node(Placement::Absolute(IVec2::new(0, 3 * root.cast_signed()))));
	let line_nodes = (roots..node_count).map(|_| node(Placement::Relative(BESIDE)));
	let shared_wires = (0..roots).map(|root| wire(roots, root, 1));
	let line_wires = (roots + 1..node_count).map(|index| wire(index, index - 1, 0));

	Graph {
		nodes: root_nodes.chain(line_nodes).collect(),
		wires: shared_wires.chain(line_wires).collect(),
	}
}

fn stacked_layers(node_count: u32) -> Graph {
	let layers = node_count / 4;
	let stacked_layer = |index: u32| layer(if index == 0 { Placement::Absolute(IVec2::ZERO) } else { Placement::Relative(STACKED) });
	let chain = |_| [node(Placement::Relative(BESIDE)), node(Placement::Relative(BESIDE)), node(Placement::Relative(BESIDE))];
	let stack_wires = |index: u32| (index > 0).then(|| wire(4 * index, 4 * (index - 1), 0));
	let chain_wires = |index: u32| [wire(4 * index + 1, 4 * index, 1), wire(4 * index + 2, 4 * index + 1, 0), wire(4 * index + 3, 4 * index + 2, 0)];

	Graph {
		nodes: (0..layers).flat_map(|index| std::iter::once(stacked_layer(index)).chain(chain(index))).collect(),
		wires: (0..layers).flat_map(|index| stack_wires(index).into_iter().chain(chain_wires(index))).collect(),
	}
}

fn random_graph(node_count: u32) -> Graph {
	let mut random = Random(0x9E37_79B9_7F4A_7C15);
	let mut graph = Graph::default();
	for index in 0..node_count {
		if index == 0 || random.below(100) == 0 {
			let position = IVec2::new(random.below(1_000).cast_signed(), random.below(1_000).cast_signed());
			graph.nodes.push(node(Placement::Absolute(position)));
			continue;
		}

		graph.nodes.push(node(Placement::Relative(IVec2::new(-7, random.below(7).cast_signed()))));
		for input in 0..=random.below(3) {
			graph.wires.push(wire(index, random.below(index), input));
		}
	}
	graph
}

fn bench_layout(criterion: &mut Criterion) {
	let shapes: [(&str, GraphShape); 4] = [("line", line), ("fan_in", fan_in), ("stacked_layers", stacked_layers), ("random", random_graph)];

	let mut group = criterion.benchmark_group("layout");
	for (name, shape) in shapes {
		for size in SIZES {
			let graph = shape(size);
			group.bench_with_input(BenchmarkId::new(name, size), &graph, |bencher, graph| bencher.iter(|| Layout::new(black_box(graph))));
		}
	}
	group.finish();
}

fn bench_drag(criterion: &mut Criterion) {
	let mut group = criterion.benchmark_group("drag_pointer_move");
	for size in [1_000, 10_000] {
		let graph = stacked_layers(size);
		let bottom_layer = NodeIndex::from(4 * (size / 4 - 1));
		let mut warm = Drag::new(graph.clone(), Layout::new(&graph), vec![bottom_layer], false);
		let _ = warm.moved(IVec2::new(0, -40));

		for (name, offset) in [("one_row_further", IVec2::new(0, -41)), ("back", IVec2::new(0, -20)), ("sideways", IVec2::new(5, -40))] {
			group.bench_function(BenchmarkId::new(name, size), |bencher| {
				bencher.iter_batched(
					|| warm.clone(),
					|mut drag| {
						let moved = drag.moved(black_box(offset));
						(drag, moved)
					},
					BatchSize::LargeInput,
				);
			});
		}
	}
	group.finish();
}

criterion_group!(benches, bench_layout, bench_drag);
criterion_main!(benches);
