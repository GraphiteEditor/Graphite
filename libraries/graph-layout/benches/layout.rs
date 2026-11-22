use criterion::BatchSize;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use glam::DVec2;
use glam::IVec2;
use graph_layout::{Drag, GRID_SIZE, Graph, Node, Placement, Wire, layout, target_at};
use std::hint::black_box;

const SIZES: [usize; 3] = [1_000, 10_000, 100_000];

/// Builds a graph of the given node count.
type GraphShape = fn(usize) -> Graph;

/// A deterministic xorshift generator, so every run lays out the same graphs.
struct Random(u64);

impl Random {
	fn below(&mut self, bound: usize) -> usize {
		self.0 ^= self.0 << 13;
		self.0 ^= self.0 >> 7;
		self.0 ^= self.0 << 17;
		(self.0 % bound as u64) as usize
	}
}

fn node(placement: Placement, is_layer: bool) -> Node {
	Node {
		placement,
		is_layer,
		rows: 2,
		width: 5,
		inputs: 3,
		outputs: 1,
		chainable: true,
		waypoint: false,
		held: false,
		name_width: 60.,
		locked: false,
	}
}

fn wire(source: usize, target: usize, input: u32) -> Wire {
	Wire {
		source: source as u32,
		output: 0,
		target: target as u32,
		input,
	}
}

/// One placed root and a single line of relative nodes, each read by the one before it, the deepest a graph can get.
fn line(node_count: usize) -> Graph {
	let mut graph = Graph::default();
	graph.nodes.push(node(Placement::Absolute(IVec2::ZERO), false));
	for index in 1..node_count {
		graph.nodes.push(node(Placement::Relative(IVec2::new(-7, 0)), false));
		graph.wires.push(wire(index, index - 1, 0));
	}
	graph
}

/// Half the nodes are placed roots, all reading one shared node, whose position then spans every one of them.
fn fan_in(node_count: usize) -> Graph {
	let roots = node_count / 2;
	let mut graph = Graph::default();
	for index in 0..roots {
		graph.nodes.push(node(Placement::Absolute(IVec2::new(0, 3 * index as i32)), false));
	}
	let shared = graph.nodes.len();
	graph.nodes.push(node(Placement::Relative(IVec2::new(-7, 0)), false));
	for root in 0..roots {
		graph.wires.push(wire(shared, root, 1));
	}
	for index in graph.nodes.len()..node_count {
		graph.nodes.push(node(Placement::Relative(IVec2::new(-7, 0)), false));
		graph.wires.push(wire(index, index - 1, 0));
	}
	graph
}

/// Layers stacked on each other, each with a chain of three nodes, as a long Layers panel gives.
fn stacked_layers(node_count: usize) -> Graph {
	let mut graph = Graph::default();
	let mut previous_layer = None;
	while graph.nodes.len() + 4 <= node_count {
		let layer = graph.nodes.len();
		let placement = if previous_layer.is_some() { Placement::Stack(0) } else { Placement::Absolute(IVec2::ZERO) };
		graph.nodes.push(node(placement, true));
		if let Some(previous_layer) = previous_layer {
			graph.wires.push(wire(layer, previous_layer, 0));
		}

		let mut reader = (layer, 1);
		for _ in 0..3 {
			let chained = graph.nodes.len();
			graph.nodes.push(node(Placement::Chain, false));
			graph.wires.push(wire(chained, reader.0, reader.1));
			reader = (chained, 0);
		}
		previous_layer = Some(layer);
	}
	graph
}

/// Each node read by one to three random nodes added before it, mostly relative, with a few placed roots.
fn random_graph(node_count: usize) -> Graph {
	let mut random = Random(0x9E37_79B9_7F4A_7C15);
	let mut graph = Graph::default();
	for index in 0..node_count {
		if index == 0 || random.below(100) == 0 {
			graph.nodes.push(node(Placement::Absolute(IVec2::new(random.below(1_000) as i32, random.below(1_000) as i32)), false));
			continue;
		}

		graph.nodes.push(node(Placement::Relative(IVec2::new(-7, random.below(7) as i32)), false));
		for input in 0..1 + random.below(3) {
			graph.wires.push(wire(index, random.below(index), input as u32));
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
			group.bench_with_input(BenchmarkId::new(name, size), &graph, |bencher, graph| bencher.iter(|| layout(black_box(graph))));
		}
	}
	group.finish();
}

fn bench_hit_test(criterion: &mut Criterion) {
	let mut group = criterion.benchmark_group("node_at");
	for size in SIZES {
		let graph_layout = layout(&random_graph(size));
		let point = IVec2::new(500, 500);
		group.bench_with_input(BenchmarkId::from_parameter(size), &graph_layout, |bencher, graph_layout| {
			bencher.iter(|| graph_layout.node_at(black_box(point)))
		});
	}
	group.finish();
}

fn bench_click_targets(criterion: &mut Criterion) {
	let mut group = criterion.benchmark_group("target_at");
	for size in SIZES {
		let graph = stacked_layers(size);
		let graph_layout = layout(&graph);

		// The middle of a chained node halfway down the stack, and a point beside the whole graph that every node's reach misses
		let middle_node = graph_layout.positions[size / 2 + 1].as_dvec2() * GRID_SIZE + DVec2::new(60., 30.);
		let beside = DVec2::new(1e6, 0.);

		for (name, point) in [("hit", middle_node), ("miss", beside)] {
			group.bench_with_input(BenchmarkId::new(name, size), &(&graph, &graph_layout), |bencher, (graph, graph_layout)| {
				bencher.iter(|| target_at(black_box(graph), black_box(graph_layout), black_box(point)))
			});
		}
	}
	group.finish();
}

/// Drags the bottom layer of a stack up, where each row it moves carries the stack above it, as one pointer move of a drag
/// that has already come 40 rows: one row further, back to a row it passed, and sideways from where it is.
fn bench_drag(criterion: &mut Criterion) {
	let mut group = criterion.benchmark_group("drag_pointer_move");
	for size in [1_000, 10_000] {
		let graph = stacked_layers(size);
		let bottom_layer = graph.nodes.iter().rposition(|node| node.is_layer).unwrap_or_default() as u32;
		let mut warm = Drag::new(&graph, vec![bottom_layer], false, layout);
		warm.moved(IVec2::new(0, -40));

		for (name, offset) in [("one_row_further", IVec2::new(0, -41)), ("back", IVec2::new(0, -20)), ("sideways", IVec2::new(5, -40))] {
			group.bench_function(BenchmarkId::new(name, size), |bencher| {
				bencher.iter_batched(
					|| warm.clone(),
					|mut drag| {
						let moved = drag.moved(black_box(offset));
						(drag, moved)
					},
					BatchSize::LargeInput,
				)
			});
		}
	}
	group.finish();
}

criterion_group!(benches, bench_layout, bench_hit_test, bench_click_targets, bench_drag);
criterion_main!(benches);
