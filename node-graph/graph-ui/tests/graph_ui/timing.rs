use super::{ARTWORK, artwork, badge, config, connector, context, executor, gpu, layer, network, network_connector, node, thumbnail, wgpu_executor, wire};
use core_types::list::List;
use core_types::uuid::NodeId;
use core_types::{Context, OwnedContextImpl, Poll};
use glam::{DAffine2, DVec2, IVec2, UVec2};
use graph_craft::document::value::{RenderOutput, RenderOutputType, TaggedValue};
use graph_ui::GraphUi;
use graph_ui::card::{Card, Connector, DataType, Highlight, Selection};
use graph_ui::error_badge::ErrorBadge;
use graph_ui::layer::Layer;
use graph_ui::network_connector::{NetworkConnector, NetworkConnectorKind};
use graph_ui::replicated_map::ReplicatedMap;
use graph_ui::scene::{GraphUiScene, GraphUiSceneEdit};
use graph_ui::wire::{Wire, WireKey};
use graphene_std::application_io::{ExportFormat, RenderConfig};
use graphene_std::transform::Footprint;
use graphic_types::{Graphic, Vector};
use rendering::vello_scene_builder::VelloSceneBuilder;
use rendering::{Render, RenderParams};
use std::time::{Duration, Instant};
use vello_gpu::kurbo::{Affine, BezPath, Rect};

const CARDS: usize = 300;
const LAYERS: usize = 60;
const ERRORS: usize = 10;
const GLIDING: usize = 5;
const COLUMNS: usize = 20;
const SAMPLES: usize = 21;
const RESOLUTION: UVec2 = UVec2::new(1920, 1080);
const NAMES: [&str; 10] = [
	"Transform",
	"Fill",
	"Stroke",
	"Boolean Operation",
	"Copy to Points",
	"Sample Polyline",
	"Morph",
	"Blend Shapes",
	"Image Color Palette",
	"Rectangle",
];
const ICONS: [&str; 10] = [
	"NodeTransform",
	"NodeFill",
	"NodeStroke",
	"NodeBooleanOperation",
	"NodeRepeatOnPoints",
	"NodeSamplePolyline",
	"NodeMorph",
	"NodeBlend",
	"NodeImageColorPalette",
	"NodeRectangle",
];
const ERROR: &str = "These input types are incompatible:\n• Input 2:\n…found:       Color\n…expected: Number\n\nThis is not a supported arrangement of types for the node.";
const ROWS: [(&str, DataType); 8] = [
	("Translation", DataType::General),
	("Rotation", DataType::Number),
	("Scale", DataType::General),
	("Color", DataType::Color),
	("Width", DataType::Number),
	("Opacity", DataType::Number),
	("Gradient", DataType::Gradient),
	("Separation Disk Diameter", DataType::Number),
];

fn selection(index: usize) -> Selection {
	if index.is_multiple_of(7) { Selection::Active } else { Selection::None }
}

fn connector_from((name, data_type): (&str, DataType), connected: bool) -> Connector {
	Connector {
		connected,
		..connector(name, data_type)
	}
}

fn card(index: usize) -> Card {
	Card {
		icon: ICONS[index % ICONS.len()].to_string(),
		primary_input: Some(connector_from(("Content", DataType::Vector), true)),
		primary_output: Some(connector_from(("Out", DataType::Vector), !index.is_multiple_of(4))),
		secondary_inputs: (0..2 + index % 3)
			.map(|row| connector_from(ROWS[(index + row) % ROWS.len()], (index + row).is_multiple_of(3)))
			.collect(),
		secondary_outputs: if index.is_multiple_of(5) {
			vec![connector_from(("Area", DataType::Number), false)]
		} else {
			Vec::new()
		},
		selection: selection(index),
		visible: !index.is_multiple_of(11),
		previewed: index == 0,
		..super::card(index as u64, ((index % COLUMNS) as i32 * 7, (index / COLUMNS) as i32 * 6), NAMES[index % NAMES.len()])
	}
}

fn nth_layer(index: usize) -> Layer {
	Layer {
		width: 8 + (index % 3) as u32,
		chain_width: if index.is_multiple_of(2) { 8 } else { 0 },
		has_left_input_wire: index.is_multiple_of(4),
		primary_input: Some(connector_from(("", DataType::Graphic), !index.is_multiple_of(10))),
		primary_output: Some(connector_from(("", DataType::Graphic), true)),
		secondary_input: Some(connector_from(("", DataType::Vector), index.is_multiple_of(2))),
		primary_input_connected_to_layer: !index.is_multiple_of(10),
		primary_output_connected_to_layer: !index.is_multiple_of(10),
		visible: !index.is_multiple_of(11),
		locked: index.is_multiple_of(13),
		selection: selection(index),
		previewed: index == 1,
		..layer((CARDS + index) as u64, ((index % 10) as i32 * 14 + 3, (index / 10) as i32 * 6 + 3), NAMES[index % NAMES.len()])
	}
}

fn unnamed(mut card: Card) -> Card {
	card.name.clear();
	let connectors = card
		.primary_input
		.iter_mut()
		.chain(&mut card.primary_output)
		.chain(&mut card.secondary_inputs)
		.chain(&mut card.secondary_outputs);
	connectors.for_each(|connector| connector.name.clear());
	card
}

fn scene(cards: impl IntoIterator<Item = Card>) -> GraphUiScene {
	let mut edit = GraphUiScene::default().edit();
	edit.cards.replace_all(cards);
	edit.finish().0
}

fn rewire(edit: &mut GraphUiSceneEdit) {
	let wires = wires(&edit.cards);
	edit.wires.replace_all(wires);
}

fn wires(cards: &ReplicatedMap<Card>) -> Vec<Wire> {
	cards.iter().zip(cards.iter().skip(1)).map(|(from, to)| wire_between(from, to)).collect()
}

fn wire_between(from: &Card, to: &Card) -> Wire {
	let center = |(top_left, _): (DVec2, &Connector)| top_left + 4.;
	let [start, end] = [from.outputs().next(), to.inputs().next()].map(|connector| connector.map(center).expect("every card has a primary input and output"));
	let point = |point: DVec2| (point.x, point.y);
	let mut path = BezPath::new();
	path.move_to(point(start));
	path.line_to(point(start + DVec2::X * 12.));
	path.curve_to(point(start + DVec2::X * 48.), point(end - DVec2::X * 48.), point(end - DVec2::X * 12.));
	path.line_to(point(end));
	Wire {
		list: to.key.0.is_multiple_of(4),
		..wire(WireKey::Input { node: to.key, index: 0 }, path, DataType::Vector)
	}
}

fn network_connectors(cells_down: i32) -> Vec<NetworkConnector> {
	let column = |kind, x| {
		let place = move |row: usize| (x, (cells_down + row as i32) * 24);
		let named = ROWS[..4].iter().enumerate().map(move |(index, &row)| NetworkConnector {
			editable: true,
			..network_connector(kind, index, place(if index == 0 { 0 } else { index + 1 }), Some(connector_from(row, true)))
		});
		named.chain([network_connector(kind, 4, place(6), None)])
	};
	column(NetworkConnectorKind::Import, -96).chain(column(NetworkConnectorKind::Export, COLUMNS as i32 * 7 * 24)).collect()
}

fn error(index: usize) -> ErrorBadge {
	let card = card(index * 31 % CARDS);
	badge(card.key.0, card.cell.into(), ERROR)
}

fn full_scene() -> GraphUiScene {
	let mut edit = scene((0..CARDS).map(card)).edit();
	edit.layers.replace_all((0..LAYERS).map(nth_layer));
	edit.network_connectors.replace_all(network_connectors(0));
	edit.errors.replace_all((0..ERRORS).map(error));
	rewire(&mut edit);
	edit.finish().0
}

fn moved(card: &Card, offset: IVec2) -> Card {
	Card {
		cell: card.cell + offset,
		..card.clone()
	}
}

fn reselected(card: &Card) -> Card {
	let selection = match card.selection {
		Selection::None => Selection::Inactive,
		Selection::Inactive => Selection::Active,
		Selection::Active => Selection::None,
	};
	Card { selection, ..card.clone() }
}

fn median(mut times: Vec<Duration>) -> Duration {
	times.sort();
	times[times.len() / 2]
}

fn ms(time: Duration) -> f64 {
	time.as_secs_f64() * 1000.
}

#[test]
#[ignore = "prints timings"]
fn time_a_pan_a_card_change_and_a_rebuild_of_300_cards() {
	let gpu = gpu();
	let mut wrapped = network(&gpu, [node(artwork::IDENTIFIER, vec![wgpu_executor()])]);
	graph_ui::render::wrap_network(&mut wrapped);
	let render_graph_ui = executor(wrapped);
	let draw_cards = executor(network(&gpu, [node(graph_ui::nodes::draw_cards::IDENTIFIER, vec![])]));
	let draw_wires = executor(network(&gpu, [node(graph_ui::nodes::draw_wires::IDENTIFIER, vec![])]));
	let draw_grid = executor(network(&gpu, [node(graph_ui::nodes::draw_grid::IDENTIFIER, vec![])]));
	let draw_thumbnails = executor(network(&gpu, [node(graph_ui::nodes::draw_thumbnails::IDENTIFIER, vec![])]));

	let render_config = RenderConfig {
		viewport: Footprint {
			resolution: RESOLUTION,
			..Default::default()
		},
		scale: 1.,
		export_format: ExportFormat::Raster,
		..Default::default()
	};
	let view = |sample: usize| DAffine2::from_scale_angle_translation(DVec2::splat(0.5), 0., DVec2::new(100. + sample as f64, 0.));
	let mut editor_scene = full_scene();
	let mut runtime_scene = editor_scene.clone();
	let thumbnails: Vec<(NodeId, Graphic)> = editor_scene.layers().iter().map(|layer| (layer.key, thumbnail(&gpu, Rect::new(12., 0., 60., 48.), ARTWORK))).collect();
	let mut render = |change: &mut dyn FnMut(&mut GraphUiSceneEdit), view: DAffine2| {
		let mut edit = std::mem::take(&mut editor_scene).edit();
		change(&mut edit);
		let (scene, delta) = edit.finish();
		editor_scene = scene;

		let start = Instant::now();
		runtime_scene.apply(delta);
		let graph_ui = GraphUi {
			scene: runtime_scene.clone(),
			config: config(view, 0.5),
			thumbnails: thumbnails.clone(),
		};
		let context = context(render_config, Some(graph_ui));
		let output = render_graph_ui.tree().eval_tagged_value(render_graph_ui.output(), &context).unwrap();
		let time = start.elapsed();

		let Poll::Final(TaggedValue::RenderOutput(RenderOutput {
			data: RenderOutputType::Texture(texture),
			..
		})) = output
		else {
			panic!("{output:?}")
		};
		gpu.read_texture_pixels(&texture).unwrap();
		time
	};

	render(&mut |_| {}, view(0));
	let pan = median((1..=SAMPLES).map(|sample| render(&mut |_| {}, view(sample))).collect());
	let interface_pan = median(
		(1..=SAMPLES)
			.map(|sample| render(&mut |edit| edit.network_connectors.replace_all(network_connectors(sample as i32 % 2)), view(sample)))
			.collect(),
	);
	let card_change = median(
		(0..SAMPLES)
			.map(|sample| {
				let key = NodeId((sample * 37 % CARDS) as u64);
				render(
					&mut |edit| {
						edit.cards.update(&key, |card| *card = reselected(&moved(card, IVec2::Y)));
						rewire(edit);
					},
					view(0),
				)
			})
			.collect(),
	);
	let highlight = |card: &mut Card, highlight| card.primary_output.as_mut().expect("every card has a primary output").highlight = highlight;
	let mut hovered = None;
	let hover_move = median(
		(0..SAMPLES)
			.map(|sample| {
				let key = NodeId((sample * 37 % CARDS) as u64);
				let left = hovered.replace(key);
				render(
					&mut |edit| {
						if let Some(left) = left {
							edit.cards.update(&left, |card| highlight(card, Highlight::None));
						}
						edit.cards.update(&key, |card| highlight(card, Highlight::Hovered));
					},
					view(0),
				)
			})
			.collect(),
	);
	let glide_frame = median(
		(0..SAMPLES)
			.map(|sample| {
				let glide = DVec2::new(-24. * 0.6_f64.powi(sample as i32 % 8 + 1), 0.);
				render(
					&mut |edit| {
						for index in 0..GLIDING {
							edit.cards.update(&NodeId((index * 61 % CARDS) as u64), |card| card.glide = glide);
						}
						rewire(edit);
					},
					view(0),
				)
			})
			.collect(),
	);
	let mut every_card = |changed: fn(&Card, IVec2) -> Card| {
		median(
			(0..SAMPLES)
				.map(|sample| {
					let offset = if sample % 2 == 0 { IVec2::X } else { -IVec2::X };
					render(
						&mut |edit| {
							let cards: Vec<Card> = edit.cards.iter().map(|card| changed(card, offset)).collect();
							edit.cards.replace_all(cards);
							rewire(edit);
						},
						view(0),
					)
				})
				.collect(),
		)
	};
	let rebuild = every_card(|card, offset| reselected(&moved(card, offset)));
	let every_card_moved = every_card(moved);

	let draw = |card_at: fn(usize) -> Card| {
		let cards: Vec<Card> = (0..CARDS).map(card_at).collect();
		let scenes = [scene(cards.clone()), scene(cards.iter().map(reselected))];
		let times = (0..SAMPLES).map(|sample| {
			let context = OwnedContextImpl::default().with_vararg(Box::new(scenes[sample % 2].clone())).into_context();
			let start = Instant::now();
			draw_cards.tree().eval::<Context, List<Vector>>(draw_cards.output(), &context).expect("the cards are drawn");
			start.elapsed()
		});
		median(times.collect())
	};
	let drawn = draw(card);
	let drawn_unnamed = draw(|index| unnamed(card(index)));
	let text = drawn.saturating_sub(drawn_unnamed);

	let wired = |cards: Vec<Card>| {
		let mut edit = scene(cards).edit();
		rewire(&mut edit);
		edit.finish().0
	};
	let cards: Vec<Card> = (0..CARDS).map(card).collect();
	let scenes = [wired(cards.clone()), wired(cards.iter().map(|card| moved(card, IVec2::X)).collect())];
	let wires_drawn = median(
		(0..SAMPLES)
			.map(|sample| {
				let context = OwnedContextImpl::default().with_vararg(Box::new(scenes[sample % 2].clone())).into_context();
				let start = Instant::now();
				draw_wires.tree().eval::<Context, List<Vector>>(draw_wires.output(), &context).expect("the wires are drawn");
				start.elapsed()
			})
			.collect(),
	);

	let grid = |zoom: f64, scale: f64| {
		let resolution = (RESOLUTION.as_dvec2() * scale).as_uvec2();
		let render_config = RenderConfig {
			viewport: Footprint { resolution, ..render_config.viewport },
			scale,
			..render_config
		};
		let times = (1..=SAMPLES).map(|sample| {
			let graph_ui = GraphUi {
				scene: GraphUiScene::default(),
				config: config(DAffine2::from_scale_angle_translation(DVec2::splat(zoom), 0., DVec2::new(100. + sample as f64, 0.)), 0.5),
				thumbnails: Vec::new(),
			};
			let context = context(render_config, Some(graph_ui));
			let mut scene = VelloSceneBuilder::new(resolution.x as u16, resolution.y as u16, Affine::IDENTITY, resolution.x);
			let start = Instant::now();
			let dots = draw_grid.tree().eval::<Context, List<Vector>>(draw_grid.output(), &context).expect("the grid is drawn");
			let drawn = start.elapsed();
			dots.render_to_vello(&mut scene, DAffine2::from_scale(DVec2::splat(scale)), &RenderParams::default());
			(drawn, start.elapsed() - drawn)
		});
		let (drawn, encoded): (Vec<Duration>, Vec<Duration>) = times.unzip();
		(median(drawn), median(encoded))
	};
	let grids = [1., 2.].map(|scale| (scale, grid(0.5, scale), grid(1., scale)));

	let times = (1..=SAMPLES).map(|sample| {
		let graph_ui = GraphUi {
			scene: editor_scene.clone(),
			config: config(view(sample), 0.5),
			thumbnails: thumbnails.clone(),
		};
		let context = context(render_config, Some(graph_ui));
		let mut scene = VelloSceneBuilder::new(RESOLUTION.x as u16, RESOLUTION.y as u16, Affine::IDENTITY, RESOLUTION.x);
		let start = Instant::now();
		let drawn = draw_thumbnails
			.tree()
			.eval::<Context, List<Vector>>(draw_thumbnails.output(), &context)
			.expect("the thumbnails are drawn");
		let elapsed = start.elapsed();
		drawn.render_to_vello(&mut scene, view(sample), &RenderParams::default());
		(elapsed, start.elapsed() - elapsed)
	});
	let (thumbnails_drawn, thumbnails_encoded): (Vec<Duration>, Vec<Duration>) = times.unzip();

	let scene_context = OwnedContextImpl::default().with_vararg(Box::new(full_scene())).into_context();
	let scene_bands = [
		("cards", graph_ui::nodes::draw_cards::IDENTIFIER),
		("raised", graph_ui::nodes::draw_raised::IDENTIFIER),
		("layers", graph_ui::nodes::draw_layers::IDENTIFIER),
		("wires", graph_ui::nodes::draw_wires::IDENTIFIER),
		("errors", graph_ui::nodes::draw_errors::IDENTIFIER),
		("imports/exports", graph_ui::nodes::draw_network_connectors::IDENTIFIER),
	]
	.map(|(name, band)| {
		let band = executor(network(&gpu, [node(band, Vec::new())]));
		(name, band.tree().eval::<Context, List<Vector>>(band.output(), &scene_context).expect("the band is drawn"))
	});
	let encoded = |band: &List<Vector>, zoom: f64, left: f64| {
		let times = (1..=SAMPLES).map(|sample| {
			let view = DAffine2::from_scale_angle_translation(DVec2::splat(zoom), 0., DVec2::new(left + sample as f64, 0.));
			let mut scene = VelloSceneBuilder::new(RESOLUTION.x as u16, RESOLUTION.y as u16, Affine::IDENTITY, RESOLUTION.x);
			let start = Instant::now();
			band.render_to_vello(&mut scene, view, &RenderParams::default());
			start.elapsed()
		});
		median(times.collect())
	};
	let band_encodes = [0.5, 1.].map(|zoom| {
		let bands = scene_bands.iter().map(|(name, band)| (*name, band.len(), encoded(band, zoom, 100.), encoded(band, zoom, -100_000.)));
		(zoom, bands.collect::<Vec<_>>())
	});

	let profile = if cfg!(debug_assertions) { "dev" } else { "release" };
	println!(
		"{CARDS} cards, each wired to the next, {LAYERS} layers, four imports and four exports and {ERRORS} error badges at {}x{}, {profile} profile, CPU time without the GPU wait, median of {SAMPLES}",
		RESOLUTION.x, RESOLUTION.y
	);
	println!("pan step (view change only): {:.3} ms", ms(pan));
	println!("pan step that moves the imports and exports a cell: {:.3} ms", ms(interface_pan));
	println!("one-card change (one card moved and reselected, so drawn again, and its two wires): {:.3} ms", ms(card_change));
	println!("hover moving from one card to another: {:.3} ms", ms(hover_move));
	println!("glide frame of {GLIDING} cards and their wires: {:.3} ms", ms(glide_frame));
	println!("full rebuild (every card and wire changed): {:.3} ms", ms(rebuild));
	println!("every card only moved a cell (placed again, not drawn again), every wire changed: {:.3} ms", ms(every_card_moved));
	println!("draw_cards alone, every card drawn again: {:.3} ms, with every name empty: {:.3} ms", ms(drawn), ms(drawn_unnamed));
	println!("draw_wires alone, every wire drawn again: {:.3} ms", ms(wires_drawn));
	println!(
		"text (width, ellipsized, outline), the difference: {:.3} ms, {:.0}% of draw_cards, {:.0}% of a one-card change",
		ms(text),
		100. * text.as_secs_f64() / drawn.as_secs_f64(),
		100. * text.as_secs_f64() / card_change.as_secs_f64()
	);
	for (scale, (drawn, encoded), (drawn_at_1, encoded_at_1)) in grids {
		println!(
			"grid alone at zoom 0.5 and device scale {scale}: draw_grid {:.3} ms + Vello encode {:.3} ms (at zoom 1: {:.3} ms + {:.3} ms)",
			ms(drawn),
			ms(encoded),
			ms(drawn_at_1),
			ms(encoded_at_1)
		);
	}
	println!(
		"thumbnails alone, one on each of the {LAYERS} layers: draw_thumbnails {:.3} ms + Vello encode {:.3} ms",
		ms(median(thumbnails_drawn)),
		ms(median(thumbnails_encoded))
	);
	for (zoom, bands) in band_encodes {
		println!("Vello encode alone of each scene band at zoom {zoom}, on screen and with every item panned off screen (the bookkeeping before the cull):");
		for (name, items, on_screen, off_screen) in &bands {
			println!("  {name} ({items} items): {:.3} ms on screen, {:.3} ms off screen", ms(*on_screen), ms(*off_screen));
		}
		let on_screen: Duration = bands.iter().map(|(_, _, on_screen, _)| *on_screen).sum();
		let off_screen: Duration = bands.iter().map(|(_, _, _, off_screen)| *off_screen).sum();
		println!("  all six: {:.3} ms on screen, {:.3} ms off screen", ms(on_screen), ms(off_screen));
	}
}
