use super::{badge, card, config, connector, context, executor, gpu, layer, nested, network_connector, node, thumbnail, wire};
use core_types::color::SRGBA8;
use core_types::list::List;
use core_types::uuid::NodeId;
use core_types::{Context, Ctx, Poll};
use glam::{DAffine2, DVec2, IVec2, UVec2};
use graph_craft::document::value::{RenderOutput, RenderOutputType, TaggedValue};
use graph_craft::document::{DocumentNodeImplementation, NodeInput, NodeNetwork};
use graph_ui::card::{Card, Connector, DataType, Highlight, Selection};
use graph_ui::dot::Dot;
use graph_ui::error_badge::ErrorBadge;
use graph_ui::layer::{Layer, LayerButton};
use graph_ui::network_connector::{NetworkConnector, NetworkConnectorKind};
use graph_ui::scene::GraphUiScene;
use graph_ui::wire::{Wire, WireInProgress, WireKey};
use graph_ui::{GraphUi, GraphUiConfig};
use graphene_std::application_io::{ExportFormat, RenderConfig, Texture};
use graphene_std::transform::Footprint;
use graphic_types::{Graphic, Vector};
use interpreted_executor::dynamic_executor::{DynamicExecutor, OutputHandle};
use std::cell::RefCell;
use vello_gpu::kurbo::{BezPath, PathEl, Rect};
use wgpu_executor::WgpuExecutor;

const SIZE: UVec2 = UVec2::new(160, 96);
const SCALE: f64 = 2.;
const BACKDROP: SRGBA8 = SRGBA8::new_opaque(0x22, 0x22, 0x22);
const NOTHING: SRGBA8 = SRGBA8::new(0, 0, 0, 0);
const BORDER: SRGBA8 = SRGBA8::new_opaque(0x55, 0x55, 0x55);
const PREVIEWED_BORDER: SRGBA8 = SRGBA8::new_opaque(0xee, 0xee, 0xee);
const GENERAL: SRGBA8 = SRGBA8::new_opaque(0xcf, 0xcf, 0xcf);
const GENERAL_DIM: SRGBA8 = SRGBA8::new_opaque(0x8a, 0x8a, 0x8a);
const VECTOR: SRGBA8 = SRGBA8::new_opaque(0x65, 0xbb, 0xe5);
const VECTOR_DIM: SRGBA8 = SRGBA8::new_opaque(0x41, 0x78, 0x92);
const NUMBER: SRGBA8 = SRGBA8::new_opaque(0xc9, 0xa6, 0x99);
const NUMBER_DIM: SRGBA8 = SRGBA8::new_opaque(0x88, 0x6b, 0x60);
const COLOR: SRGBA8 = SRGBA8::new_opaque(0xce, 0x6e, 0xa7);
const COLOR_DIM: SRGBA8 = SRGBA8::new_opaque(0x92, 0x40, 0x71);
const GRAPHIC: SRGBA8 = SRGBA8::new_opaque(0x68, 0xc5, 0x87);
const GRAPHIC_DIM: SRGBA8 = SRGBA8::new_opaque(0x37, 0x75, 0x4c);
const WIRE_SPLIT: SRGBA8 = SRGBA8::new_opaque(0x44, 0x44, 0x44);
const ERROR_RED: SRGBA8 = SRGBA8::new_opaque(0xd6, 0x53, 0x6e);
const TEXT: SRGBA8 = SRGBA8::new_opaque(0xee, 0xee, 0xee);
const HOVERED_BUTTON: SRGBA8 = SRGBA8::new_opaque(0x55, 0x55, 0x55);
const DISABLED_ICON: SRGBA8 = SRGBA8::new_opaque(0x88, 0x88, 0x88);
const CHECKER_LIGHT: SRGBA8 = SRGBA8::new_opaque(0xff, 0xff, 0xff);
const CHECKER_DARK: SRGBA8 = SRGBA8::new_opaque(0xcc, 0xcc, 0xcc);

const TOP_BORDER: DVec2 = DVec2::new(30., 12.5);
const ABOVE: DVec2 = DVec2::new(30., 6.);
const NEXT_TOP_BORDER: DVec2 = DVec2::new(30., 36.5);
const IN_THE_ICON: DVec2 = DVec2::new(34., 24.);

thread_local! {
	static RECORDED_CARDS: RefCell<Option<List<Vector>>> = const { RefCell::new(None) };
}

fn resolution() -> UVec2 {
	(SIZE.as_dvec2() * SCALE).as_uvec2()
}

#[node_macro::node(category(""))]
fn record_cards(_: impl Ctx, cards: List<Vector>) -> List<Vector> {
	RECORDED_CARDS.set(Some(cards.clone()));
	cards
}

fn branding() -> bool {
	graph_ui::icons::icon("NodeCustomNode", TEXT).is_some()
}

fn graph_ui_network(gpu: &WgpuExecutor) -> NodeNetwork {
	let mut network = graph_ui::render::network();

	let id_of = |identifier| {
		let implementation = DocumentNodeImplementation::ProtoNode(identifier);
		network.nodes.iter().find(|(_, node)| node.implementation == implementation).map(|(id, _)| *id).unwrap()
	};
	let (drawn_cards, root) = (id_of(graph_ui::nodes::draw_cards::IDENTIFIER), id_of(graph_ui::render::render_graph_ui::IDENTIFIER));
	let recorded_cards = NodeId(network.nodes.keys().max().unwrap().0 + 1);
	let root = network.nodes.get_mut(&root).unwrap();
	let cards = root.inputs.iter_mut().find(|input| **input == NodeInput::node(drawn_cards, 0)).unwrap();
	*cards = NodeInput::node(recorded_cards, 0);
	network.nodes.insert(recorded_cards, node(record_cards::IDENTIFIER, vec![NodeInput::node(drawn_cards, 0)]));
	super::network(gpu, [nested(network)])
}

fn graph(scene: &GraphUiScene, config: GraphUiConfig) -> GraphUi {
	GraphUi {
		scene: scene.clone(),
		config,
		thumbnails: Vec::new(),
	}
}

fn render_config(export_format: ExportFormat) -> RenderConfig {
	RenderConfig {
		viewport: Footprint {
			resolution: resolution(),
			..Default::default()
		},
		scale: SCALE,
		export_format,
		..Default::default()
	}
}

fn evaluated(executor: &DynamicExecutor, output: &OutputHandle, context: &Context<'static>) -> Poll<TaggedValue> {
	executor.tree().eval_tagged_value(output.node(), context).unwrap()
}

fn frame(output: Poll<TaggedValue>) -> (Poll<()>, RenderOutputType) {
	let status = output.clone().map(|_| ());
	let Some(TaggedValue::RenderOutput(RenderOutput { data, .. })) = output.value() else {
		panic!("{status:?}")
	};
	(status, data)
}

fn texture(data: RenderOutputType) -> Texture {
	let RenderOutputType::Texture(texture) = data else { panic!("The frame is a texture") };
	texture
}

fn colors(gpu: &WgpuExecutor, texture: &Texture, view: DAffine2) -> impl Fn(DVec2) -> SRGBA8 + use<> {
	let resolution = resolution();
	let pixels = gpu.read_texture_pixels(texture).unwrap();
	move |point: DVec2| {
		let pixel = (view.transform_point2(point) * SCALE).as_uvec2();
		let [red, green, blue, alpha] = pixels.as_chunks::<4>().0[(pixel.y * resolution.x + pixel.x) as usize];
		SRGBA8::new(red, green, blue, alpha)
	}
}

fn drawn_over_the_backdrop(gpu: &WgpuExecutor, scene: &GraphUiScene, view: DAffine2) -> impl Fn(DVec2) -> SRGBA8 + use<> {
	drawn_graph_ui_over_the_backdrop(gpu, graph(scene, config(view, 1.)))
}

fn drawn_graph_ui_over_the_backdrop(gpu: &WgpuExecutor, graph_ui: GraphUi) -> impl Fn(DVec2) -> SRGBA8 + use<> {
	let view = graph_ui.config.view;
	let (executor, output) = executor(graph_ui_network(gpu));
	let (status, data) = frame(evaluated(&executor, &output, &context(render_config(ExportFormat::Raster), graph_ui)));
	assert_eq!(status, Poll::Final(()));
	colors(gpu, &texture(data), view)
}

fn layer_drawn(gpu: &WgpuExecutor, change: &dyn Fn(&mut Layer)) -> impl Fn(DVec2) -> SRGBA8 + use<> {
	let mut shown = layer(1, (0, 0), "Layer");
	change(&mut shown);
	let mut edit = GraphUiScene::default().edit();
	edit.layers.put(shown);
	drawn_over_the_backdrop(gpu, &edit.finish().0, DAffine2::from_translation(DVec2::new(-60., 0.)))
}

fn dot(key: u64, (x, y): (i32, i32), data_type: DataType) -> Dot {
	Dot {
		key: NodeId(key),
		cell: IVec2::new(x, y),
		data_type,
		wired: false,
		selected: false,
		previewed: false,
		highlight: Highlight::None,
		output_highlight: Highlight::None,
		glide: DVec2::ZERO,
	}
}

fn line(from: (f64, f64), to: (f64, f64)) -> BezPath {
	let mut path = BezPath::new();
	path.move_to(from);
	path.line_to(to);
	path
}

#[test]
fn cards_are_drawn_over_the_backdrop_where_the_view_puts_them() {
	let gpu = gpu();
	let (executor, output) = executor(graph_ui_network(&gpu));
	let render = |scene: &GraphUiScene, view: DAffine2, fade_artwork: f64| {
		let graph_ui = graph(scene, config(view, fade_artwork));
		let (status, data) = frame(evaluated(&executor, &output, &context(render_config(ExportFormat::Raster), graph_ui)));
		assert_eq!(status, Poll::Final(()));
		colors(&gpu, &texture(data), view)
	};
	let recorded_cards = || RECORDED_CARDS.with_borrow(|cards| cards.clone().unwrap());

	let mut edit = GraphUiScene::default().edit();
	edit.cards.put(card(1, (1, 0), "A Node With a Very Long Name"));
	let (mut scene, _) = edit.finish();

	let color = render(&scene, DAffine2::IDENTITY, 0.);
	let drawn = recorded_cards().elements_witness();
	assert_eq!(color(TOP_BORDER), BORDER, "on the top border");
	assert_eq!(color(ABOVE), NOTHING, "above the card, where the backdrop is see-through");
	assert_ne!(color(DVec2::new(48., 0.5)), NOTHING, "on a grid dot");
	assert!((56..90).any(|x| color(DVec2::new(x as f64, 24.)).red > 0xc0), "in the name");
	assert!(!branding() || color(IN_THE_ICON) == GENERAL, "in the header icon");
	let mut past_the_cut_name = (140..143).flat_map(|x| (16..32).map(move |y| DVec2::new(x as f64, y as f64)));
	assert!(past_the_cut_name.all(|point| color(point) == color(DVec2::new(142., 24.))), "past the cut name");

	let zoomed_in = DAffine2::from_scale_angle_translation(DVec2::splat(2.), 0., DVec2::new(-20., 4.));
	let color = render(&scene, zoomed_in, 0.);
	assert!(drawn.matches(&recorded_cards()), "the cards of a scene are drawn once for every view");
	assert_eq!(color(TOP_BORDER), BORDER, "on the top border, zoomed in");
	assert_eq!(color(ABOVE), NOTHING, "above the card, zoomed in");
	assert_eq!(color(NEXT_TOP_BORDER), NOTHING, "where the next card goes");

	let mut edit = scene.clone().edit();
	edit.cards.put(card(2, (1, 1), "Path"));
	let (_, delta) = edit.finish();
	scene.apply(delta);
	let color = render(&scene, zoomed_in, 0.);
	assert!(!drawn.matches(&recorded_cards()), "the cards are drawn again when the scene changes");
	assert_eq!(color(NEXT_TOP_BORDER), BORDER, "on the top border of the next card");
	let drawn = recorded_cards().elements_witness();

	let color = render(&scene, zoomed_in, 1.);
	assert!(drawn.matches(&recorded_cards()), "the cards of a scene are drawn once for every fade");
	assert_eq!(color(ABOVE), BACKDROP, "above the card, with an opaque backdrop");
	assert_eq!(color(NEXT_TOP_BORDER), BORDER, "on the top border, with an opaque backdrop");

	let zoomed_out = DAffine2::from_scale(DVec2::splat(0.25));
	let color = render(&scene, zoomed_out, 0.);
	assert_ne!(color(DVec2::new(48., 0.)), NOTHING, "on a grid dot, zoomed out");
	assert_eq!(color(DVec2::new(24., 0.)), NOTHING, "where a grid dot is left out, zoomed out");

	let connector = |name: &str, data_type, connected| Connector {
		name: name.to_string(),
		data_type,
		connected,
		highlight: Highlight::None,
	};
	let mut edit = GraphUiScene::default().edit();
	edit.cards.put(Card {
		primary_output: Some(connector("Shape", DataType::Vector, true)),
		secondary_inputs: vec![connector("Width", DataType::Number, false)],
		secondary_outputs: vec![connector("Area", DataType::Number, false), connector("Perimeter", DataType::Number, true)],
		..card(1, (1, 0), "Rectangle")
	});
	let (rows, _) = edit.finish();
	let color = render(&rows, DAffine2::IDENTITY, 1.);
	assert_eq!(color(DVec2::new(30., 83.5)), BORDER, "on the bottom border below the rows");
	assert_ne!(color(DVec2::new(90., 48.)), color(DVec2::new(90., 30.)), "below the header");
	assert!((32..70).any(|x| color(DVec2::new(x as f64, 48.)).red > 0xc0), "in the name of an input");
	assert!((52..70).all(|x| color(DVec2::new(x as f64, 72.)) == color(DVec2::new(90., 48.))), "left of the name of an output");
	assert!((90..136).any(|x| color(DVec2::new(x as f64, 72.)).red > 0xc0), "in the name of an output");
	assert_eq!(color(DVec2::new(146., 24.)), SRGBA8::new_opaque(0x65, 0xbb, 0xe5), "in the connected primary output");
	assert_eq!(color(DVec2::new(24., 48.)), SRGBA8::new_opaque(0x88, 0x6b, 0x60), "in the unconnected input");
	assert_eq!(color(DVec2::new(146., 72.)), SRGBA8::new_opaque(0xc9, 0xa6, 0x99), "in the connected output");
	assert_eq!(color(DVec2::new(24., 24.)), color(DVec2::new(24.5, 30.)), "on the left border, without a primary input");
	assert_eq!(color(DVec2::new(24.5, 41.)), color(DVec2::new(26.5, 41.)), "in the gap of the left border around an input");
	assert_eq!(color(DVec2::new(24.5, 38.)), BORDER, "on the left border above the gap");
	assert_eq!(color(DVec2::new(24.5, 66.)), BORDER, "on the left border beside an output");
	assert_eq!(color(DVec2::new(143.5, 41.)), color(DVec2::new(141.5, 41.)), "in the gap of the right border around an output");

	let mut edit = rows.edit();
	edit.cards.update(&NodeId(1), |card| card.selection = Selection::Inactive);
	let (selected, _) = edit.finish();
	let (header, secondary) = (DVec2::new(30., 16.), DVec2::new(30., 66.));
	let inactive = render(&selected, DAffine2::IDENTITY, 1.);
	assert!(color(header).red < inactive(header).red, "in the header of a card in an inactive selection");
	assert!(color(secondary).red < inactive(secondary).red, "in the rows of a card in an inactive selection");
	let mut edit = selected.edit();
	edit.cards.update(&NodeId(1), |card| card.selection = Selection::Active);
	let active = render(&edit.finish().0, DAffine2::IDENTITY, 1.);
	assert!(inactive(header).red < active(header).red, "in the header of a card in the active selection");
	assert_eq!(active(secondary), inactive(header), "in the rows of a card in the active selection");

	let mut edit = scene.edit();
	edit.cards.update(&NodeId(1), |card| card.visible = false);
	let (hidden, _) = edit.finish();
	let color = render(&hidden, DAffine2::IDENTITY, 1.);
	assert!(color(DVec2::new(141., 18.)).red > color(DVec2::new(141., 42.)).red, "in the header of a hidden card");
	let brightest = (56..90).map(|x| color(DVec2::new(x as f64, 24.)).red).max().unwrap();
	assert!((0x90..=0xaa).contains(&brightest), "in the name of a hidden card");
	assert!(!branding() || color(IN_THE_ICON) == GENERAL, "in the header icon of a hidden card");

	let mut edit = hidden.edit();
	edit.cards.update(&NodeId(2), |card| card.previewed = true);
	let (previewed, _) = edit.finish();
	let color = render(&previewed, DAffine2::IDENTITY, 1.);
	let left_border = || (37..60).map(|y| DVec2::new(24.5, y as f64));
	assert!(left_border().any(|point| color(point) == PREVIEWED_BORDER), "on a dash of the left border of a previewed card");
	assert!(left_border().any(|point| color(point) == color(point + DVec2::X * 2.)), "between the dashes of the left border");
	let top_border = |x: f64| color(DVec2::new(x, 36.5));
	assert_eq!([26.25, 27.5, 28.75].map(top_border), [PREVIEWED_BORDER; 3], "on the first dash of the top border");
	assert_ne!(top_border(29.25), PREVIEWED_BORDER, "after the first dash of the top border");
}

#[test]
fn every_grid_dot_is_the_same_disc_of_whole_pixels_at_the_device_pixel_nearest_its_place() {
	let gpu = gpu();
	let views: [(f64, DVec2, f64, &[i32]); 3] = [
		(0.4, DVec2::ZERO, 48., &[2, 2]),
		(0.8, DVec2::new(3.05, 7.7), 24., &[2, 4, 4, 2]),
		(2., DVec2::new(3.3, 7.7), 24., &[4, 6, 6, 6, 6, 4]),
	];
	for (zoom, pan, spacing, row_widths) in views {
		let view = DAffine2::from_scale_angle_translation(DVec2::splat(zoom), 0., pan);
		let color = drawn_over_the_backdrop(&gpu, &GraphUiScene::default(), view);
		let pixel = |at: IVec2| color(view.inverse().transform_point2((at.as_dvec2() + 0.5) / SCALE));
		let size = row_widths.len() as i32;
		let disc: Vec<IVec2> = row_widths
			.iter()
			.zip(0..)
			.flat_map(|(&width, y)| ((size - width) / 2..(size + width) / 2).map(move |x| IVec2::new(x, y)))
			.collect();

		let mut dots = 0;
		for place in (0..10).flat_map(|y| (0..10).map(move |x| DVec2::new(x as f64, y as f64) * spacing)) {
			let corner = view.transform_point2(place) * SCALE - size as f64 / 2.;
			let [min, max] = [corner.round().as_ivec2() - 2, corner.round().as_ivec2() + size + 2];
			if min.min_element() < 0 || max.cmpgt(resolution().as_ivec2()).any() {
				continue;
			}
			let lit: Vec<IVec2> = (min.y..max.y).flat_map(|y| (min.x..max.x).map(move |x| IVec2::new(x, y))).filter(|&p| pixel(p) != BACKDROP).collect();
			let top_left = lit.iter().copied().reduce(IVec2::min).expect("a grid dot lights pixels");
			assert!(lit.iter().all(|&p| pixel(p) == pixel(lit[0])), "a grid dot is one colour at zoom {zoom}");
			assert_eq!(lit.iter().map(|&p| p - top_left).collect::<Vec<_>>(), disc, "a grid dot is a disc of whole pixels at zoom {zoom}");
			assert!(
				(top_left.as_dvec2() - corner).abs().max_element() <= 0.5 + 1. / 64.,
				"a grid dot is at the device pixel nearest its place at zoom {zoom}"
			);
			dots += 1;
		}
		assert!(dots >= 4, "the viewport shows grid dots at zoom {zoom}");
	}
}

#[test]
fn redirect_dots_are_discs_in_the_colour_of_their_data_type_dim_while_unwired() {
	let gpu = gpu();
	let mut edit = GraphUiScene::default().edit();
	edit.dots.put(Dot {
		wired: true,
		..dot(1, (1, 0), DataType::Vector)
	});
	edit.dots.put(dot(2, (3, 1), DataType::Color));
	edit.dots.put(Dot {
		wired: true,
		selected: true,
		..dot(3, (1, 2), DataType::Number)
	});
	edit.dots.put(Dot {
		selected: true,
		previewed: true,
		..dot(4, (4, 2), DataType::Number)
	});
	let (scene, _) = edit.finish();

	let color = drawn_over_the_backdrop(&gpu, &scene, DAffine2::IDENTITY);
	assert_eq!(color(DVec2::new(24., 24.)), VECTOR, "in the middle of a wired dot");
	assert_eq!(color(DVec2::new(20., 22.)), VECTOR, "inside the edge of a wired dot");
	assert_eq!(color(DVec2::new(30., 24.)), BACKDROP, "right of a wired dot");
	assert_eq!(color(DVec2::new(24., 30.)), BACKDROP, "below a wired dot");
	assert_eq!(color(DVec2::new(72., 48.)), COLOR_DIM, "in the middle of an unwired dot");

	let half_white = |color: SRGBA8| (0x8e..=0x92).contains(&color.red) && color.red == color.blue;
	assert!(half_white(color(DVec2::new(30., 72.))), "on the ring around a selected dot: {:?}", color(DVec2::new(30., 72.)));
	assert_eq!(color(DVec2::new(32., 72.)), BACKDROP, "outside the ring around a selected dot");
	let near = |a: SRGBA8, b: SRGBA8| [a.red.abs_diff(b.red), a.green.abs_diff(b.green), a.blue.abs_diff(b.blue)].into_iter().all(|delta| delta <= 8);
	let ring: Vec<SRGBA8> = (0..64)
		.map(|step| color(DVec2::new(96., 72.) + 7.5 * DVec2::from_angle(step as f64 / 64. * std::f64::consts::TAU)))
		.collect();
	assert!(ring.iter().any(|&color| near(color, NUMBER)), "on a dash of the ring around a previewed dot, even though it is unwired");
	assert!(ring.iter().any(|&color| near(color, BACKDROP)), "between the dashes of the ring around a previewed dot");
	assert_eq!(color(DVec2::new(102., 72.)), BACKDROP, "where a selected dot has its ring, around a previewed one");
}

#[test]
fn a_hovered_redirect_dot_grows_with_a_ring_and_shows_its_output_arrow_where_a_wire_is_pulled_out() {
	let gpu = gpu();
	let highlighted = |key, cell, data_type, highlight, output_highlight| Dot {
		wired: true,
		selected: true,
		highlight,
		output_highlight,
		..dot(key, cell, data_type)
	};
	let mut edit = GraphUiScene::default().edit();
	edit.dots.put(highlighted(1, (1, 0), DataType::Vector, Highlight::Hovered, Highlight::None));
	edit.dots.put(highlighted(2, (4, 0), DataType::Color, Highlight::None, Highlight::Hovered));
	edit.dots.put(highlighted(3, (1, 2), DataType::Number, Highlight::None, Highlight::WireSource));
	edit.dots.put(Dot {
		wired: false,
		..highlighted(4, (4, 2), DataType::Number, Highlight::WireTarget, Highlight::None)
	});
	let color = drawn_over_the_backdrop(&gpu, &edit.finish().0, DAffine2::IDENTITY);

	assert_eq!(color(DVec2::new(29.8, 24.)), VECTOR, "inside the grown disc of a hovered dot");
	assert_eq!(
		color(DVec2::new(32., 24.)),
		VECTOR_DIM,
		"on the ring of a hovered dot, in its dim colour instead of the selection's white"
	);
	assert_eq!(color(DVec2::new(34.5, 24.)), BACKDROP, "beyond the ring of a hovered dot");

	assert_eq!(color(DVec2::new(96., 24.)), COLOR_DIM, "in a dot with its output hovered, dimmed");
	assert_eq!(color(DVec2::new(106., 24.)), COLOR, "in the output arrow beside it, bright");
	assert_eq!(color(DVec2::new(112., 24.)), COLOR_DIM, "on the ring beyond the tip of the hovered output arrow");

	assert_eq!(color(DVec2::new(24., 72.)), NUMBER_DIM, "in a dot a wire is pulled out of, dimmed");
	assert_eq!(color(DVec2::new(34., 72.)), NUMBER, "in the output arrow the wire comes out of, bright");
	assert_eq!(color(DVec2::new(39.5, 72.)), BACKDROP, "beyond the tip of that arrow, which keeps its size");

	assert_eq!(color(DVec2::new(104., 72.)), NUMBER_DIM, "on the ring of the dot a wire snaps to");
}

#[test]
fn wires_are_drawn_in_the_dim_colour_of_their_data_type_over_the_layers_and_under_the_cards() {
	let gpu = gpu();
	let mut edit = GraphUiScene::default().edit();
	edit.layers.put(layer(1, (0, 0), "Layer"));
	edit.cards.put(card(2, (10, 0), "Path"));
	edit.wires.put(wire(WireKey::Input { node: NodeId(1), index: 1 }, line((0., 44.), (200., 44.)), DataType::Vector));
	edit.wires.put(wire(WireKey::Export(0), line((230., 30.), (380., 30.)), DataType::Vector));
	let scene = edit.finish().0;
	let color = drawn_over_the_backdrop(&gpu, &scene, DAffine2::IDENTITY);
	assert_eq!(color(DVec2::new(6., 44.)), VECTOR_DIM, "on a wire, left of the layer");
	assert_eq!(color(DVec2::new(150., 44.)), VECTOR_DIM, "on a wire over the layer");
	assert_eq!(
		[44.75, 45.25].map(|y| color(DVec2::new(150., y))),
		[VECTOR_DIM, color(DVec2::new(150., 40.))],
		"on the lower edge of a wire and beside it, two pixels wide, over the layer"
	);

	let color = drawn_over_the_backdrop(&gpu, &scene, DAffine2::from_translation(DVec2::new(-220., 0.)));
	assert_eq!(color(DVec2::new(232., 30.)), VECTOR_DIM, "on a wire, left of the card");
	let under_the_card = color(DVec2::new(340., 30.));
	assert!(
		under_the_card.red < VECTOR_DIM.red && under_the_card.blue > BACKDROP.blue,
		"on a wire under the card: {under_the_card:?}"
	);
}

#[test]
fn a_stack_wire_is_drawn_thick_under_the_layers_and_split_down_its_middle() {
	let gpu = gpu();
	let mut edit = GraphUiScene::default().edit();
	edit.layers.put(layer(1, (0, 0), "Layer"));
	edit.wires.put(Wire {
		thick: true,
		center_line: line((60., -12.), (60., 92.)),
		..wire(WireKey::Input { node: NodeId(1), index: 0 }, line((60., -10.), (60., 90.)), DataType::Graphic)
	});
	let color = drawn_over_the_backdrop(&gpu, &edit.finish().0, DAffine2::IDENTITY);
	assert_eq!(
		[56.25, 64.25].map(|x| color(DVec2::new(x, 70.))),
		[GRAPHIC_DIM, BACKDROP],
		"on the left edge of a stack wire and right of it, eight pixels wide"
	);
	assert_eq!(color(DVec2::new(60., 70.)), WIRE_SPLIT, "on the line down the middle of a stack wire");
	assert_eq!(color(DVec2::new(60., 24.)), CHECKER_DARK, "on the checkerboard of the thumbnail, which covers the stack wire");
}

#[test]
fn a_list_wire_is_drawn_as_two_lines_side_by_side() {
	let gpu = gpu();
	let mut edit = GraphUiScene::default().edit();
	edit.wires.put(Wire {
		list: true,
		..wire(WireKey::Export(0), line((10., 30.), (150., 30.)), DataType::Graphic)
	});
	let color = drawn_over_the_backdrop(&gpu, &edit.finish().0, DAffine2::IDENTITY);
	assert_eq!(color(DVec2::new(60., 28.25)), GRAPHIC_DIM, "on the upper line");
	assert_eq!(color(DVec2::new(60., 30.)), WIRE_SPLIT, "between the lines");
	assert_eq!(color(DVec2::new(60., 31.5)), GRAPHIC_DIM, "on the lower line");
	assert_eq!(color(DVec2::new(60., 32.25)), BACKDROP, "below the lines, four pixels apart");
}

#[test]
fn the_wire_to_the_export_from_a_previewed_node_is_dashed_in_the_bright_colour_of_its_data_type() {
	let gpu = gpu();
	let mut edit = GraphUiScene::default().edit();
	edit.wires.put(wire(WireKey::Preview, line((10., 30.), (150., 30.)), DataType::Vector));
	let color = drawn_over_the_backdrop(&gpu, &edit.finish().0, DAffine2::IDENTITY);
	assert_eq!(
		[10.25, 12.75, 15.25].map(|x| color(DVec2::new(x, 30.))),
		[VECTOR; 3],
		"on both ends of the first dash, three pixels long, and the start of the next"
	);
	assert_eq!(
		[13.25, 14.75].map(|x| color(DVec2::new(x, 30.))),
		[BACKDROP; 2],
		"on both ends of the gap between them, two pixels long"
	);
}

#[test]
fn the_wire_a_dragged_node_would_be_inserted_on_is_drawn_in_the_bright_colour_of_its_data_type() {
	let gpu = gpu();
	let mut edit = GraphUiScene::default().edit();
	edit.wires.put(Wire {
		insertion_target: true,
		..wire(WireKey::Export(0), line((10., 30.), (150., 30.)), DataType::Color)
	});
	edit.wires.put(wire(WireKey::Export(1), line((10., 60.), (150., 60.)), DataType::Color));
	let color = drawn_over_the_backdrop(&gpu, &edit.finish().0, DAffine2::IDENTITY);
	assert_eq!(color(DVec2::new(60., 30.)), COLOR, "on the wire the node would be inserted on");
	assert_eq!(color(DVec2::new(60., 60.)), COLOR_DIM, "on another wire");
}

#[test]
fn the_wire_in_progress_is_drawn_dim_over_the_wires_and_under_the_cards() {
	let gpu = gpu();
	let mut edit = GraphUiScene::default().edit();
	edit.cards.put(card(1, (1, 2), "Path"));
	edit.wires.put(wire(WireKey::Export(0), line((15., 10.), (15., 50.)), DataType::Color));
	let path = [
		PathEl::MoveTo((10., 30.).into()),
		PathEl::LineTo((20., 30.).into()),
		PathEl::CurveTo((40., 30.).into(), (30., 70.).into(), (50., 70.).into()),
		PathEl::LineTo((150., 70.).into()),
	];
	let config = GraphUiConfig {
		wire_in_progress: Some(WireInProgress { path, data_type: DataType::Number }),
		..config(DAffine2::IDENTITY, 1.)
	};
	let color = drawn_graph_ui_over_the_backdrop(&gpu, graph(&edit.finish().0, config));
	assert_eq!(color(DVec2::new(12., 30.)), NUMBER_DIM, "on the wire in progress");
	assert_eq!(
		[30.75, 31.25].map(|y| color(DVec2::new(12., y))),
		[NUMBER_DIM, BACKDROP],
		"on the lower edge of the wire in progress and beside it, two pixels wide"
	);
	assert_eq!(color(DVec2::new(147., 70.)), NUMBER_DIM, "on the loose end of the wire in progress");
	let under_the_card = color(DVec2::new(130., 70.));
	assert!(under_the_card.red < NUMBER_DIM.red && under_the_card.red > BACKDROP.red, "under the card: {under_the_card:?}");
	assert_eq!(color(DVec2::new(15., 20.)), COLOR_DIM, "on a wire");
	assert_eq!(color(DVec2::new(15., 30.)), NUMBER_DIM, "where the wire in progress crosses it");
}

#[test]
fn the_connectors_of_a_card_are_drawn_over_later_cards_and_dots() {
	let gpu = gpu();
	let mut edit = GraphUiScene::default().edit();
	edit.cards.put(Card {
		primary_output: Some(Connector {
			connected: true,
			..connector("", DataType::Vector)
		}),
		..card(1, (0, 0), "Path")
	});
	edit.cards.put(card(2, (5, 0), "Fill"));
	edit.dots.put(Dot {
		wired: true,
		..dot(3, (5, 0), DataType::Color)
	});
	let color = drawn_over_the_backdrop(&gpu, &edit.finish().0, DAffine2::IDENTITY);
	assert_eq!(color(DVec2::new(122., 24.)), VECTOR, "in the output connector, over the card and the dot drawn after it");
	assert_eq!(color(DVec2::new(116., 24.)), COLOR, "in the dot, beside the connector");
}

#[test]
fn a_hovered_connector_is_drawn_bright_and_enlarged_with_a_ring_of_its_dim_colour() {
	let gpu = gpu();
	let highlighted = |data_type, highlight| Connector {
		highlight,
		..connector("", data_type)
	};
	let render = |highlight| {
		let mut edit = GraphUiScene::default().edit();
		edit.cards.put(Card {
			primary_output: Some(highlighted(DataType::Vector, highlight)),
			..card(1, (1, 0), "Path")
		});
		edit.layers.put(Layer {
			primary_output: Some(highlighted(DataType::Graphic, highlight)),
			..layer(2, (0, 3), "Layer")
		});
		edit.network_connectors.put(NetworkConnector {
			add_highlight: highlight,
			..network_connector(NetworkConnectorKind::Import, 0, (132, 48), None)
		});
		drawn_over_the_backdrop(&gpu, &edit.finish().0, DAffine2::IDENTITY)
	};
	let [resting, hovered] = [render(Highlight::None), render(Highlight::Hovered)];

	let [middle, beyond_the_tip] = [DVec2::new(145., 24.), DVec2::new(152., 24.)];
	assert_eq!([resting(middle), resting(beyond_the_tip)], [VECTOR_DIM, BACKDROP], "a connector at rest");
	assert_eq!(hovered(middle), VECTOR, "in a hovered connector, bright though unconnected");
	assert_eq!(hovered(beyond_the_tip), VECTOR_DIM, "on the ring beyond the tip of the enlarged arrow");

	let [middle, above] = [DVec2::new(48., 68.4), DVec2::new(48., 62.5)];
	assert_eq!([resting(middle), resting(above)], [GRAPHIC_DIM, BACKDROP], "a stack connector at rest");
	assert_eq!([hovered(middle), hovered(above)], [GRAPHIC, GRAPHIC_DIM], "a hovered stack connector, enlarged about its own middle");

	let [middle, beyond_the_tip] = [DVec2::new(133., 48.), DVec2::new(140., 48.)];
	assert_eq!([resting(middle), resting(beyond_the_tip)], [GENERAL_DIM, BACKDROP], "a connector for adding an import at rest");
	assert_eq!([hovered(middle), hovered(beyond_the_tip)], [GENERAL, GENERAL_DIM], "a hovered connector for adding an import");
}

#[test]
fn the_connector_a_wire_is_dragged_from_is_bright_and_the_one_it_snaps_to_enlarged() {
	let gpu = gpu();
	let highlighted = |highlight| Connector {
		highlight,
		..connector("", DataType::Number)
	};
	let mut edit = GraphUiScene::default().edit();
	edit.cards.put(Card {
		primary_input: Some(highlighted(Highlight::WireTarget)),
		primary_output: Some(highlighted(Highlight::WireSource)),
		..card(1, (1, 0), "Path")
	});
	let color = drawn_over_the_backdrop(&gpu, &edit.finish().0, DAffine2::IDENTITY);
	assert_eq!(color(DVec2::new(145., 24.)), NUMBER, "in the output the wire is dragged from, bright though unconnected");
	assert_eq!(color(DVec2::new(152., 24.)), BACKDROP, "beyond the tip of the output, which keeps its size");
	assert_eq!(color(DVec2::new(25., 24.)), NUMBER, "in the input the wire snaps to");
	assert_eq!(color(DVec2::new(32., 24.)), NUMBER_DIM, "on the ring beyond the tip of the enlarged input");
}

#[test]
fn a_layer_is_a_rounded_body_with_its_name_and_a_border_left_out_around_the_thumbnail_and_the_visibility_button() {
	let gpu = gpu();
	let mut edit = GraphUiScene::default().edit();
	edit.layers.put(layer(1, (0, 0), "Layer"));
	let (scene, _) = edit.finish();

	let color = drawn_over_the_backdrop(&gpu, &scene, DAffine2::IDENTITY);
	let fill = color(DVec2::new(150., 40.));
	assert!(fill.red < BACKDROP.red && fill.red == fill.blue, "inside the layer: {fill:?}");
	assert_eq!(color(DVec2::new(100., 0.5)), BORDER, "on the top border right of the thumbnail");
	assert_eq!(color(DVec2::new(100., 47.5)), BORDER, "on the bottom border right of the thumbnail");
	assert_eq!(color(DVec2::new(87., 0.5)), fill, "on the top edge right of the thumbnail");
	assert_eq!(color(DVec2::new(12.5, 24.)), fill, "on the left edge beside the thumbnail");
	assert_eq!(color(DVec2::new(12.5, 0.5)), BACKDROP, "outside the rounded top left corner");
	assert_eq!(color(DVec2::new(13.5, 24.)), BORDER, "on the left border of the thumbnail");
	assert_eq!(color(DVec2::new(82.5, 24.)), BORDER, "on the right border of the thumbnail");
	assert_eq!(color(DVec2::new(40., 0.5)), BORDER, "on the top border of the thumbnail");
	assert_eq!(color(DVec2::new(14.5, 24.)), BACKDROP, "between the border of the thumbnail and its checkerboard");
	let checkerboard = [(15.5, 2.5), (23.5, 2.5), (15.5, 10.5), (23.5, 10.5), (80.5, 45.5)].map(|(x, y)| color(DVec2::new(x, y)));
	assert_eq!(
		checkerboard,
		[CHECKER_LIGHT, CHECKER_DARK, CHECKER_DARK, CHECKER_LIGHT, CHECKER_DARK],
		"on the checkerboard of the thumbnail, starting light, its last cells cut"
	);
	assert_eq!(color(DVec2::new(8., 24.)), BACKDROP, "left of the layer");
	assert!((92..130).any(|x| color(DVec2::new(x as f64, 24.)).red > 0xc0), "in the name");

	let color = drawn_over_the_backdrop(&gpu, &scene, DAffine2::from_translation(DVec2::new(-40., 10.)));
	assert_eq!(color(DVec2::new(191.5, 10.)), BORDER, "on the right border above the visibility button");
	assert_eq!(color(DVec2::new(191.5, 13.)), fill, "on the right edge behind the visibility button");
	assert_eq!(color(DVec2::new(194., 14.)), BACKDROP, "right of the layer");

	let mut edit = scene.edit();
	edit.layers.put(Layer {
		chain_width: 2,
		..layer(1, (3, 0), "Layer")
	});
	edit.layers.put(Layer {
		chain_width: 2,
		has_left_input_wire: true,
		..layer(2, (3, 2), "Layer")
	});
	let color = drawn_over_the_backdrop(&gpu, &edit.finish().0, DAffine2::IDENTITY);
	assert_eq!(color(DVec2::new(20., 30.)), BACKDROP, "left of the chain");
	assert_eq!(color(DVec2::new(30., 30.)), fill, "in the chain");
	assert_eq!(color(DVec2::new(24.5, 30.)), BORDER, "on the left border of the chain");
	assert_eq!(color(DVec2::new(24.5, 74.)), fill, "where a wire enters the chain");
	assert_eq!(color(DVec2::new(24.5, 62.)), BORDER, "on the left border above where a wire enters the chain");
}

#[test]
fn a_layer_has_connectors_above_and_below_its_thumbnail_and_on_its_left_side() {
	let gpu = gpu();
	let mut edit = GraphUiScene::default().edit();
	edit.layers.put(Layer {
		primary_output: Some(Connector {
			connected: true,
			..connector("", DataType::Graphic)
		}),
		secondary_input: Some(connector("", DataType::Vector)),
		primary_output_connected_to_layer: true,
		..layer(1, (1, 1), "Layer")
	});
	edit.cards.put(card(2, (-2, 1), "Covering"));
	let (scene, _) = edit.finish();

	let color = drawn_over_the_backdrop(&gpu, &scene, DAffine2::IDENTITY);
	assert_eq!(color(DVec2::new(72., 22.)), GRAPHIC, "in the connected stack output");
	assert_eq!(color(DVec2::new(69.5, 14.)), GRAPHIC_DIM, "in the end of the wire to the layer above");
	assert_eq!(color(DVec2::new(72., 14.)), BACKDROP, "in the middle of the end of the wire to the layer above");
	assert_eq!(color(DVec2::new(69.5, 74.)), GENERAL_DIM, "in the stack input, without an input as General and dim");
	assert_eq!(color(DVec2::new(69.5, 82.)), BACKDROP, "below the stack input, unconnected so without a wire end");
	assert_eq!(color(DVec2::new(23., 48.)), VECTOR_DIM, "in the unconnected input on the left side, above the card covering it");
}

#[test]
fn a_layer_shows_whether_it_is_visible_and_whether_it_is_locked() {
	let gpu = gpu();
	assert!(!branding() || ["EyeVisible", "EyeHidden", "PadlockLocked"].iter().all(|name| graph_ui::icons::icon(name, TEXT).is_some()));
	let [on_the_slash, on_the_padlock] = [DVec2::new(185.5, 30.25), DVec2::new(164.5, 27.)];

	let color = layer_drawn(&gpu, &|_| {});
	let fill = color(DVec2::new(150., 40.));
	assert_eq!(color(on_the_slash), fill, "where the eye of a hidden layer is slashed");
	assert_eq!(color(on_the_padlock), fill, "where the padlock of a locked layer goes");
	assert!(!branding() || (180..204).any(|x| color(DVec2::new(x as f64 + 0.25, 24.25)) == TEXT), "on the eye");

	let color = layer_drawn(&gpu, &|layer| layer.visible = false);
	assert!(!branding() || color(on_the_slash) == TEXT, "on the slashed eye of a hidden layer");

	let color = layer_drawn(&gpu, &|layer| layer.locked = true);
	assert!(!branding() || color(on_the_padlock) == TEXT, "on the padlock of a locked layer");
}

#[test]
fn a_hovered_layer_button_is_backed_by_a_grey_square_and_shows_what_a_click_would_do_until_it_is_clicked() {
	let gpu = gpu();
	let [behind_the_eye, on_the_slash, left_of_the_shackle, on_a_grip_dot] = [DVec2::new(181.5, 13.5), DVec2::new(185.5, 30.25), DVec2::new(165.5, 22.), DVec2::new(174.5, 21.5)];

	let color = layer_drawn(&gpu, &|_| {});
	let fill = color(DVec2::new(150., 40.));
	assert_eq!([color(behind_the_eye), color(on_the_slash), color(on_a_grip_dot)], [fill; 3], "a layer at rest");

	let color = layer_drawn(&gpu, &|layer| layer.hovered_button = Some(LayerButton::Visibility));
	assert_eq!(color(behind_the_eye), HOVERED_BUTTON, "behind the hovered visibility button");
	assert!(!branding() || color(on_the_slash) == TEXT, "on the slash through the eye, as a click would hide the layer");
	let color = layer_drawn(&gpu, &|layer| {
		layer.hovered_button = Some(LayerButton::Visibility);
		layer.hovered_button_clicked = true;
	});
	assert_eq!([color(behind_the_eye), color(on_the_slash)], [HOVERED_BUTTON; 2], "once clicked, the button shows the layer's state");

	let color = layer_drawn(&gpu, &|layer| layer.locked = true);
	assert!(!branding() || color(left_of_the_shackle) == TEXT, "on the closed shackle of the padlock");
	let color = layer_drawn(&gpu, &|layer| {
		layer.locked = true;
		layer.hovered_button = Some(LayerButton::Lock);
	});
	assert_eq!(color(left_of_the_shackle), HOVERED_BUTTON, "where the shackle of the hovered padlock is open");

	let color = layer_drawn(&gpu, &|layer| layer.hovered_button = Some(LayerButton::Grip));
	assert!(color(on_a_grip_dot).red > fill.red + 0x40, "on a dot of the hovered grip of a layer that is not selected");
}

#[test]
fn the_buttons_of_a_layer_in_a_read_only_network_are_greyed_and_do_not_respond_to_hover() {
	let gpu = gpu();
	let [inside, behind_the_eye, on_the_slash] = [DVec2::new(150., 40.), DVec2::new(181.5, 13.5), DVec2::new(185.5, 30.25)];
	let [behind_the_padlock, left_of_the_shackle] = [DVec2::new(157.5, 13.5), DVec2::new(165.5, 22.)];

	let color = layer_drawn(&gpu, &|layer| {
		layer.read_only = true;
		layer.visible = false;
		layer.locked = true;
	});
	assert!(!branding() || color(on_the_slash) == DISABLED_ICON, "on the slashed eye of a hidden layer");
	assert!(!branding() || color(left_of_the_shackle) == DISABLED_ICON, "on the closed shackle of the padlock");

	let color = layer_drawn(&gpu, &|layer| {
		layer.read_only = true;
		layer.hovered_button = Some(LayerButton::Visibility);
	});
	assert_eq!([color(behind_the_eye), color(on_the_slash)], [color(inside); 2], "behind the hovered eye and where it would be slashed");

	let color = layer_drawn(&gpu, &|layer| {
		layer.read_only = true;
		layer.locked = true;
		layer.hovered_button = Some(LayerButton::Lock);
	});
	assert_eq!(color(behind_the_padlock), color(inside), "behind the hovered lock button");
	assert!(!branding() || color(left_of_the_shackle) == DISABLED_ICON, "on the shackle of the hovered padlock, still closed");
}

#[test]
fn a_selected_layer_shows_its_grip() {
	let gpu = gpu();
	let [inside, on_a_grip_dot, on_a_grip_dot_when_locked] = [DVec2::new(150., 40.), DVec2::new(174.5, 21.5), DVec2::new(150.5, 21.5)];

	let color = layer_drawn(&gpu, &|_| {});
	assert_eq!(color(on_a_grip_dot), color(inside), "where the grip of a selected layer shows");
	let color = layer_drawn(&gpu, &|layer| layer.selection = Selection::Inactive);
	assert!(color(on_a_grip_dot).red > color(inside).red + 0x40, "on a dot of the grip of a layer in an inactive selection");
	let color = layer_drawn(&gpu, &|layer| {
		layer.selection = Selection::Active;
		layer.locked = true;
	});
	assert!(color(on_a_grip_dot_when_locked).red > color(inside).red + 0x40, "on a dot of the grip of a selected and locked layer");
}

#[test]
fn a_selected_layer_is_lighter_in_the_active_selection() {
	let gpu = gpu();
	let inside = DVec2::new(150., 40.);
	let fill = layer_drawn(&gpu, &|_| {})(inside).red;
	let fill_inactive = layer_drawn(&gpu, &|layer| layer.selection = Selection::Inactive)(inside).red;
	let color = layer_drawn(&gpu, &|layer| layer.selection = Selection::Active);
	assert!(fill < fill_inactive && fill_inactive < color(inside).red, "inside a selected layer, lighter in the active selection");
}

#[test]
fn a_hidden_layer_is_lighter_unless_selected_and_has_a_dimmer_name() {
	let gpu = gpu();
	let render = |visible, selection| {
		let mut edit = GraphUiScene::default().edit();
		edit.layers.put(Layer {
			visible,
			selection,
			..layer(1, (0, 0), "Layer")
		});
		drawn_over_the_backdrop(&gpu, &edit.finish().0, DAffine2::IDENTITY)
	};
	let inside = DVec2::new(150., 40.);
	let brightest_in_the_name = |color: &dyn Fn(DVec2) -> SRGBA8| (92..130).map(|x| color(DVec2::new(x as f64, 24.)).red).max().unwrap();

	let [shown, hidden] = [render(true, Selection::None), render(false, Selection::None)];
	assert!(shown(inside).red < hidden(inside).red, "inside a hidden layer");
	assert!(brightest_in_the_name(&shown) > 0xd0, "in the name of a shown layer");
	assert!((0x90..=0xaa).contains(&brightest_in_the_name(&hidden)), "in the name of a hidden layer");
	assert_eq!(
		render(false, Selection::Inactive)(inside),
		render(true, Selection::Inactive)(inside),
		"inside a selected layer, hidden or not"
	);
}

#[test]
fn a_layer_leaves_its_name_undrawn_while_it_is_renamed() {
	let gpu = gpu();
	let along_the_name = |name: &str, renaming| {
		let mut edit = GraphUiScene::default().edit();
		edit.layers.put(Layer { renaming, ..layer(1, (0, 0), name) });
		let color = drawn_over_the_backdrop(&gpu, &edit.finish().0, DAffine2::IDENTITY);
		(92..150).map(|x| color(DVec2::new(x as f64, 24.))).collect::<Vec<_>>()
	};
	let unnamed = along_the_name("", false);

	assert!(
		along_the_name("Layer", false).iter().zip(&unnamed).any(|(named, unnamed)| named.red > unnamed.red + 0x80),
		"in the name"
	);
	assert_eq!(along_the_name("Layer", true), unnamed, "where the name of the renamed layer was");
}

#[test]
fn a_layer_shows_its_thumbnail_stretched_over_the_checkerboard_inside_the_thumbnail_border() {
	let gpu = gpu();
	let red = SRGBA8::new_opaque(0xff, 0x00, 0x00);
	let thumbnail = thumbnail(&gpu, Rect::new(0., 0., 36., 48.), red);
	let mut edit = GraphUiScene::default().edit();
	edit.layers.put(layer(1, (1, 1), "Layer"));
	let scene = edit.finish().0;
	let render = |thumbnails: Vec<(NodeId, Graphic)>, view| {
		drawn_graph_ui_over_the_backdrop(
			&gpu,
			GraphUi {
				thumbnails,
				..graph(&scene, config(view, 1.))
			},
		)
	};
	let [left, right, between_border_and_image, image_edge] = [DVec2::new(45., 48.), DVec2::new(100., 48.), DVec2::new(38.5, 48.), DVec2::new(39.5, 48.)];

	let without = render(Vec::new(), DAffine2::IDENTITY);
	let with = render(vec![(NodeId(1), thumbnail.clone())], DAffine2::IDENTITY);
	assert_ne!(without(left), red, "on the checkerboard of a layer without a thumbnail");
	assert_eq!(with(left), red, "on the opaque half of the thumbnail");
	assert_eq!(with(right), without(right), "on the transparent half of the thumbnail, over the checkerboard");
	assert_eq!(with(image_edge), red, "on the first column of the image, two pixels in from the thumbnail's edge");
	assert_eq!(with(between_border_and_image), without(between_border_and_image), "between the border of the thumbnail and its image");
	assert_eq!(with(between_border_and_image), BACKDROP, "on the background of the thumbnail");

	let zoomed = DAffine2::from_scale_angle_translation(DVec2::splat(1.5), 0., DVec2::new(-30., -20.));
	let with = render(vec![(NodeId(1), thumbnail.clone()), (NodeId(2), thumbnail)], zoomed);
	assert_eq!(
		[with(left), with(image_edge)],
		[red; 2],
		"on the opaque half of the thumbnail, zoomed in, beside a thumbnail of a layer the scene does not have"
	);
	assert_ne!(with(right), red, "on the transparent half of the thumbnail, zoomed in");
}

#[test]
fn a_previewed_layer_has_a_dashed_border() {
	let gpu = gpu();
	let mut edit = GraphUiScene::default().edit();
	edit.layers.put(Layer {
		previewed: true,
		chain_width: 2,
		..layer(1, (3, 1), "Layer")
	});
	let color = drawn_over_the_backdrop(&gpu, &edit.finish().0, DAffine2::IDENTITY);
	let top_border: Vec<SRGBA8> = (0..40).map(|step| color(DVec2::new(40. + step as f64 / 2., 24.5))).collect();
	assert!(top_border.contains(&PREVIEWED_BORDER), "on a dash of the top border");
	assert!(top_border.iter().any(|&color| color != PREVIEWED_BORDER && color != BORDER), "between the dashes of the top border");
	let left_border: Vec<SRGBA8> = (0..40).map(|step| color(DVec2::new(24.5, 34. + step as f64 / 2.))).collect();
	assert!(left_border.contains(&PREVIEWED_BORDER), "on a dash of the left border of the chain");
	assert_eq!(color(DVec2::new(80., 24.5)), color(DVec2::new(80., 27.)), "where the thumbnail leaves out the dashed border");
}

#[test]
fn the_imports_and_exports_are_connectors_in_the_colour_of_their_data_type_beside_their_names() {
	let gpu = gpu();
	let shape = Connector {
		connected: true,
		..connector("Shape", DataType::Vector)
	};
	let editable = |index, place, connector| NetworkConnector {
		editable: true,
		..network_connector(NetworkConnectorKind::Import, index, place, Some(connector))
	};
	let mut edit = GraphUiScene::default().edit();
	edit.network_connectors.put(editable(0, (72, 24), shape));
	edit.network_connectors.put(editable(1, (72, 72), connector("Size", DataType::Number)));
	edit.network_connectors
		.put(network_connector(NetworkConnectorKind::Export, 0, (96, 72), Some(connector("Result", DataType::Vector))));
	let color = drawn_over_the_backdrop(&gpu, &edit.finish().0, DAffine2::IDENTITY);
	assert_eq!(color(DVec2::new(72., 24.)), VECTOR, "in the connected primary import");
	assert_eq!(color(DVec2::new(72., 72.)), NUMBER_DIM, "in the unconnected secondary import");
	assert_eq!(color(DVec2::new(74., 48.)), BACKDROP, "in the cell between the primary and the secondary imports");
	assert_eq!(color(DVec2::new(96., 72.)), VECTOR_DIM, "in the unconnected export");
	assert!((24..64).any(|x| color(DVec2::new(x as f64, 24.)).red > 0xc0), "in the name left of the primary import");
	assert!((66..69).chain(78..94).all(|x| color(DVec2::new(x as f64, 24.)) == BACKDROP), "beside the primary import");
	assert!((105..150).any(|x| color(DVec2::new(x as f64, 72.)).red > 0xc0), "in the name right of the export");
	let separator = SRGBA8::new_opaque(0x88, 0x88, 0x88);
	assert_eq!(
		[30., 60., 67.].map(|x| color(DVec2::new(x, 47.5))),
		[separator; 3],
		"on the line between the primary and the secondary names"
	);
	assert_eq!([color(DVec2::new(60., 46.)), color(DVec2::new(60., 49.))], [BACKDROP; 2], "beside the line, a pixel thick");
	assert_eq!(color(DVec2::new(110., 95.5)), BACKDROP, "where a network that cannot be edited has no line");
}

#[test]
fn an_import_leaves_its_name_undrawn_while_it_is_renamed() {
	let gpu = gpu();
	let along_the_name = |name: &str, renaming| {
		let named = Connector {
			connected: true,
			..connector(name, DataType::Vector)
		};
		let mut edit = GraphUiScene::default().edit();
		edit.network_connectors.put(NetworkConnector {
			renaming,
			..network_connector(NetworkConnectorKind::Import, 0, (72, 24), Some(named))
		});
		let color = drawn_over_the_backdrop(&gpu, &edit.finish().0, DAffine2::IDENTITY);
		(20..64).map(|x| color(DVec2::new(x as f64, 24.))).collect::<Vec<_>>()
	};
	let unnamed = along_the_name("", false);

	assert!(
		along_the_name("Shape", false).iter().zip(&unnamed).any(|(named, unnamed)| named.red > unnamed.red + 0x80),
		"in the name"
	);
	assert_eq!(along_the_name("Shape", true), unnamed, "where the name of the renamed import was");
}

#[test]
fn a_white_bar_shows_where_a_dragged_import_or_export_would_be_dropped() {
	let gpu = gpu();
	let mut edit = GraphUiScene::default().edit();
	edit.network_connectors
		.put(network_connector(NetworkConnectorKind::Import, 0, (96, 0), Some(connector("A", DataType::General))));
	edit.network_connectors
		.put(network_connector(NetworkConnectorKind::Import, 1, (96, 48), Some(connector("B", DataType::General))));
	let scene = edit.finish().0;
	let view = DAffine2::from_scale_angle_translation(DVec2::splat(0.5), 0., DVec2::new(10., 20.));
	let dropped_before = |kind| {
		let reorder_bar = Some((kind, DVec2::new(96., 48.)));
		graph(&scene, GraphUiConfig { reorder_bar, ..config(view, 1.) })
	};

	let color = drawn_graph_ui_over_the_backdrop(&gpu, dropped_before(NetworkConnectorKind::Import));
	assert_eq!(
		[40., 60., 86.].map(|x| color(DVec2::new(x, 37.))),
		[SRGBA8::new_opaque(0xff, 0xff, 0xff); 3],
		"on the bar above the secondary import"
	);
	assert_eq!([color(DVec2::new(60., 33.)), color(DVec2::new(60., 41.))], [BACKDROP; 2], "above and below the bar, two pixels thick");
	assert_eq!(color(DVec2::new(30., 37.)), BACKDROP, "left of the bar, fifty pixels wide");

	let color = drawn_graph_ui_over_the_backdrop(&gpu, dropped_before(NetworkConnectorKind::Export));
	assert_eq!(
		[color(DVec2::new(60., 37.)), color(DVec2::new(120., 37.))],
		[BACKDROP, SRGBA8::new_opaque(0xff, 0xff, 0xff)],
		"the bar of a dragged export starts right of the row"
	);
}

#[test]
fn the_selection_box_is_drawn_between_its_corners_in_viewport_pixels_over_the_cards() {
	let gpu = gpu();
	let mut edit = GraphUiScene::default().edit();
	edit.cards.put(card(1, (1, 0), "Path"));
	let scene = edit.finish().0;
	let view = DAffine2::from_scale_angle_translation(DVec2::splat(2.), 0., DVec2::new(-20., 0.));
	let render = |box_selection| {
		let color = drawn_graph_ui_over_the_backdrop(&gpu, graph(&scene, GraphUiConfig { box_selection, ..config(view, 1.) }));
		move |x: f64, y: f64| color(view.inverse().transform_point2(DVec2::new(x, y)))
	};
	let [with_box, without_box] = [render(Some([DVec2::new(100., 60.), DVec2::new(10., 4.)])), render(None)];
	let blue = SRGBA8::new_opaque(0x00, 0xa8, 0xff);

	assert_eq!(
		[(10.5, 30.), (101.5, 30.), (50., 4.5), (50., 61.5)].map(|(x, y)| with_box(x, y)),
		[blue; 4],
		"on the border, a pixel outside the dragged size"
	);
	for (x, y) in [(9.5, 30.), (102.5, 30.), (50., 3.5), (50., 62.5)] {
		assert_eq!(with_box(x, y), without_box(x, y), "just outside the box at {x}, {y}");
	}
	for (x, y) in [(20., 15.), (60., 50.)] {
		let [inside, under] = [with_box(x, y), without_box(x, y)];
		assert!(
			inside.blue > under.blue && inside.red < under.red,
			"inside the box at {x}, {y}, faintly blue: {inside:?} over {under:?}"
		);
	}
}

#[test]
fn an_error_is_a_faded_red_badge_with_its_message_pointing_down_at_its_node_over_the_cards() {
	let gpu = gpu();
	let mut edit = GraphUiScene::default().edit();
	edit.cards.put(card(1, (1, 0), "Path"));
	let (without_error, _) = edit.finish();
	let mut edit = without_error.clone().edit();
	edit.errors.put(badge(1, (1, 2), "Error"));
	let (with_error, _) = edit.finish();
	let [under, color] = [without_error, with_error].map(|scene| drawn_over_the_backdrop(&gpu, &scene, DAffine2::IDENTITY));
	let half_over = |point: DVec2| {
		let channels = |pixel: SRGBA8| [pixel.red, pixel.green, pixel.blue].map(f64::from);
		let [over, below, drawn] = [ERROR_RED, under(point), color(point)].map(channels);
		(0..3).all(|channel| (drawn[channel] - (over[channel] + below[channel]) / 2.).abs() <= 2.)
	};

	assert!(half_over(DVec2::new(68., 18.)), "in the badge over the card, half see-through: {:?}", color(DVec2::new(68., 18.)));
	assert!(half_over(DVec2::new(30., 44.)), "in the badge over the backdrop: {:?}", color(DVec2::new(30., 44.)));
	assert!(half_over(DVec2::new(36., 50.)), "in the tail below the badge: {:?}", color(DVec2::new(36., 50.)));
	assert_eq!(color(DVec2::new(31., 54.)), BACKDROP, "beside the tail");
	assert_eq!(color(DVec2::new(30., 11.)), under(DVec2::new(30., 11.)), "above the badge, a line of text and its padding high");
	assert!((32..64).any(|x| color(DVec2::new(x as f64, 30.)).red < 0x40), "in the message");
}

#[test]
fn a_hovered_error_badge_is_opaque_and_drawn_over_the_other_badges() {
	let gpu = gpu();
	let render = |hovered| {
		let mut edit = GraphUiScene::default().edit();
		edit.errors.put(ErrorBadge { hovered, ..badge(1, (1, 3), "Error") });
		edit.errors.put(ErrorBadge {
			on_layer: true,
			..badge(2, (1, 4), "Another error")
		});
		drawn_over_the_backdrop(&gpu, &edit.finish().0, DAffine2::IDENTITY)
	};
	let [in_the_first, where_they_overlap] = [DVec2::new(30., 68.), DVec2::new(50., 68.)];
	let [faded, hovered] = [render(false), render(true)];
	assert_ne!(faded(in_the_first), ERROR_RED, "in a badge that is not hovered, half see-through");
	assert_eq!(hovered(in_the_first), ERROR_RED, "in the hovered badge");
	assert_ne!(faded(where_they_overlap), ERROR_RED, "where the later badge lies over the first");
	assert_eq!(hovered(where_they_overlap), ERROR_RED, "where the hovered badge lies over the later one");
}

#[test]
fn what_glides_is_drawn_its_glide_away_from_its_place() {
	let gpu = gpu();
	let render = |glide| {
		let mut edit = GraphUiScene::default().edit();
		edit.cards.put(Card { glide, ..card(1, (1, 0), "Path") });
		edit.errors.put(ErrorBadge {
			hovered: true,
			glide,
			..badge(1, (1, 3), "Error")
		});
		drawn_over_the_backdrop(&gpu, &edit.finish().0, DAffine2::IDENTITY)
	};
	let glide = DVec2::new(12., 6.);
	let [in_place, gliding] = [render(DVec2::ZERO), render(glide)];
	assert_eq!(in_place(TOP_BORDER), BORDER, "on the top border of a card in its place");
	assert_ne!(gliding(TOP_BORDER), BORDER, "where the top border of a gliding card would be in its place");
	assert_eq!(gliding(TOP_BORDER + glide), BORDER, "on the top border of a gliding card");
	assert_eq!(gliding(DVec2::new(30., 68.) + glide), ERROR_RED, "in the badge of the gliding node, which follows it");
}

#[test]
fn an_import_or_export_without_a_connector_is_a_dim_connector_for_adding_one() {
	let gpu = gpu();
	let mut edit = GraphUiScene::default().edit();
	edit.network_connectors.put(network_connector(NetworkConnectorKind::Import, 0, (24, 0), None));
	edit.network_connectors.put(network_connector(NetworkConnectorKind::Export, 1, (120, 72), None));
	let color = drawn_over_the_backdrop(&gpu, &edit.finish().0, DAffine2::IDENTITY);
	assert_eq!(color(DVec2::new(26., 0.)), GENERAL_DIM, "in the connector for adding an import");
	assert_eq!(color(DVec2::new(122., 72.)), GENERAL_DIM, "in the connector for adding an export");
}
