use crate::card::{Card, Connector, DataType, Highlight, Selection};
use crate::dashes::{border, dashed};
use crate::dot::Dot;
use crate::error_badge::ErrorBadge;
use crate::layer::{Layer, LayerButton};
use crate::look::{
	BORDER, BOX_SELECTION, BOX_SELECTION_FILL_OPACITY, CARD_OPACITY, CIRCLE_TOLERANCE, CONNECTOR_OUTLINE, DISABLED_ICON, ERROR_OPACITY, ERROR_RED, ERROR_TEXT, GRID_DOT_OPACITY, HIDDEN_CARD,
	HIDDEN_TEXT, HOVERED_BUTTON, LAYER_IN_ACTIVE_SELECTION, LAYER_IN_INACTIVE_SELECTION, PREVIEWED_BORDER, SELECTED_DOT_RING_OPACITY, SEPARATOR, STACK_INPUT_OUTLINE, STACK_INPUT_WIRE_END,
	STACK_OUTPUT_OUTLINE, STACK_OUTPUT_WIRE_END, TEXT, THUMBNAIL_BACKGROUND, WIRE_SPLIT, checkerboard, connector, data_type_colors, drawn_connector, icon, outlined, painted, rounded_rect, text,
};
use crate::metrics::{
	BORDER_WIDTH, CONNECTOR_GAP, CONNECTOR_SIZE, DOT_HOVERED_RING, DOT_OUTPUT_ARROW, DOT_PREVIEWED_RING_OFFSET, DOT_RADIUS, DOT_SELECTED_RING, ERROR_LINE_HEIGHT, ERROR_MAX_WIDTH, ERROR_RADIUS,
	GRID_COLLAPSE_SPACING, GRID_SIZE, HOVERED_CONNECTOR_SCALE, ICON_SIZE, LAYER_BUTTON_RADIUS, LAYER_BUTTON_SIZE, LAYER_GRIP_DOT_COUNT, LAYER_GRIP_DOT_RADIUS, LAYER_GRIP_DOT_SPACING, LAYER_GRIP_DOTS,
	LAYER_RADIUS, LAYER_THUMBNAIL_GAP, LAYER_THUMBNAIL_RADIUS, LIST_WIRE_WIDTH, NODE_ICON_LEFT, NODE_NAME_LEFT, NODE_NAME_MAX_WIDTH, NODE_RADIUS, NODE_ROW_PADDING, NODE_WIDTH, THICK_WIRE_WIDTH,
	WIRE_DASHES, WIRE_WIDTH,
};
use crate::network_connector::{NetworkConnector, reorder_bar};
use crate::render::{graph_ui, render_config};
use crate::replicated_map::Keyed;
use crate::scene::GraphUiScene;
use crate::text::{Labels, Paragraphs};
use crate::wire::{Wire, WireKey};
use core_types::color::SRGBA8;
use core_types::list::{ATTR_GRADIENT_INTERPOLATION, ATTR_GRADIENT_SPREAD, ATTR_OPACITY, Item, List};
use core_types::transform::ApplyTransform;
use core_types::{Color, Ctx, ExtractVarArgs};
use glam::{DAffine2, DVec2, IVec2};
use graphic_types::{CoverPlacement, Coverage, Graphic, Vector, stamp_coverage};
use std::collections::{BTreeMap, HashSet};
use std::convert::Infallible;
use std::sync::{Arc, Mutex, PoisonError};
use text_nodes::TextToVectorError;
use vector_types::vector::style::Stroke;
use vector_types::{Gradient, GradientInterpolation, GradientSpread, GradientStop};
use vello_gpu::kurbo::{BezPath, Circle, Rect, Shape, flatten};

const GRID_DOT_ROUNDING_BIAS: f64 = 1. / 64.;
const WIRE_FLATTENING_TOLERANCE: f64 = 0.02;

fn scene(ctx: &impl ExtractVarArgs) -> &GraphUiScene {
	ctx.vararg(0).ok().and_then(|scene| scene.downcast_ref()).expect("The scene is the only vararg")
}

#[node_macro::node(category(""))]
fn draw_grid(ctx: impl Ctx + ExtractVarArgs) -> List<Vector> {
	let render_config = render_config(&ctx);
	let viewport_size = render_config.viewport.resolution.as_dvec2() / render_config.scale;
	grid_dots(graph_ui(&ctx).config.view, viewport_size, render_config.scale)
}

fn grid_dots(view: DAffine2, viewport_size: DVec2, scale: f64) -> List<Vector> {
	let zoom = view.matrix2.x_axis.length();
	let mut spacing = zoom * GRID_SIZE;
	if !spacing.is_finite() || spacing <= 0. {
		return List::new();
	}
	while spacing < GRID_COLLAPSE_SPACING {
		spacing *= 2.;
	}
	let radius = ((1. + (zoom - 0.5 + 0.001).floor() / 2.) * scale).ceil() as u32;
	let period = spacing * scale;
	let origin = (view.translation * scale - radius as f64).rem_euclid(DVec2::splat(period)) - period - GRID_DOT_ROUNDING_BIAS;

	let dot_color = Color::WHITE.with_alpha(GRID_DOT_OPACITY);
	let stop = |column: u32, color| GradientStop {
		position: column as f64 / period,
		midpoint: 0.5,
		color,
	};
	let mut dots = List::new();
	for ([top, bottom], runs) in disc_columns(radius) {
		let mut bands = BezPath::new();
		for row in 0..=((viewport_size.y * scale - origin.y) / period).ceil() as u32 {
			let y = (origin.y + row as f64 * period).round();
			bands.extend(Rect::new(0., (y + top as f64) / scale, viewport_size.x, (y + bottom as f64) / scale).path_elements(0.));
		}

		let start = runs[0][0];
		let stops = runs.iter().flat_map(|&[left, right]| [stop(left - start, dot_color), stop(right - start, Color::TRANSPARENT)]);
		let mut columns = Item::new_from_element(Gradient::new(stops));
		columns.set_attribute(ATTR_GRADIENT_SPREAD, GradientSpread::Repeat);
		columns.set_attribute(ATTR_GRADIENT_INTERPOLATION, GradientInterpolation::Stepped);
		columns.left_apply_transform(&DAffine2::from_scale_angle_translation(DVec2::splat(spacing), 0., DVec2::new((origin.x + start as f64) / scale, 0.)));

		let mut group = Vector::item_from_bezpath(&bands);
		stamp_coverage(&mut group, Coverage::new_fill(), Graphic::Gradient(columns), CoverPlacement::Above);
		dots.push(group);
	}
	dots
}

fn disc_columns(radius: u32) -> BTreeMap<[u32; 2], Vec<[u32; 2]>> {
	let lit = |column: u32, row: u32| (DVec2::new(column as f64, row as f64) + 0.5 - radius as f64).length() < radius as f64;
	let mut runs_by_rows = BTreeMap::<[u32; 2], Vec<[u32; 2]>>::new();
	for column in 0..2 * radius {
		let top = (0..radius).take_while(|&row| !lit(column, row)).count() as u32;
		let runs = runs_by_rows.entry([top, 2 * radius - top]).or_default();
		match runs.last_mut() {
			Some(run) if run[1] == column => run[1] += 1,
			_ => runs.push([column, column + 1]),
		}
	}
	runs_by_rows
}

type Cache<Row, Texts = ()> = Arc<Mutex<RowCache<Row, Texts>>>;

#[derive(Debug)]
struct RowCache<Row: Keyed, Texts> {
	drawn: BTreeMap<Row::Key, (Row, List<Vector>)>,
	labels: Texts,
}

impl<Row: Keyed, Texts: Default> Default for RowCache<Row, Texts> {
	fn default() -> Self {
		Self {
			drawn: BTreeMap::new(),
			labels: Texts::default(),
		}
	}
}

fn draw_changed_rows<'a, Row: Keyed + Placed + PartialEq + 'a, Texts, Error>(
	rows: impl IntoIterator<Item = &'a Row>,
	cache: &mut RowCache<Row, Texts>,
	draw: impl Fn(&Row, &mut Texts) -> Result<List<Vector>, Error>,
) -> Result<List<Vector>, Error> {
	let mut kept = BTreeMap::new();
	let mut graphic = List::new();
	for row in rows {
		let entry = match cache.drawn.remove(&row.key()).map(|(drawn, drawing)| (drawn.placed_at(row.place()), drawing)) {
			Some(entry) if entry.0 == *row => entry,
			_ => {
				let at_origin = row.clone().placed_at(Default::default());
				let drawing = draw(&at_origin, &mut cache.labels)?;
				(at_origin, drawing)
			}
		};
		let mut placed = entry.1.clone();
		let offset = row.offset();
		if offset != DVec2::ZERO {
			placed.left_apply_transform(&DAffine2::from_translation(offset));
		}
		graphic.extend(placed);
		kept.insert(row.key(), entry);
	}
	cache.drawn = kept;
	Ok(graphic)
}

fn draw_rows<'a, Row: Keyed + Placed + PartialEq + 'a>(rows: impl IntoIterator<Item = &'a Row>, cache: &Cache<Row>, draw: impl Fn(&Row) -> List<Vector>) -> List<Vector> {
	let Ok(graphic) = draw_changed_rows(rows, &mut cache.lock().unwrap_or_else(PoisonError::into_inner), |row, _| Ok::<_, Infallible>(draw(row)));
	graphic
}

trait Placed: Clone {
	fn place(&self) -> (IVec2, DVec2) {
		Default::default()
	}

	fn placed_at(self, _place: (IVec2, DVec2)) -> Self {
		self
	}

	fn offset(&self) -> DVec2 {
		let (cell, glide) = self.place();
		cell.as_dvec2() * GRID_SIZE + glide
	}
}

impl Placed for Wire {}

impl Placed for Card {
	fn place(&self) -> (IVec2, DVec2) {
		(self.cell, self.glide)
	}

	fn placed_at(self, (cell, glide): (IVec2, DVec2)) -> Self {
		Self { cell, glide, ..self }
	}
}

impl Placed for Layer {
	fn place(&self) -> (IVec2, DVec2) {
		(self.cell, self.glide)
	}

	fn placed_at(self, (cell, glide): (IVec2, DVec2)) -> Self {
		Self { cell, glide, ..self }
	}
}

impl Placed for Dot {
	fn place(&self) -> (IVec2, DVec2) {
		(self.cell, self.glide)
	}

	fn placed_at(self, (cell, glide): (IVec2, DVec2)) -> Self {
		Self { cell, glide, ..self }
	}
}

impl Placed for ErrorBadge {
	fn place(&self) -> (IVec2, DVec2) {
		(self.cell, self.glide)
	}

	fn placed_at(self, (cell, glide): (IVec2, DVec2)) -> Self {
		Self { cell, glide, ..self }
	}
}

impl Placed for NetworkConnector {
	fn place(&self) -> (IVec2, DVec2) {
		(self.position, self.glide)
	}

	fn placed_at(self, (position, glide): (IVec2, DVec2)) -> Self {
		Self { position, glide, ..self }
	}

	fn offset(&self) -> DVec2 {
		self.position.as_dvec2() + self.glide
	}
}

#[node_macro::node(category(""), memoize)]
fn draw_layers(ctx: impl Ctx + ExtractVarArgs, #[data] cache: Cache<Layer, Labels>) -> Result<List<Vector>, TextToVectorError> {
	let scene = scene(&ctx);
	let mut cache = cache.lock().unwrap_or_else(PoisonError::into_inner);
	let graphic = draw_changed_rows(scene.layers(), &mut cache, draw_layer)?;

	let shown: HashSet<&str> = scene.layers().iter().map(|layer| layer.name.as_str()).collect();
	cache.labels.retain(|text| shown.contains(text));
	Ok(graphic)
}

fn draw_layer(layer: &Layer, labels: &mut Labels) -> Result<List<Vector>, TextToVectorError> {
	let mut graphic = List::new();
	let [min, max] = layer.body();
	let fill = match (layer.selection, layer.visible) {
		(Selection::Active, _) => LAYER_IN_ACTIVE_SELECTION.into(),
		(Selection::Inactive, _) => LAYER_IN_INACTIVE_SELECTION.into(),
		(Selection::None, true) => Color::BLACK,
		(Selection::None, false) => HIDDEN_CARD.into(),
	};
	graphic.push(painted(rounded_rect(min, max, [LAYER_RADIUS; 4]), Coverage::new_fill(), fill.with_alpha(CARD_OPACITY)));

	if !layer.renaming {
		let [name, _] = layer.name_box();
		let text_color = if layer.visible { TEXT } else { HIDDEN_TEXT };
		graphic.extend(text(&labels.label(&layer.name, layer.name_width())?.outline, name, text_color));
	}

	let [thumbnail_min, thumbnail_max] = layer.thumbnail();
	let thumbnail = rounded_rect(thumbnail_min, thumbnail_max, [LAYER_THUMBNAIL_RADIUS; 4]);
	graphic.push(painted(thumbnail, Coverage::new_fill(), THUMBNAIL_BACKGROUND.into()));
	let thumbnail_border = Vector::item_from_bezpath(&border(thumbnail_min, thumbnail_max, LAYER_THUMBNAIL_RADIUS, &[], false));
	graphic.push(painted(thumbnail_border, Coverage::new_stroke(&Stroke::new(BORDER_WIDTH)), BORDER.into()));
	let [image_min, image_max] = layer.thumbnail_image();
	graphic.extend(checkerboard(image_min, image_max));

	if let Some(output) = &layer.primary_output {
		graphic.extend(connector(&STACK_OUTPUT_OUTLINE, layer.stack_output(), output));
		if output.connected && layer.primary_output_connected_to_layer {
			graphic.push(outlined(&STACK_OUTPUT_WIRE_END, layer.stack_output(), data_type_colors(output.data_type)[1]));
		}
	}
	match &layer.primary_input {
		Some(input) => {
			graphic.extend(connector(&STACK_INPUT_OUTLINE, layer.stack_input(), input));
			if input.connected && layer.primary_input_connected_to_layer {
				graphic.push(outlined(&STACK_INPUT_WIRE_END, layer.stack_input(), data_type_colors(input.data_type)[1]));
			}
		}
		None => graphic.push(outlined(&STACK_INPUT_OUTLINE, layer.stack_input(), data_type_colors(DataType::General)[1])),
	}

	let [block_min, block_max] = layer.thumbnail_block();
	let mut gaps = vec![[block_min - LAYER_THUMBNAIL_GAP, block_max + LAYER_THUMBNAIL_GAP], layer.visibility()];
	gaps.extend(layer.left_input_wire().map(|wire| [wire - CONNECTOR_GAP / 2., wire + CONNECTOR_GAP / 2.]));
	let border = Vector::item_from_bezpath(&border(min, max, LAYER_RADIUS, &gaps, layer.previewed));
	let border_color = if layer.previewed { PREVIEWED_BORDER } else { BORDER };
	graphic.push(painted(border, Coverage::new_stroke(&Stroke::new(BORDER_WIDTH)), border_color.into()));
	Ok(graphic)
}

#[node_macro::node(category(""))]
fn draw_thumbnails(ctx: impl Ctx + ExtractVarArgs) -> List<Vector> {
	let graph_ui = graph_ui(&ctx);
	let thumbnails = graph_ui.thumbnails.iter().filter_map(|(key, thumbnail)| Some((graph_ui.scene.layers().get(key)?, thumbnail)));
	thumbnails
		.map(|(layer, thumbnail)| {
			let [min, max] = layer.thumbnail_image();
			let mut image = rounded_rect(DVec2::ZERO, DVec2::ONE, [0.; 4]);
			image.left_apply_transform(&DAffine2::from_scale_angle_translation(max - min, 0., min));
			stamp_coverage(&mut image, Coverage::new_fill(), thumbnail.clone(), CoverPlacement::Above);
			image
		})
		.collect()
}

#[node_macro::node(category(""), memoize)]
fn draw_raised(ctx: impl Ctx + ExtractVarArgs, #[data] layers: Cache<Layer>, #[data] cards: Cache<Card>) -> List<Vector> {
	let scene = scene(&ctx);
	let mut graphic = draw_rows(scene.layers(), layers, draw_raised_parts_of_layer);
	graphic.extend(draw_rows(scene.cards(), cards, draw_connectors_of_card));
	graphic
}

fn draw_raised_parts_of_layer(layer: &Layer) -> List<Vector> {
	let mut graphic = List::new();
	if let Some(input) = &layer.secondary_input {
		graphic.extend(connector(&CONNECTOR_OUTLINE, layer.secondary_input_connector(), input));
	}
	let hovered = |button| layer.hovered_button == Some(button);
	let grip_color = match (hovered(LayerButton::Grip), layer.selection != Selection::None) {
		(true, _) => Some(Color::WHITE),
		(false, true) => Some(TEXT.into()),
		(false, false) => None,
	};
	if let Some(grip_color) = grip_color {
		let [grip, _] = layer.grip();
		let mut dots = BezPath::new();
		let [columns, rows] = LAYER_GRIP_DOT_COUNT;
		for (column, row) in (0..rows).flat_map(|row| (0..columns).map(move |column| (column, row))) {
			let center = grip + LAYER_GRIP_DOTS + DVec2::new(column as f64, row as f64) * LAYER_GRIP_DOT_SPACING;
			dots.extend(Circle::new((center.x, center.y), LAYER_GRIP_DOT_RADIUS).path_elements(CIRCLE_TOLERANCE));
		}
		graphic.push(painted(Vector::item_from_bezpath(&dots), Coverage::new_fill(), grip_color));
	}

	let mut button = |[min, max]: [DVec2; 2], which: LayerButton, [resting, hover]: [&str; 2]| {
		let hovered = hovered(which) && !layer.read_only;
		if hovered {
			graphic.push(painted(rounded_rect(min, max, [LAYER_BUTTON_RADIUS; 4]), Coverage::new_fill(), HOVERED_BUTTON.into()));
		}
		let name = if hovered && !layer.hovered_button_clicked { hover } else { resting };
		graphic.extend(icon(name, min + (LAYER_BUTTON_SIZE - ICON_SIZE) / 2., if layer.read_only { DISABLED_ICON } else { TEXT }));
	};
	if let Some(lock) = layer.lock() {
		button(lock, LayerButton::Lock, ["PadlockLocked", "PadlockUnlocked"]);
	}
	let eye = if layer.visible { ["EyeVisible", "EyeHide"] } else { ["EyeHidden", "EyeShow"] };
	button(layer.visibility(), LayerButton::Visibility, eye);
	graphic
}

fn draw_connectors_of_card(card: &Card) -> List<Vector> {
	card.inputs()
		.chain(card.outputs())
		.flat_map(|(top_left, input_or_output)| connector(&CONNECTOR_OUTLINE, top_left, input_or_output))
		.collect()
}

#[node_macro::node(category(""), memoize)]
fn draw_thick_wires(ctx: impl Ctx + ExtractVarArgs, #[data] cache: Cache<Wire>) -> List<Vector> {
	draw_rows(scene(&ctx).wires().iter().filter(|wire| wire.thick), cache, draw_wire)
}

#[node_macro::node(category(""), memoize)]
fn draw_wires(ctx: impl Ctx + ExtractVarArgs, #[data] cache: Cache<Wire>) -> List<Vector> {
	draw_rows(scene(&ctx).wires().iter().filter(|wire| !wire.thick), cache, draw_wire)
}

fn draw_wire(wire: &Wire) -> List<Vector> {
	let [bright, dim] = data_type_colors(wire.data_type);
	let previewed = wire.key == WireKey::Preview;
	let color = if previewed || wire.insertion_target { bright } else { dim };
	let stroke = |path: &BezPath, width: f64, color: SRGBA8| painted(Vector::item_from_bezpath(path), Coverage::new_stroke(&Stroke::new(width)), color.into());
	let drawn = |path: &BezPath| if previewed { flattened(&dashed(path, WIRE_DASHES)) } else { flattened(path) };
	let path = drawn(&wire.path);
	if wire.thick {
		List::from_iter([stroke(&path, THICK_WIRE_WIDTH, color), stroke(&drawn(&wire.center_line), WIRE_WIDTH, WIRE_SPLIT)])
	} else if wire.list {
		List::from_iter([stroke(&path, LIST_WIRE_WIDTH, color), stroke(&path, WIRE_WIDTH, WIRE_SPLIT)])
	} else {
		List::new_from_item(stroke(&path, WIRE_WIDTH, color))
	}
}

#[node_macro::node(category(""))]
fn draw_wire_in_progress(ctx: impl Ctx + ExtractVarArgs) -> List<Vector> {
	let Some(wire) = graph_ui(&ctx).config.wire_in_progress else { return List::new() };
	let [_, dim] = data_type_colors(wire.data_type);
	let path = flattened(&BezPath::from_vec(wire.path.to_vec()));
	List::new_from_item(painted(Vector::item_from_bezpath(&path), Coverage::new_stroke(&Stroke::new(WIRE_WIDTH)), dim.into()))
}

fn flattened(path: &BezPath) -> BezPath {
	let mut lines = BezPath::new();
	flatten(path, WIRE_FLATTENING_TOLERANCE, |element| lines.push(element));
	lines
}

#[node_macro::node(category(""), memoize)]
fn draw_cards(ctx: impl Ctx + ExtractVarArgs, #[data] cache: Cache<Card, Labels>) -> Result<List<Vector>, TextToVectorError> {
	let scene = scene(&ctx);
	let mut cache = cache.lock().unwrap_or_else(PoisonError::into_inner);
	let graphic = draw_changed_rows(scene.cards(), &mut cache, draw_card)?;

	let shown: HashSet<&str> = scene.cards().iter().flat_map(texts).collect();
	cache.labels.retain(|text| shown.contains(text));
	Ok(graphic)
}

fn draw_card(card: &Card, labels: &mut Labels) -> Result<List<Vector>, TextToVectorError> {
	let mut graphic = List::new();
	let [min, max] = card.body();

	let (card_color, text_color) = if card.visible { (Color::BLACK, TEXT) } else { (HIDDEN_CARD.into(), HIDDEN_TEXT) };
	let body = rounded_rect(min, max, [NODE_RADIUS; 4]);
	graphic.push(painted(body, Coverage::new_fill(), card_color.with_alpha(CARD_OPACITY)));

	let (header_tint, rows_tint) = match card.selection {
		Selection::None => (0.05, 0.),
		Selection::Inactive => (0.15, 0.1),
		Selection::Active => (0.2, 0.15),
	};
	let header_bottom = if card.rows() == 1 { NODE_RADIUS } else { 0. };
	let header = rounded_rect(min, DVec2::new(max.x, min.y + GRID_SIZE), [NODE_RADIUS, NODE_RADIUS, header_bottom, header_bottom]);
	graphic.push(painted(header, Coverage::new_fill(), Color::WHITE.with_alpha(header_tint)));
	if card.selection != Selection::None && card.rows() > 1 {
		let rows = rounded_rect(DVec2::new(min.x, min.y + GRID_SIZE), max, [0., 0., NODE_RADIUS, NODE_RADIUS]);
		graphic.push(painted(rows, Coverage::new_fill(), Color::WHITE.with_alpha(rows_tint)));
	}
	graphic.extend(icon(&card.icon, min + DVec2::new(NODE_ICON_LEFT, (GRID_SIZE - ICON_SIZE) / 2.), text_color));
	let name = labels.label(&card.name, NODE_NAME_MAX_WIDTH)?;
	graphic.extend(text(&name.outline, min + DVec2::new(NODE_NAME_LEFT, 0.), text_color));

	for row in 1..card.rows() {
		let (input, output) = (card.secondary_inputs.get(row - 1), card.secondary_outputs.get(row - 1));
		let Some(connector) = input.or(output) else { continue };
		let name = labels.label(&connector.name, NODE_WIDTH - 2. * NODE_ROW_PADDING)?;
		let left = if input.is_some() { NODE_ROW_PADDING } else { NODE_WIDTH - NODE_ROW_PADDING - name.width };
		graphic.extend(text(&name.outline, min + DVec2::new(left, row as f64 * GRID_SIZE), text_color));
	}

	let border_color = if card.previewed { PREVIEWED_BORDER } else { BORDER };
	let gap = |edge: f64, (top_left, _): (DVec2, &Connector)| {
		let center = DVec2::new(edge, top_left.y + CONNECTOR_SIZE / 2.);
		[center - CONNECTOR_GAP / 2., center + CONNECTOR_GAP / 2.]
	};
	let gaps: Vec<[DVec2; 2]> = card.inputs().map(|input| gap(min.x, input)).chain(card.outputs().map(|output| gap(max.x, output))).collect();
	let border = Vector::item_from_bezpath(&border(min, max, NODE_RADIUS, &gaps, card.previewed));
	graphic.push(painted(border, Coverage::new_stroke(&Stroke::new(BORDER_WIDTH)), border_color.into()));
	Ok(graphic)
}

fn texts(card: &Card) -> impl Iterator<Item = &str> {
	let connectors = card.secondary_inputs.iter().chain(&card.secondary_outputs);
	std::iter::once(card.name.as_str()).chain(connectors.map(|connector| connector.name.as_str()))
}

#[node_macro::node(category(""), memoize)]
fn draw_dots(ctx: impl Ctx + ExtractVarArgs, #[data] cache: Cache<Dot>) -> List<Vector> {
	draw_rows(scene(&ctx).dots(), cache, draw_dot)
}

fn draw_dot(dot: &Dot) -> List<Vector> {
	let [wired, unwired] = data_type_colors(dot.data_type);
	let center = dot.center();
	let grown = matches!(dot.highlight, Highlight::Hovered | Highlight::WireTarget);
	let scale = if grown { HOVERED_CONNECTOR_SCALE } else { 1. };
	let circle = |radius: f64| Vector::item_from_bezpath(&Circle::new((center.x, center.y), radius * scale).to_path(CIRCLE_TOLERANCE));
	let output_shown = dot.output_highlight != Highlight::None;
	let fill = if dot.wired && !output_shown { wired } else { unwired };
	let mut graphic = List::new_from_item(painted(circle(DOT_RADIUS), Coverage::new_fill(), fill.into()));

	if grown {
		let offset = if dot.previewed { DOT_PREVIEWED_RING_OFFSET } else { 0. };
		let ring = circle(DOT_RADIUS + offset + DOT_HOVERED_RING / 2.);
		graphic.push(painted(ring, Coverage::new_stroke(&Stroke::new(DOT_HOVERED_RING * scale)), unwired.into()));
	} else if dot.previewed {
		let radius = DOT_RADIUS + DOT_PREVIEWED_RING_OFFSET + BORDER_WIDTH;
		let ring = Vector::item_from_bezpath(&border(center - radius, center + radius, radius, &[], true));
		graphic.push(painted(ring, Coverage::new_stroke(&Stroke::new(BORDER_WIDTH)), wired.into()));
	} else if dot.selected {
		let ring = circle(DOT_RADIUS + DOT_SELECTED_RING / 2.);
		graphic.push(painted(ring, Coverage::new_stroke(&Stroke::new(DOT_SELECTED_RING)), Color::WHITE.with_alpha(SELECTED_DOT_RING_OPACITY)));
	}

	if output_shown {
		graphic.extend(drawn_connector(&CONNECTOR_OUTLINE, center + DOT_OUTPUT_ARROW, dot.data_type, true, dot.output_highlight));
	}
	graphic
}

#[node_macro::node(category(""), memoize)]
fn draw_errors(ctx: impl Ctx + ExtractVarArgs, #[data] cache: Cache<ErrorBadge, Paragraphs>) -> Result<List<Vector>, TextToVectorError> {
	let scene = scene(&ctx);
	let errors = scene.errors().iter();
	let mut cache = cache.lock().unwrap_or_else(PoisonError::into_inner);
	let graphic = draw_changed_rows(errors.clone().filter(|error| !error.hovered).chain(errors.filter(|error| error.hovered)), &mut cache, draw_error)?;

	let shown: HashSet<&str> = scene.errors().iter().map(|error| error.text.as_str()).collect();
	cache.labels.retain(|text| shown.contains(text));
	Ok(graphic)
}

fn draw_error(error: &ErrorBadge, paragraphs: &mut Paragraphs) -> Result<List<Vector>, TextToVectorError> {
	let paragraph = paragraphs.paragraph(&error.text, ERROR_LINE_HEIGHT, ERROR_MAX_WIDTH)?;
	let [min, max] = error.body(paragraph.size);
	let mut badge = List::new_from_item(painted(rounded_rect(min, max, [ERROR_RADIUS; 4]), Coverage::new_fill(), ERROR_RED.into()));
	let corners = error.tail();
	let mut tail = BezPath::new();
	tail.move_to((corners[0].x, corners[0].y));
	for corner in &corners[1..] {
		tail.line_to((corner.x, corner.y));
	}
	tail.close_path();
	badge.push(painted(Vector::item_from_bezpath(&tail), Coverage::new_fill(), ERROR_RED.into()));

	badge.extend(text(&paragraph.outline, error.text_top_left(paragraph.size), ERROR_TEXT));

	let mut grouped = rounded_rect(min, corners.into_iter().fold(max, DVec2::max), [0.; 4]);
	stamp_coverage(&mut grouped, Coverage::new_fill(), Graphic::VectorList(badge), CoverPlacement::Above);
	if !error.hovered {
		grouped.set_attribute(ATTR_OPACITY, ERROR_OPACITY);
	}
	Ok(List::new_from_item(grouped))
}

#[node_macro::node(category(""), memoize)]
fn draw_network_connectors(ctx: impl Ctx + ExtractVarArgs, #[data] cache: Cache<NetworkConnector, Labels>) -> Result<List<Vector>, TextToVectorError> {
	let scene = scene(&ctx);
	let mut cache = cache.lock().unwrap_or_else(PoisonError::into_inner);
	let graphic = draw_changed_rows(scene.network_connectors(), &mut cache, draw_network_connector)?;

	let connectors = scene.network_connectors().iter().filter_map(|row| row.connector.as_ref());
	let shown: HashSet<&str> = connectors.map(|connector| connector.name.as_str()).collect();
	cache.labels.retain(|text| shown.contains(text));
	Ok(graphic)
}

fn draw_network_connector(row: &NetworkConnector, labels: &mut Labels) -> Result<List<Vector>, TextToVectorError> {
	let Some(entry) = &row.connector else {
		return Ok(drawn_connector(&CONNECTOR_OUTLINE, row.connector_top_left(), DataType::General, false, row.add_highlight));
	};
	let mut graphic = connector(&CONNECTOR_OUTLINE, row.connector_top_left(), entry);
	let name = labels.label(&entry.name, f64::INFINITY)?;
	if !row.renaming {
		let [top_left, _] = row.name_box(name.width);
		graphic.extend(text(&name.outline, top_left, TEXT));
	}
	if row.editable && row.index < 2 {
		let [min, max] = row.separator(name.width);
		graphic.push(painted(rounded_rect(min, max, [0.; 4]), Coverage::new_fill(), SEPARATOR.into()));
	}
	Ok(graphic)
}

#[node_macro::node(category(""))]
fn draw_reorder_bar(ctx: impl Ctx + ExtractVarArgs) -> List<Vector> {
	let Some((kind, anchor)) = graph_ui(&ctx).config.reorder_bar else { return List::new() };
	let [min, max] = reorder_bar(kind, anchor);
	List::new_from_item(painted(rounded_rect(min, max, [0.; 4]), Coverage::new_fill(), Color::WHITE))
}

#[node_macro::node(category(""))]
fn draw_box_selection(ctx: impl Ctx + ExtractVarArgs) -> List<Vector> {
	let Some([start, end]) = graph_ui(&ctx).config.box_selection else { return List::new() };
	let min = start.min(end);
	let max = min + (end - start).abs() + 2. * BORDER_WIDTH;
	let fill = painted(rounded_rect(min, max, [0.; 4]), Coverage::new_fill(), Color::from(BOX_SELECTION).with_alpha(BOX_SELECTION_FILL_OPACITY));
	let [inner_min, inner_max] = [min + BORDER_WIDTH / 2., max - BORDER_WIDTH / 2.];
	let outline = Rect::new(inner_min.x, inner_min.y, inner_max.x, inner_max.y).to_path(0.);
	let border = painted(Vector::item_from_bezpath(&outline), Coverage::new_stroke(&Stroke::new(BORDER_WIDTH)), BOX_SELECTION.into());
	List::from_iter([fill, border])
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::network_connector::NetworkConnectorKind;
	use crate::test_utils::{badge, card, connector, dot, layer, network_connector, wire};
	use core_types::bounds::{BoundingBox, RenderBoundingBox};
	use core_types::uuid::NodeId;
	use core_types::{Context, OwnedContextImpl};
	use vello_gpu::kurbo::PathEl;

	fn scene_context(scene: GraphUiScene) -> Context<'static> {
		OwnedContextImpl::default().with_vararg(Box::new(scene)).into_context()
	}

	fn bounds(graphic: &List<Vector>) -> RenderBoundingBox {
		graphic.bounding_box(DAffine2::IDENTITY, false)
	}

	#[test]
	fn a_card_that_only_moved_to_another_cell_keeps_its_vectors_and_is_drawn_at_that_cell() {
		let cache = Default::default();
		let draw = |scene: GraphUiScene| draw_cards(scene_context(scene), &cache).unwrap();
		let mut edit = GraphUiScene::default().edit();
		edit.cards.put(Card {
			name: "Path".to_string(),
			cell: IVec2::new(2, 1),
			..card(0)
		});
		let (scene, _) = edit.finish();
		draw(scene.clone());
		let drawn = cache.lock().unwrap().drawn[&NodeId(0)].1.elements_witness();

		let mut edit = scene.edit();
		edit.cards.update(&NodeId(0), |card| card.cell = IVec2::new(-3, 5));
		let (scene, _) = edit.finish();
		let moved = draw(scene.clone());
		assert!(drawn.matches(&cache.lock().unwrap().drawn[&NodeId(0)].1), "a moved card keeps its vectors");
		let in_place = draw_card(scene.cards().get(&NodeId(0)).unwrap(), &mut Labels::default()).unwrap();
		assert_eq!(bounds(&moved), bounds(&in_place), "a moved card is drawn at its new cell");
	}

	#[test]
	fn a_gliding_layer_keeps_its_vectors_and_is_drawn_its_glide_away() {
		let cache = Default::default();
		let draw = |scene: GraphUiScene| draw_layers(scene_context(scene), &cache).unwrap();
		let mut edit = GraphUiScene::default().edit();
		edit.layers.put(Layer {
			name: "Path".to_string(),
			cell: IVec2::new(2, 1),
			..layer(0)
		});
		let (scene, _) = edit.finish();
		draw(scene.clone());
		let drawn = cache.lock().unwrap().drawn[&NodeId(0)].1.elements_witness();

		let mut edit = scene.edit();
		edit.layers.update(&NodeId(0), |layer| layer.glide = DVec2::new(-7.25, 3.5));
		let (scene, _) = edit.finish();
		let gliding = draw(scene.clone());
		assert!(drawn.matches(&cache.lock().unwrap().drawn[&NodeId(0)].1), "a gliding layer keeps its vectors");
		let in_place = draw_layer(scene.layers().get(&NodeId(0)).unwrap(), &mut Labels::default()).unwrap();
		assert_eq!(bounds(&gliding), bounds(&in_place), "a gliding layer is drawn its glide away");
	}

	#[test]
	fn a_dot_whose_look_changed_is_drawn_again_while_a_dot_that_only_moved_keeps_its_vectors() {
		let cache = Default::default();
		let draw = |scene: GraphUiScene| draw_dots(scene_context(scene), &cache);
		let mut edit = GraphUiScene::default().edit();
		edit.dots.replace_all([dot(0), dot(1)]);
		let (scene, _) = edit.finish();
		draw(scene.clone());
		let witness = |key| cache.lock().unwrap().drawn[&NodeId(key)].1.elements_witness();
		let [moved, changed] = [witness(0), witness(1)];

		let mut edit = scene.edit();
		edit.dots.update(&NodeId(0), |dot| dot.cell = IVec2::new(4, -2));
		edit.dots.update(&NodeId(1), |dot| {
			dot.cell = IVec2::new(4, 2);
			dot.selected = true;
		});
		let (scene, _) = edit.finish();
		let drawn = draw(scene.clone());
		let cache = cache.lock().unwrap();
		assert!(moved.matches(&cache.drawn[&NodeId(0)].1), "a dot that only moved keeps its vectors");
		assert!(!changed.matches(&cache.drawn[&NodeId(1)].1), "a dot whose look changed is drawn again");
		let in_place: List<Vector> = scene.dots().iter().flat_map(draw_dot).collect();
		assert_eq!(bounds(&drawn), bounds(&in_place), "both dots are drawn at their new cells");
	}

	#[test]
	fn only_the_cards_that_changed_are_drawn_again() {
		let cache = Default::default();
		let draw = |scene: GraphUiScene| draw_cards(scene_context(scene), &cache).unwrap();
		let mut edit = GraphUiScene::default().edit();
		edit.cards.replace_all([card(0), card(1), card(2)]);
		let (scene, _) = edit.finish();
		draw(scene.clone());
		let witness = |key| cache.lock().unwrap().drawn[&NodeId(key)].1.elements_witness();
		let [unchanged, changed] = [witness(0), witness(1)];

		let mut edit = scene.edit();
		edit.cards.update(&NodeId(1), |card| card.selection = Selection::Active);
		edit.cards.remove(&NodeId(2));
		draw(edit.finish().0);
		let cache = cache.lock().unwrap();
		assert!(unchanged.matches(&cache.drawn[&NodeId(0)].1), "an unchanged card keeps its vectors");
		assert!(!changed.matches(&cache.drawn[&NodeId(1)].1), "a changed card is drawn again");
		assert!(!cache.drawn.contains_key(&NodeId(2)), "a removed card is forgotten");
	}

	#[test]
	fn the_label_of_a_text_is_forgotten_once_no_card_shows_it() {
		let named = |key, name: &str| Card { name: name.to_string(), ..card(key) };
		let cache = Default::default();
		let draw = |cards: Vec<Card>| {
			let mut edit = GraphUiScene::default().edit();
			edit.cards.replace_all(cards);
			draw_cards(scene_context(edit.finish().0), &cache).unwrap();
		};
		draw(vec![named(0, "Path"), named(1, "Fill")]);
		let label = |text| cache.lock().unwrap().labels.label(text, NODE_NAME_MAX_WIDTH).unwrap().outline.elements_witness();
		let [path, fill] = [label("Path"), label("Fill")];

		draw(vec![named(0, "Path")]);
		let mut cache = cache.lock().unwrap();
		assert!(
			path.matches(&cache.labels.label("Path", NODE_NAME_MAX_WIDTH).unwrap().outline),
			"the label of a text still shown is kept"
		);
		assert!(
			!fill.matches(&cache.labels.label("Fill", NODE_NAME_MAX_WIDTH).unwrap().outline),
			"the label of a text no card shows is laid out again"
		);
	}

	#[test]
	fn the_label_of_a_text_is_forgotten_once_no_layer_shows_it() {
		let named = |key, name: &str| Layer { name: name.to_string(), ..layer(key) };
		let cache = Default::default();
		let draw = |layers: Vec<Layer>| {
			let mut edit = GraphUiScene::default().edit();
			edit.layers.replace_all(layers);
			draw_layers(scene_context(edit.finish().0), &cache).unwrap();
		};
		draw(vec![named(0, "Path"), named(1, "Fill")]);
		let max_width = layer(0).name_width();
		let label = |text| cache.lock().unwrap().labels.label(text, max_width).unwrap().outline.elements_witness();
		let [path, fill] = [label("Path"), label("Fill")];

		draw(vec![named(0, "Path")]);
		let mut cache = cache.lock().unwrap();
		assert!(path.matches(&cache.labels.label("Path", max_width).unwrap().outline), "the label of a text still shown is kept");
		assert!(
			!fill.matches(&cache.labels.label("Fill", max_width).unwrap().outline),
			"the label of a text no layer shows is laid out again"
		);
	}

	#[test]
	fn the_label_of_a_text_is_forgotten_once_no_import_or_export_shows_it() {
		let named = |kind, name: &str| NetworkConnector {
			connector: Some(connector(name)),
			..network_connector(kind, 0)
		};
		let cache = Default::default();
		let draw = |rows: Vec<NetworkConnector>| {
			let mut edit = GraphUiScene::default().edit();
			edit.network_connectors.replace_all(rows);
			draw_network_connectors(scene_context(edit.finish().0), &cache).unwrap();
		};
		draw(vec![named(NetworkConnectorKind::Import, "Path"), named(NetworkConnectorKind::Export, "Fill")]);
		let label = |text| cache.lock().unwrap().labels.label(text, f64::INFINITY).unwrap().outline.elements_witness();
		let [path, fill] = [label("Path"), label("Fill")];

		draw(vec![named(NetworkConnectorKind::Import, "Path")]);
		let mut cache = cache.lock().unwrap();
		assert!(path.matches(&cache.labels.label("Path", f64::INFINITY).unwrap().outline), "the label of a text still shown is kept");
		assert!(
			!fill.matches(&cache.labels.label("Fill", f64::INFINITY).unwrap().outline),
			"the label of a text no import or export shows is laid out again"
		);
	}

	#[test]
	fn the_paragraph_of_an_error_is_forgotten_once_no_badge_shows_it() {
		let cache = Default::default();
		let draw = |badges: Vec<ErrorBadge>| {
			let mut edit = GraphUiScene::default().edit();
			edit.errors.replace_all(badges);
			draw_errors(scene_context(edit.finish().0), &cache).unwrap();
		};
		draw(vec![badge(0, "Failed"), badge(1, "Missing input")]);
		let paragraph = |text| cache.lock().unwrap().labels.paragraph(text, ERROR_LINE_HEIGHT, ERROR_MAX_WIDTH).unwrap().outline.elements_witness();
		let [failed, missing] = [paragraph("Failed"), paragraph("Missing input")];

		draw(vec![badge(0, "Failed")]);
		let mut cache = cache.lock().unwrap();
		assert!(
			failed.matches(&cache.labels.paragraph("Failed", ERROR_LINE_HEIGHT, ERROR_MAX_WIDTH).unwrap().outline),
			"the paragraph of a text still shown is kept"
		);
		assert!(
			!missing.matches(&cache.labels.paragraph("Missing input", ERROR_LINE_HEIGHT, ERROR_MAX_WIDTH).unwrap().outline),
			"the paragraph of a text no badge shows is laid out again"
		);
	}

	#[test]
	fn the_column_groups_of_a_grid_dot_light_each_pixel_of_its_disc_once() {
		for (radius, groups, pixels) in [(1, 1, 4), (2, 2, 12), (3, 2, 32), (4, 3, 52)] {
			let disc: Vec<[u32; 2]> = (0..2 * radius)
				.flat_map(|row| (0..2 * radius).map(move |column| [column, row]))
				.filter(|&[column, row]| (column as f64 + 0.5 - radius as f64).hypot(row as f64 + 0.5 - radius as f64) < radius as f64)
				.collect();
			let columns = disc_columns(radius);
			let mut lit: Vec<[u32; 2]> = columns
				.iter()
				.flat_map(|(&[top, bottom], runs)| (top..bottom).flat_map(move |row| runs.iter().flat_map(move |&[left, right]| (left..right).map(move |column| [column, row]))))
				.collect();
			lit.sort_by_key(|&[column, row]| [row, column]);
			assert_eq!((columns.len(), disc.len()), (groups, pixels), "the groups and pixels of the disc at radius {radius}");
			assert_eq!(lit, disc, "every pixel of the disc at radius {radius} is lit by exactly one group");
		}
	}

	#[test]
	fn only_the_wires_that_changed_are_drawn_again() {
		let key = |node| WireKey::Input { node: NodeId(node), index: 0 };
		let at = |node, y: f64| Wire {
			path: BezPath::from_vec(vec![PathEl::MoveTo((0., y).into()), PathEl::LineTo((48., y).into())]),
			..wire(key(node))
		};
		let cache = Default::default();
		let draw = |scene: GraphUiScene| draw_wires(scene_context(scene), &cache);
		let mut edit = GraphUiScene::default().edit();
		edit.wires.replace_all([at(0, 0.), at(1, 24.)]);
		let (scene, _) = edit.finish();
		draw(scene.clone());
		let witness = |node| cache.lock().unwrap().drawn[&key(node)].1.elements_witness();
		let [unchanged, changed] = [witness(0), witness(1)];

		let mut edit = scene.edit();
		edit.wires.put(at(1, 48.));
		draw(edit.finish().0);
		let cache = cache.lock().unwrap();
		assert!(unchanged.matches(&cache.drawn[&key(0)].1), "the other wire keeps its vectors");
		assert!(!changed.matches(&cache.drawn[&key(1)].1), "the changed wire is drawn again");
	}
}
