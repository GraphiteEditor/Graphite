use super::test_support::open_demo;
use glam::IVec2;
use graph_layout::analysis::{Discontinuity, DragField, misses, optimal_elongations};
use graph_layout::{Drag, Fitting, Graph, Layout, layout, layout_as_stored, layout_with, layout_with_elongations};
use std::cmp::Reverse;
use std::fmt::Write;

const CELL_SIZE: i32 = 10;

/// Both ways the editor lays out a network, named for file names and reports.
/// A way of laying out a graph, named for file names and reports.
type NamedLayout = (&'static str, fn(&Graph) -> Layout);

const LAYOUT_MODES: [NamedLayout; 2] = [("fitted", layout), ("as-stored", layout_as_stored)];

/// A demo's root network as the layout pass reads it, with the name of each node.
struct DemoGraph {
	demo: String,
	graph: Graph,
	node_names: Vec<String>,
}

impl DemoGraph {
	fn open(demo: &str) -> Self {
		let document = open_demo(demo);
		let interface = &document.network_interface;
		let Some((node_ids, _, graph)) = interface.layout_graph(&[]) else {
			panic!("{demo} has a root network");
		};
		let node_names = node_ids.iter().map(|node_id| format!("{} ({node_id})", interface.display_name(node_id, &[]))).collect();
		Self {
			demo: demo.trim_end_matches(".graphite").to_string(),
			graph,
			node_names,
		}
	}

	fn node_labeled(&self, label: &str) -> u32 {
		let Some(index) = self.node_names.iter().position(|node_name| node_name == label) else {
			panic!("{} has a node labeled {label}", self.demo);
		};
		index as u32
	}

	fn node(&self, name: &str) -> u32 {
		let Some(index) = self.node_names.iter().position(|node_name| node_name.starts_with(&format!("{name} ("))) else {
			panic!("{} has a node named {name}", self.demo);
		};
		index as u32
	}
}

fn output_directory() -> std::path::PathBuf {
	let directory = std::path::PathBuf::from(std::env::var("DRAG_STABILITY_OUT").unwrap_or_else(|_| "../target/drag-stability".into()));
	std::fs::create_dir_all(&directory).expect("the output directory can be created");
	directory
}

/// Writes a heat map of how the layout of a demo's root network changes while `node_name` is dragged by every offset from `min` to `max`, in each layout mode.
/// Each cell is one offset, colored by how far the other nodes are bent from either staying or following the drag since its start,
/// and a red line between two cells marks a jump in the layout between those adjacent offsets.
fn write_drag_stability_maps(demo: &str, node_name: &str, min: IVec2, max: IVec2) {
	let demo = DemoGraph::open(demo);
	let dragged = demo.node(node_name);

	for (mode, lay_out) in LAYOUT_MODES.into_iter().chain(FITTINGS.into_iter().skip(1)) {
		let field = DragField::new(&demo.graph, vec![dragged], min, max, lay_out);

		let slug = |text: &str| text.to_lowercase().replace(['+', ','], "").split_whitespace().collect::<Vec<_>>().join("-");
		let file_name = format!("{}-{}-{}.svg", demo.demo, slug(node_name), slug(mode));
		let path = output_directory().join(&file_name);
		std::fs::write(&path, heat_map(&field, deformation_cells)).expect("the heat map can be written");
		std::fs::write(output_directory().join(file_name.replace(".svg", "-cursor.svg")), heat_map(&field, cursor_miss)).expect("the cursor heat map can be written");

		eprintln!("{mode}: {}", summary(&field));
		eprintln!("  Nodes jumping most:\n{}", jumping_nodes(&field, &demo.node_names, 8));
		eprintln!("  Connections whose elongation changes most:\n{}", changing_elongations(&field, &demo.node_names, 8));
		eprintln!("Wrote {}\n", path.display());
	}
}

/// The nodes whose connection to their owner changes elongation at the most adjacent steps, which move everything they own with them.
fn changing_elongations(field: &DragField, node_names: &[String], count: usize) -> String {
	let mut changes = vec![(0_u32, 0_u32, 0_u32); node_names.len()];
	for offset in offsets(field) {
		for step in [IVec2::X, IVec2::Y] {
			let (Some(before), Some(after)) = (field.elongations_at(offset), field.elongations_at(offset + step)) else {
				continue;
			};
			for (node, (before, after)) in before.iter().zip(after).enumerate() {
				let change = (*after - *before).abs().element_sum() as u32;
				if change > 0 {
					let (steps, cells, largest) = &mut changes[node];
					*steps += 1;
					*cells += change;
					*largest = (*largest).max(change);
				}
			}
		}
	}
	ranked(&changes, node_names, count, "changes")
}

/// The nodes jumping at the most adjacent steps, with how many steps, how many cells in total, and the largest jump.
fn jumping_nodes(field: &DragField, node_names: &[String], count: usize) -> String {
	let mut jumps = vec![(0_u32, 0_u32, 0_u32); node_names.len()];
	for offset in offsets(field) {
		for step in [IVec2::X, IVec2::Y] {
			let (Some(before), Some(after)) = (field.at(offset), field.at(offset + step)) else { continue };
			for (node, miss) in misses(before, after, step, &field.dragged) {
				let (steps, cells, largest) = &mut jumps[node as usize];
				*steps += 1;
				*cells += miss;
				*largest = (*largest).max(miss);
			}
		}
	}

	ranked(&jumps, node_names, count, "jumps")
}

/// The `count` nodes with the most cells in `tallies` of steps, cells, and largest change, one line each.
fn ranked(tallies: &[(u32, u32, u32)], node_names: &[String], count: usize, verb: &str) -> String {
	let mut ranked: Vec<usize> = (0..tallies.len()).filter(|node| tallies[*node].0 > 0).collect();
	ranked.sort_by_key(|node| std::cmp::Reverse(tallies[*node].1));
	ranked
		.into_iter()
		.take(count)
		.map(|node| {
			let (steps, cells, largest) = tallies[node];
			format!("    {:48} {verb} at {steps:4} steps by {cells:5} cells, largest {largest}", node_names[node])
		})
		.collect::<Vec<_>>()
		.join("\n")
}

fn offsets(field: &DragField) -> impl Iterator<Item = IVec2> + '_ {
	(0..field.size.y).flat_map(move |y| (0..field.size.x).map(move |x| field.min + IVec2::new(x, y)))
}

fn summary(field: &DragField) -> String {
	let displacements: Vec<u32> = offsets(field).filter_map(|offset| field.displacement(offset, IVec2::ZERO)).collect();
	let discontinuities: Vec<Discontinuity> = offsets(field)
		.flat_map(|offset| [IVec2::X, IVec2::Y].map(|step| field.step_discontinuity(offset, step)))
		.flatten()
		.collect();

	let jumps: Vec<&Discontinuity> = discontinuities.iter().filter(|discontinuity| discontinuity.is_jump()).collect();
	let jumping_nodes: u32 = jumps.iter().map(|jump| jump.nodes).sum();
	let jumped_cells: u32 = jumps.iter().map(|jump| jump.cells).sum();
	let largest_jump = jumps.iter().map(|jump| jump.largest).max().unwrap_or_default();
	let max_displacement = displacements.iter().max().copied().unwrap_or_default();
	let max_deformation = offsets(field)
		.filter_map(|offset| field.deformation(offset, IVec2::ZERO))
		.map(|deformation| deformation.cells)
		.max()
		.unwrap_or_default();
	let mean_displacement = displacements.iter().sum::<u32>() as f64 / displacements.len().max(1) as f64;

	format!(
		"{} offsets | {} of {} adjacent steps jump | {jumping_nodes} node jumps over {jumped_cells} cells, largest {largest_jump} | displacement max {max_displacement}, mean {mean_displacement:.1} | deformation max {max_deformation}",
		displacements.len(),
		jumps.len(),
		discontinuities.len()
	)
}

/// How deformed the layout is from the start, in grid cells.
fn deformation_cells(field: &DragField, offset: IVec2) -> u32 {
	field.deformation(offset, IVec2::ZERO).unwrap_or_default().cells
}

/// How far the dragged node is drawn from the cursor, in grid cells.
fn cursor_miss(field: &DragField, offset: IVec2) -> u32 {
	field.cursor_miss(offset).unwrap_or_default()
}

/// Draws each offset colored by `metric` and a line wherever the layout jumps between adjacent offsets.
fn heat_map(field: &DragField, metric: fn(&DragField, IVec2) -> u32) -> String {
	let max_value = offsets(field).map(|offset| metric(field, offset)).max().unwrap_or_default().max(1);
	let [width, height] = (field.size * CELL_SIZE).to_array();

	let mut svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" shape-rendering="crispEdges">"#);
	for offset in offsets(field) {
		let corner = (offset - field.min) * CELL_SIZE;
		let displacement = field.displacement(offset, IVec2::ZERO).unwrap_or_default();
		let deformation = field.deformation(offset, IVec2::ZERO).unwrap_or_default();
		let cursor_miss = field.cursor_miss(offset).unwrap_or_default();
		let color = heat_color(metric(field, offset) as f64 / max_value as f64);
		let _ = write!(
			svg,
			r#"<rect x="{}" y="{}" width="{CELL_SIZE}" height="{CELL_SIZE}" fill="{color}"><title>offset ({}, {}) | {} nodes bent by {} cells, largest {} | displacement {displacement} | cursor missed by {cursor_miss}</title></rect>"#,
			corner.x, corner.y, offset.x, offset.y, deformation.nodes, deformation.cells, deformation.largest
		);
	}

	for offset in offsets(field) {
		let corner = (offset - field.min) * CELL_SIZE;
		for step in [IVec2::X, IVec2::Y] {
			let Some(jump) = field.step_discontinuity(offset, step).filter(Discontinuity::is_jump) else {
				continue;
			};

			// The border shared with the next cell along `step`
			let start = corner + step * CELL_SIZE;
			let end = start + step.perp().abs() * CELL_SIZE;
			// Thicker for more cells jumped, and more opaque for more nodes jumping
			let stroke_width = 1. + (jump.cells as f64).log2();
			let opacity = (0.4 + 0.2 * jump.nodes as f64).min(1.);
			let _ = write!(
				svg,
				r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="#ff2d55" stroke-width="{stroke_width:.1}" stroke-opacity="{opacity:.1}"><title>{} nodes jump {} cells, largest {}</title></line>"##,
				start.x, start.y, end.x, end.y, jump.nodes, jump.cells, jump.largest
			);
		}
	}

	let origin = -field.min * CELL_SIZE;
	let _ = write!(
		svg,
		r#"<rect x="{}" y="{}" width="{CELL_SIZE}" height="{CELL_SIZE}" fill="none" stroke="white" stroke-width="2"/></svg>"#,
		origin.x, origin.y
	);
	svg
}

/// A color from dark blue for no displacement to yellow for the most, given `amount` from 0 to 1.
fn heat_color(amount: f64) -> String {
	let stops = [[13., 8., 135.], [156., 23., 158.], [237., 121., 83.], [240., 249., 33.]];
	let scaled = amount.clamp(0., 1.) * (stops.len() - 1) as f64;
	let index = (scaled.floor() as usize).min(stops.len() - 2);
	let t = scaled - index as f64;
	let [r, g, b] = std::array::from_fn(|channel| (stops[index][channel] + (stops[index + 1][channel] - stops[index][channel]) * t).round() as u8);
	format!("rgb({r},{g},{b})")
}

#[test]
#[ignore]
fn drag_stability_of_the_bulb_glow_gradient() {
	write_drag_stability_maps("procedural-string-lights.graphite", "Bulb Glow Gradient", IVec2::new(-20, -40), IVec2::new(40, 20));
}

/// How unstable dragging one node of a demo is in one layout mode.
struct SurveyCase {
	demo: String,
	node_name: String,
	mode: &'static str,
	jumping_steps: usize,
	steps: usize,
	jumped_cells: u32,
	largest_jump: u32,
	max_deformation: u32,
}

impl SurveyCase {
	fn new(demo: &DemoGraph, node: u32, mode: &'static str, field: &DragField) -> Self {
		let discontinuities: Vec<Discontinuity> = offsets(field)
			.flat_map(|offset| [IVec2::X, IVec2::Y].map(|step| field.step_discontinuity(offset, step)))
			.flatten()
			.collect();
		let jumps = discontinuities.iter().filter(|discontinuity| discontinuity.is_jump());

		Self {
			demo: demo.demo.clone(),
			node_name: demo.node_names[node as usize].clone(),
			mode,
			jumping_steps: jumps.clone().count(),
			steps: discontinuities.len(),
			jumped_cells: jumps.clone().map(|jump| jump.cells).sum(),
			largest_jump: jumps.map(|jump| jump.largest).max().unwrap_or_default(),
			max_deformation: offsets(field)
				.filter_map(|offset| field.deformation(offset, IVec2::ZERO))
				.map(|deformation| deformation.cells)
				.max()
				.unwrap_or_default(),
		}
	}

	fn csv_row(&self) -> String {
		let node_name = self.node_name.replace('"', "\"\"");
		format!(
			"{},\"{node_name}\",{},{},{},{},{},{}",
			self.demo, self.mode, self.jumping_steps, self.steps, self.jumped_cells, self.largest_jump, self.max_deformation
		)
	}
}

/// Drags every node of every demo's root network by every offset within `DRAG_STABILITY_RADIUS` (default 10) in both layout modes,
/// and ranks them by how many cells the layout jumps, to find unstable cases worth optimizing.
#[test]
#[ignore]
fn drag_stability_survey() {
	let radius: i32 = std::env::var("DRAG_STABILITY_RADIUS").ok().and_then(|radius| radius.parse().ok()).unwrap_or(10);
	let mut demos: Vec<String> = std::fs::read_dir("../demo-artwork")
		.expect("the demo artwork can be listed")
		.filter_map(|entry| entry.ok()?.file_name().into_string().ok())
		.filter(|file_name| file_name.ends_with(".graphite"))
		.collect();
	demos.sort();

	let threads = std::thread::available_parallelism().map_or(4, usize::from);
	let mut cases = Vec::new();
	for demo in &demos {
		let demo = DemoGraph::open(demo);
		let nodes: Vec<u32> = (0..demo.graph.nodes.len() as u32).filter(|node| !demo.graph.nodes[*node as usize].waypoint).collect();

		let started = std::time::Instant::now();
		std::thread::scope(|scope| {
			let workers: Vec<_> = nodes
				.chunks(nodes.len().div_ceil(threads).max(1))
				.map(|chunk| {
					let demo = &demo;
					scope.spawn(move || {
						let mut cases = Vec::new();
						for &node in chunk {
							for (mode, lay_out) in LAYOUT_MODES {
								let field = DragField::new(&demo.graph, vec![node], IVec2::splat(-radius), IVec2::splat(radius), lay_out);
								cases.push(SurveyCase::new(demo, node, mode, &field));
							}
						}
						cases
					})
				})
				.collect();
			for worker in workers {
				cases.extend(worker.join().expect("a survey worker finished"));
			}
		});
		eprintln!("{:28} {:4} nodes surveyed in {:?}", demo.demo, nodes.len(), started.elapsed());
	}

	cases.sort_by_key(|case| std::cmp::Reverse(case.jumped_cells));
	let path = output_directory().join("survey.csv");
	let rows: Vec<String> = cases.iter().map(SurveyCase::csv_row).collect();
	let header = "demo,node,mode,jumping_steps,steps,jumped_cells,largest_jump,max_deformation";
	std::fs::write(&path, format!("{header}\n{}\n", rows.join("\n"))).expect("the survey can be written");

	for mode in LAYOUT_MODES.map(|(mode, _)| mode) {
		let of_mode: Vec<&SurveyCase> = cases.iter().filter(|case| case.mode == mode).collect();
		let unstable = of_mode.iter().filter(|case| case.jumping_steps > 0).count();
		let jumped_cells: u32 = of_mode.iter().map(|case| case.jumped_cells).sum();
		eprintln!("{mode}: {unstable} of {} dragged nodes jump, by {jumped_cells} cells in total", of_mode.len());
	}
	eprintln!("Most unstable:");
	for case in cases.iter().take(25) {
		eprintln!(
			"    {:26} {:52} {:9} jumps at {:4} of {} steps by {:6} cells, largest {:3}, deformation up to {}",
			case.demo, case.node_name, case.mode, case.jumping_steps, case.steps, case.jumped_cells, case.largest_jump, case.max_deformation
		);
	}
	eprintln!("Wrote {}", path.display());
}

/// Dragging these nodes made the most unstable layouts in `drag_stability_survey`: per demo the node with the largest jump and the one jumping at the most steps.
const STABILITY_CASES: [(&str, &str); 17] = [
	("changing-seasons", "Subtract (3015793159609237965)"),
	("changing-seasons", "Repeat on Points (14130773813735045204)"),
	("isometric-fountain", "Path (2225749123534781340)"),
	("isometric-fountain", "Fill (5891705401441266824)"),
	("marbled-mandelbrot", "Noise Pattern (6323350524796370485)"),
	("marbled-mandelbrot", "Gradient Map (3606681156406984991)"),
	("painted-dreams", "Stroke (16785043320296790229)"),
	("painted-dreams", "Boolean Operation (10080296672372912698)"),
	("parametric-dunescape", "Dune (13323241418027154136)"),
	("parametric-dunescape", "Dune (17239043462037837834)"),
	("procedural-string-lights", "Into Group (5991401429480840441)"),
	("procedural-string-lights", "Stroke (5250960114142560178)"),
	("procedural-string-lights", "Bulb Glow Gradient (13371003476981866000)"),
	("red-dress", "Vary Scales (7922156219537051964)"),
	("red-dress", "Path (2959546142916532439)"),
	("valley-of-spires", "Path (304)"),
	("valley-of-spires", "Fill (140)"),
];

/// The ways of fitting upstreams together that `drag_stability_score` compares.
const FITTINGS: [NamedLayout; 7] = [
	("current", layout),
	("coarse", |graph| {
		layout_with(
			graph,
			Fitting {
				coarse_steps: true,
				..Default::default()
			},
		)
	}),
	("keep carriers", |graph| layout_with(graph, KEEP_CARRIERS)),
	("+ held first", |graph| layout_with(graph, Fitting { held_first: true, ..KEEP_CARRIERS })),
	("+ spacing", |graph| layout_with(graph, Fitting { spacing: 1, ..KEEP_CARRIERS })),
	("+ first, spacing", |graph| {
		layout_with(
			graph,
			Fitting {
				held_first: true,
				spacing: 1,
				..KEEP_CARRIERS
			},
		)
	}),
	("+ coarse", |graph| layout_with(graph, Fitting { coarse_steps: true, ..KEEP_CARRIERS })),
];

const KEEP_CARRIERS: Fitting = Fitting {
	input_order: false,
	coarse_steps: false,
	keep_carriers_of_held: true,
	held_first: false,
	spacing: 0,
};

/// How unstable one drag is under one way of fitting.
#[derive(Clone, Copy, Default)]
struct Score {
	/// Adjacent steps that jump at all.
	jump_lines: u32,
	/// Jump lines whose largest jump is a single cell.
	creep_lines: u32,
	largest_jump: u32,
	/// Offsets where the dragged node is not drawn where the cursor moved it.
	cursor_misses: u32,
	/// Offsets where more node boxes overlap than where the drag starts.
	overlapping_offsets: u32,
}

/// How many pairs of node boxes overlap, with the empty row a layer keeps below it, leaving out waypoints.
fn overlapping_pairs(graph: &Graph, positions: &[IVec2]) -> u32 {
	let boxes: Vec<[IVec2; 2]> = graph
		.nodes
		.iter()
		.zip(positions)
		.filter(|(node, _)| !node.waypoint)
		.map(|(node, position)| [*position, *position + IVec2::new(node.width as i32, node.height() + i32::from(node.is_layer))])
		.collect();

	let mut pairs = 0;
	for (i, [first_min, first_max]) in boxes.iter().enumerate() {
		for [second_min, second_max] in &boxes[i + 1..] {
			pairs += u32::from(first_min.cmplt(*second_max).all() && second_min.cmplt(*first_max).all());
		}
	}
	pairs
}

impl Score {
	fn of(field: &DragField, graph: &Graph) -> Self {
		let jumps = offsets(field)
			.flat_map(|offset| [IVec2::X, IVec2::Y].map(|step| field.step_discontinuity(offset, step)))
			.flatten()
			.filter(Discontinuity::is_jump);
		let mut score = jumps.fold(Self::default(), |score, jump| Self {
			jump_lines: score.jump_lines + 1,
			creep_lines: score.creep_lines + u32::from(jump.largest == 1),
			largest_jump: score.largest_jump.max(jump.largest),
			..score
		});
		score.cursor_misses = offsets(field).filter(|offset| cursor_miss(field, *offset) > 0).count() as u32;

		let at_start = field.at(IVec2::ZERO).map_or(0, |positions| overlapping_pairs(graph, positions));
		score.overlapping_offsets = offsets(field)
			.filter(|offset| field.at(*offset).is_some_and(|positions| overlapping_pairs(graph, positions) > at_start))
			.count() as u32;
		score
	}

	fn add(self, other: Self) -> Self {
		Self {
			jump_lines: self.jump_lines + other.jump_lines,
			creep_lines: self.creep_lines + other.creep_lines,
			largest_jump: self.largest_jump.max(other.largest_jump),
			cursor_misses: self.cursor_misses + other.cursor_misses,
			overlapping_offsets: self.overlapping_offsets + other.overlapping_offsets,
		}
	}
}

/// How many adjacent drag steps jump at all, how many of those only creep by one cell, and the largest jump.
fn jump_lines(field: &DragField) -> (u32, u32, u32) {
	let score = Score::of(field, &Graph::default());
	(score.jump_lines, score.creep_lines, score.largest_jump)
}

/// Scores each way of fitting upstreams together by how many jump lines dragging each of the `STABILITY_CASES` by up to 15 cells draws,
/// and at how many offsets the dragged node misses the cursor.
#[test]
#[ignore]
fn drag_stability_score() {
	let radius = 15;
	let mut totals = [Score::default(); FITTINGS.len()];

	eprintln!("{:60} {}", "jump lines | cursor misses", FITTINGS.map(|(name, _)| format!("{name:>18}")).join(""));
	let mut demo: Option<DemoGraph> = None;
	for (demo_name, label) in STABILITY_CASES {
		if demo.as_ref().is_none_or(|demo| demo.demo != demo_name) {
			demo = Some(DemoGraph::open(&format!("{demo_name}.graphite")));
		}
		let Some(demo) = &demo else { continue };
		let node = demo.node_labeled(label);

		let scores = std::thread::scope(|scope| {
			let workers = FITTINGS.map(|(_, lay_out)| scope.spawn(move || Score::of(&DragField::new(&demo.graph, vec![node], IVec2::splat(-radius), IVec2::splat(radius), lay_out), &demo.graph)));
			workers.map(|worker| worker.join().expect("a scoring worker finished"))
		});

		let mut row = format!("{:60}", format!("{demo_name} {label}"));
		for (total, score) in totals.iter_mut().zip(scores) {
			*total = total.add(score);
			let _ = write!(row, "{:>18}", format!("{} | {}", score.jump_lines, score.cursor_misses));
		}
		eprintln!("{row}");
	}

	for (name, values) in [
		("total jump lines", totals.map(|total| total.jump_lines)),
		("total creep lines", totals.map(|total| total.creep_lines)),
		("largest jump", totals.map(|total| total.largest_jump)),
		("total cursor misses", totals.map(|total| total.cursor_misses)),
		("total overlapping offsets", totals.map(|total| total.overlapping_offsets)),
	] {
		eprintln!("{name:60} {}", values.map(|value| format!("{value:>18}")).join(""));
	}
}

/// Brute forces the elongations clearing every overlap with the least displacement for each offset `node_name` is dragged by,
/// writes that layout's heat map, and compares which connections it elongates with the ones fitting elongates.
fn write_optimal_drag_map(demo: &str, node_name: &str, min: IVec2, max: IVec2, search_limit: usize) {
	let demo = DemoGraph::open(demo);
	let dragged = demo.node(node_name);
	let started = std::time::Instant::now();

	// One worker per band of rows, each dragging its own copy, since the search dominates
	let rows: Vec<i32> = (min.y..=max.y).collect();
	let threads = std::thread::available_parallelism().map_or(4, usize::from);
	let mut optima: Vec<(IVec2, Layout, Option<usize>)> = std::thread::scope(|scope| {
		let workers: Vec<_> = rows
			.chunks(rows.len().div_ceil(threads).max(1))
			.map(|band| {
				let demo = &demo;
				scope.spawn(move || {
					let mut drag = Drag::new(&demo.graph, vec![dragged], false, layout_as_stored);
					let mut optima = Vec::new();
					for &y in band {
						for x in min.x..=max.x {
							let offset = IVec2::new(x, y);
							let (graph, as_stored) = drag.moved(offset);
							optima.push(match optimal_elongations(&graph, search_limit) {
								Some(optimum) => (offset, layout_with_elongations(&graph, &optimum.elongations), Some(optimum.searched)),
								None => (offset, as_stored, None),
							});
						}
					}
					optima
				})
			})
			.collect();
		workers.into_iter().flat_map(|worker| worker.join().expect("an optimizing worker finished")).collect()
	});
	optima.sort_by_key(|(offset, _, _)| (offset.y, offset.x));

	let searched: Vec<usize> = optima.iter().filter_map(|(_, _, searched)| *searched).collect();
	let unsolved = optima.len() - searched.len();
	eprintln!(
		"optimal: {} offsets in {:?} | {unsolved} past the search limit | searched mean {:.0}, max {}",
		optima.len(),
		started.elapsed(),
		searched.iter().sum::<usize>() as f64 / searched.len().max(1) as f64,
		searched.iter().max().copied().unwrap_or_default()
	);

	let mut layouts = optima.into_iter().map(|(_, layout, _)| layout);
	let optimal = DragField::from_layouts(vec![dragged], min, max, |_| layouts.next().unwrap_or_default());
	let fitted = DragField::new(&demo.graph, vec![dragged], min, max, layout);

	let path = output_directory().join(format!("{}-{}-optimal.svg", demo.demo, node_name.to_lowercase().replace(' ', "-")));
	std::fs::write(&path, heat_map(&optimal, deformation_cells)).expect("the heat map can be written");
	for (name, field) in [("optimal", &optimal), ("fitted", &fitted)] {
		let (lines, creep, largest) = jump_lines(field);
		eprintln!("{name:>8}: {lines} jump lines, {creep} creeping, largest {largest} | {}", summary(field));
	}
	eprintln!(
		"  Connections elongated, at how many offsets by the optimum and by fitting:\n{}",
		elongated_connections(&optimal, &fitted, &demo.node_names)
	);
	eprintln!("Wrote {}", path.display());
}

/// For each connection either field elongates, at how many offsets the first, the second, and both elongate it.
fn elongated_connections(first: &DragField, second: &DragField, node_names: &[String]) -> String {
	let mut counts = vec![(0_u32, 0_u32, 0_u32); node_names.len()];
	for offset in offsets(first) {
		let (Some(first), Some(second)) = (first.elongations_at(offset), second.elongations_at(offset)) else {
			continue;
		};
		for (node, (first, second)) in first.iter().zip(second).enumerate() {
			let (in_first, in_second) = (*first != IVec2::ZERO, *second != IVec2::ZERO);
			let (first_count, second_count, both) = &mut counts[node];
			*first_count += u32::from(in_first);
			*second_count += u32::from(in_second);
			*both += u32::from(in_first && in_second);
		}
	}

	let mut ranked: Vec<usize> = (0..counts.len()).filter(|node| counts[*node].0 + counts[*node].1 > 0).collect();
	ranked.sort_by_key(|node| std::cmp::Reverse(counts[*node].0.max(counts[*node].1)));
	ranked
		.into_iter()
		.map(|node| {
			let (first, second, both) = counts[node];
			format!("    {:48} optimum {first:5} | fitting {second:5} | both {both:5}", node_names[node])
		})
		.collect::<Vec<_>>()
		.join("\n")
}

#[test]
#[ignore]
fn optimal_drag_of_the_bulb_glow_gradient() {
	write_optimal_drag_map("procedural-string-lights.graphite", "Bulb Glow Gradient", IVec2::new(-20, -40), IVec2::new(40, 20), 20_000);
}

/// Draws the layout at both ends of one drag step side by side: the dragged node blue, nodes the step moves red, the rest gray.
fn step_svg(graph: &Graph, before: &Layout, after: &Layout, dragged: u32, node_names: &[String], title: &str) -> String {
	const SCALE: i32 = 8;
	let bounds = before.bounds.iter().chain(&after.bounds);
	let min = bounds.clone().fold(IVec2::MAX, |min, [top_left, _]| min.min(*top_left)) - IVec2::splat(2);
	let max = bounds.fold(IVec2::MIN, |max, [_, bottom_right]| max.max(*bottom_right)) + IVec2::splat(2);
	let panel = (max - min) * SCALE;

	let mut svg = format!(
		r##"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" font-family="sans-serif" font-size="9"><rect width="100%" height="100%" fill="#1e1e1e"/><text x="4" y="12" fill="white" font-size="12">{title}</text>"##,
		panel.x * 2 + 20,
		panel.y + 20
	);
	for (panel_index, layout) in [before, after].into_iter().enumerate() {
		let origin = IVec2::new(panel_index as i32 * (panel.x + 20), 20) - min * SCALE;
		let point = |cell: IVec2| origin + cell * SCALE;

		for [output, input] in &layout.wire_ends {
			let (from, to) = (point(*output), point(*input));
			let _ = write!(svg, r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="#666" stroke-width="1"/>"##, from.x, from.y, to.x, to.y);
		}
		for (node, [top_left, bottom_right]) in layout.bounds.iter().enumerate() {
			if graph.nodes[node].waypoint {
				continue;
			}
			let moved = before.positions[node] != after.positions[node];
			let fill = if node as u32 == dragged {
				"#3478f6"
			} else if moved {
				"#e5484d"
			} else {
				"#555"
			};
			let (corner, size) = (point(*top_left), (*bottom_right - *top_left) * SCALE);
			let name = node_names[node].split(" (").next().unwrap_or_default();
			let _ = write!(
				svg,
				r##"<rect x="{}" y="{}" width="{}" height="{}" fill="{fill}" fill-opacity="0.8" stroke="#ddd" stroke-width="0.5"/><text x="{}" y="{}" fill="white">{name}</text>"##,
				corner.x,
				corner.y,
				size.x,
				size.y,
				corner.x + 2,
				corner.y + 10
			);
		}
	}
	svg + "</svg>"
}

#[test]
#[ignore]
fn inspect_difficult_drags_of_the_bulb_glow_gradient() {
	let demo = DemoGraph::open("procedural-string-lights.graphite");
	let dragged = demo.node("Bulb Glow Gradient");
	let (min, max) = (IVec2::new(-20, -40), IVec2::new(40, 20));
	let field = DragField::new(&demo.graph, vec![dragged], min, max, layout);

	let at_rest = layout(&demo.graph);
	let candidates = (0..demo.graph.nodes.len()).filter(|node| at_rest.owners[*node].is_some() && !demo.graph.nodes[*node].waypoint && at_rest.chain_layers[*node].is_none());
	eprintln!("Connections fitting may elongate: {} of {} nodes", candidates.count(), demo.graph.nodes.len());

	// The largest jumps, then creeping sideways steps through the unstable band above the start
	let mut steps: Vec<(IVec2, IVec2, Discontinuity)> = offsets(&field)
		.flat_map(|offset| [IVec2::X, IVec2::Y].map(|step| (offset, step)))
		.filter_map(|(offset, step)| Some((offset, step, field.step_discontinuity(offset, step).filter(Discontinuity::is_jump)?)))
		.collect();
	steps.sort_by_key(|(_, _, jump)| Reverse(jump.largest));
	let mut chosen: Vec<(IVec2, IVec2, Discontinuity)> = Vec::new();
	for step in steps.iter().copied() {
		if chosen.len() < 4 && chosen.iter().all(|(offset, _, _)| (*offset - step.0).abs().max_element() > 6) {
			chosen.push(step);
		}
	}
	chosen.extend(
		steps
			.iter()
			.copied()
			.filter(|(offset, step, jump)| *step == IVec2::X && jump.largest == 1 && (-15..=-5).contains(&offset.y) && offset.x >= 10)
			.step_by(40)
			.take(2),
	);

	let mut drag = Drag::new(&demo.graph, vec![dragged], false, layout);
	for (index, (offset, step, jump)) in chosen.into_iter().enumerate() {
		let (graph, before) = drag.moved(offset);
		let (_, after) = drag.moved(offset + step);

		eprintln!("Step {index}: from offset {offset} by {step}, {} nodes jump {} cells, largest {}", jump.nodes, jump.cells, jump.largest);
		for (node, miss) in misses(&before.positions, &after.positions, step, &[dragged]) {
			let moved = after.positions[node as usize] - before.positions[node as usize];
			let elongation = after.elongations[node as usize] - before.elongations[node as usize];
			eprintln!("    {:48} moves {moved:>12} (misses by {miss}), elongation changes {elongation}", demo.node_names[node as usize]);
		}

		let title = format!("step {index}: offset {offset} + {step}");
		let svg = step_svg(&graph, &before, &after, dragged, &demo.node_names, &title);
		std::fs::write(output_directory().join(format!("step-{index}.svg")), svg).expect("the step can be written");
	}
}
