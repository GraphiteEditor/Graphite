use crate::consts::DEFAULT_STROKE_WIDTH;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::TransactionStatus;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::utility_types::DocumentToolData;
use graphene_std::Color;
use graphene_std::vector::style::{FillChoice, FillChoiceUI, PaintOrder, StrokeAlign, StrokeCap, StrokeJoin};

/// Color selector widgets seen in [`LayoutTarget::ToolOptions`] bar.
pub struct ToolColorOptions {
	/// The fill/stroke value shown in the swatch. `None` = mixed across selected layers.
	pub fill_choice: Option<FillChoice>,
	/// The checkbox state. `None` = mixed across selected layers.
	pub enabled: Option<bool>,
	/// When set, `fill_choice` is a working-color fallback (vs. a layer-derived saved color) and is refreshed live by `WorkingColorChanged`.
	pub tracks_working_color: bool,
}

impl Default for ToolColorOptions {
	fn default() -> Self {
		Self {
			fill_choice: Some(FillChoice::Solid(Color::BLACK)),
			enabled: Some(true),
			tracks_working_color: true,
		}
	}
}

impl ToolColorOptions {
	pub fn new_enabled() -> Self {
		Self::default()
	}

	pub fn new_disabled() -> Self {
		Self {
			fill_choice: Some(FillChoice::None),
			enabled: Some(false),
			tracks_working_color: true,
		}
	}

	/// True when the slot is actively applied, i.e. `enabled` is `Some(true)`.
	/// `None` (mixed) and `Some(false)` both count as not actively applied.
	pub fn is_active(&self) -> bool {
		self.enabled == Some(true)
	}

	/// The active solid color, suitable for storing in a working color or downstream rendering input.
	pub fn active_color(&self) -> Option<Color> {
		if !self.is_active() {
			return None;
		}
		self.fill_choice.as_ref()?.as_solid()
	}

	pub fn apply_fill(&self, layer: LayerNodeIdentifier, responses: &mut VecDeque<Message>) {
		if !self.is_active() {
			return;
		}
		if let Some(FillChoice::Solid(color)) = &self.fill_choice {
			let fill = graphene_std::vector::style::Fill::Solid(*color);
			responses.add(GraphOperationMessage::FillSet { layer, fill });
		}
	}

	pub fn apply_stroke(&self, weight: f64, layer: LayerNodeIdentifier, responses: &mut VecDeque<Message>) {
		if !self.is_active() {
			return;
		}
		if let Some(FillChoice::Solid(color)) = &self.fill_choice {
			let color = Some(*color);
			let stroke = graphene_std::vector::style::Stroke::new(weight);
			responses.add(GraphOperationMessage::StrokeSet { layer, color, stroke });
		}
	}

	pub fn create_widgets(
		&self,
		label_text: impl Into<String>,
		checkbox_callback: impl Fn(&CheckboxInput) -> Message + 'static + Send + Sync,
		color_callback: impl Fn(&ColorInput) -> Message + 'static + Send + Sync,
	) -> Vec<WidgetInstance> {
		let checkbox_id = CheckboxId::new();
		// In the mixed state (`fill_choice` is `None`) the dash overlay covers the swatch, so the underlying widget value just drives the picker's initial position.
		// `FillChoice::None` gives it a neutral starting point.
		let mixed_color = self.fill_choice.is_none();
		// Convert the internal linear-light `FillChoice` to the JS-boundary `FillChoiceUI` (with `SRGBA8` colors) for the widget value.
		let widget_value = FillChoiceUI::from(self.fill_choice.as_ref().unwrap_or(&FillChoice::None));
		let mixed_enabled = self.enabled.is_none();
		// In the mixed-enabled state the underlying `checked` value is hidden behind the indeterminate dash.
		// The frontend's click handler sends `true` when the user resolves the mixed state by clicking.
		let checked = self.enabled.unwrap_or(false);

		vec![
			CheckboxInput::new(checked).mixed(mixed_enabled).on_update(checkbox_callback).for_label(checkbox_id).widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			TextLabel::new(label_text).for_checkbox(checkbox_id).widget_instance(),
			Separator::new(SeparatorStyle::Related).widget_instance(),
			ColorInput::new(widget_value)
				.mixed(mixed_color)
				.min_width(48)
				.max_width(48)
				.narrow(true)
				.on_update(color_callback)
				.widget_instance(),
		]
	}
}

/// Shared per-tool state for drawing tools that produce a stroked-and-filled shape (Shape, Pen, Freehand, Spline).
pub struct DrawingToolState {
	/// The current stroke weight. `None` = mixed across selected layers.
	pub line_weight: Option<f64>,
	/// Persistent default weight, updated when the user edits the weight while no layer is selected.
	pub default_line_weight: f64,
	/// Stroke alignment from the selection. `None` = mixed.
	pub stroke_align: Option<StrokeAlign>,
	/// Stroke cap from the selection. `None` = mixed.
	pub stroke_cap: Option<StrokeCap>,
	/// Stroke join from the selection. `None` = mixed.
	pub stroke_join: Option<StrokeJoin>,
	/// Stroke miter limit from the selection. `None` = mixed.
	pub miter_limit: Option<f64>,
	/// Paint order from the selection. `None` = mixed.
	pub paint_order: Option<PaintOrder>,
	/// Dash lengths from the selection. `None` = mixed.
	pub dash_lengths: Option<Vec<f64>>,
	/// Dash offset from the selection. `None` = mixed.
	pub dash_offset: Option<f64>,
	/// Set of layers we last synced from, used to detect real selection changes vs. internal node toggles.
	pub last_synced_selection: Vec<LayerNodeIdentifier>,
	/// The fill swatch's color, checkbox, and mixed state.
	pub fill: ToolColorOptions,
	/// The stroke swatch's color, checkbox, and mixed state.
	pub stroke: ToolColorOptions,
	/// When false (default), fill follows the secondary working color and stroke follows the primary; when true, the routing is reversed.
	/// Persisted per-tool. The Shape tool additionally persists it for each shape mode via its options.
	pub colors_swapped: bool,
}

impl DrawingToolState {
	pub fn new(fill_enabled: bool) -> Self {
		Self {
			line_weight: Some(DEFAULT_STROKE_WIDTH),
			default_line_weight: DEFAULT_STROKE_WIDTH,
			stroke_align: Some(StrokeAlign::default()),
			stroke_cap: Some(StrokeCap::default()),
			stroke_join: Some(StrokeJoin::default()),
			miter_limit: Some(4.),
			paint_order: Some(PaintOrder::default()),
			dash_lengths: Some(Vec::new()),
			dash_offset: Some(0.),
			last_synced_selection: Vec::new(),
			fill: if fill_enabled { ToolColorOptions::new_enabled() } else { ToolColorOptions::new_disabled() },
			stroke: ToolColorOptions::new_enabled(),
			colors_swapped: false,
		}
	}

	/// The line weight to apply, falling back to the persistent default when [`Self::line_weight`] is `None` (mixed).
	pub fn effective_line_weight(&self) -> f64 {
		self.line_weight.unwrap_or(self.default_line_weight)
	}

	/// Dash lengths to apply, falling back to empty when [`Self::dash_lengths`] is `None` (mixed).
	pub fn effective_dash_lengths(&self) -> Vec<f64> {
		self.dash_lengths.clone().unwrap_or_default()
	}

	/// Applies a stroke to a freshly created `layer` using the tool's currently selected color, weight, and stroke options (align, cap, join, etc.).
	/// Used by the drawing tools at shape-creation time so new shapes inherit the popover's options instead of defaulting to the `Stroke` struct's defaults.
	pub fn apply_stroke_to_new_layer(&self, layer: LayerNodeIdentifier, responses: &mut VecDeque<Message>) {
		if !self.stroke.is_active() {
			return;
		}
		let Some(FillChoice::Solid(color)) = &self.stroke.fill_choice else { return };
		let color = Some(*color);
		let stroke = graphene_std::vector::style::Stroke {
			weight: self.effective_line_weight(),
			align: self.stroke_align.unwrap_or_default(),
			cap: self.stroke_cap.unwrap_or_default(),
			join: self.stroke_join.unwrap_or_default(),
			join_miter_limit: self.miter_limit.unwrap_or(4.),
			paint_order: self.paint_order.unwrap_or_default(),
			dash_lengths: self.effective_dash_lengths(),
			dash_offset: self.dash_offset.unwrap_or(0.),
			transform: glam::DAffine2::IDENTITY,
		};
		responses.add(GraphOperationMessage::StrokeSet { layer, color, stroke });
	}
}

/// Builds a `FillChoice::Solid` from a color.
pub fn solid(color: Color) -> FillChoice {
	FillChoice::Solid(color)
}

/// The fill working color (the source for the fill swatch when nothing is selected).
/// Defaults to secondary, swapped to primary when the per-tool [`DrawingToolState::colors_swapped`] flag is set.
pub fn fill_working_color(global: &DocumentToolData, colors_swapped: bool) -> Color {
	if colors_swapped { global.primary_color } else { global.secondary_color }
}

/// The stroke working color (the source for the stroke swatch when nothing is selected).
/// Defaults to primary, swapped to secondary when the per-tool [`DrawingToolState::colors_swapped`] flag is set.
pub fn stroke_working_color(global: &DocumentToolData, colors_swapped: bool) -> Color {
	if colors_swapped { global.secondary_color } else { global.primary_color }
}

/// Syncs fill and stroke from the selection (or working colors when empty). With `selection_changed = false`, preserves display values
/// for inactive states instead of resetting them. Returns `true` if anything changed.
pub fn sync_color_options(
	drawing: &mut DrawingToolState,
	natural_fill_enabled: bool,
	natural_stroke_enabled: bool,
	global: &DocumentToolData,
	document: &DocumentMessageHandler,
	selection_changed: bool,
) -> bool {
	let fill_fallback = solid(fill_working_color(global, drawing.colors_swapped));
	let stroke_fallback = solid(stroke_working_color(global, drawing.colors_swapped));

	let mut changed = false;

	// FILL

	let new_fill = if let Some(state) = graph_modification_utils::selected_fill_state(document) {
		let active = state.enabled == Some(true);
		let (display_choice, tracks_working) = match &state.fill_choice {
			Some(choice) if active => (Some(choice.clone()), false),
			Some(_) if selection_changed => (Some(fill_fallback.clone()), true),
			Some(_) => (drawing.fill.fill_choice.clone(), drawing.fill.tracks_working_color),
			None => (None, true),
		};
		(state.enabled, display_choice, tracks_working)
	} else {
		// On a real deselect, revert to the working color; otherwise preserve the displayed value.
		let display_choice = if selection_changed { Some(fill_fallback) } else { drawing.fill.fill_choice.clone() };
		let tracks_working = if selection_changed { true } else { drawing.fill.tracks_working_color };
		(Some(natural_fill_enabled), display_choice, tracks_working)
	};
	if drawing.fill.enabled != new_fill.0 || drawing.fill.fill_choice != new_fill.1 || drawing.fill.tracks_working_color != new_fill.2 {
		drawing.fill.enabled = new_fill.0;
		drawing.fill.fill_choice = new_fill.1;
		drawing.fill.tracks_working_color = new_fill.2;
		changed = true;
	}

	// STROKE

	let new_stroke = if let Some(state) = graph_modification_utils::selected_stroke_state(document) {
		let active = state.enabled == Some(true);
		let (display_choice, tracks_working) = match state.optional_color {
			Some(color) if active => (Some(color.map_or(FillChoice::None, FillChoice::Solid)), false),
			Some(_) if selection_changed => (Some(stroke_fallback.clone()), true),
			Some(_) => (drawing.stroke.fill_choice.clone(), drawing.stroke.tracks_working_color),
			None => (None, true),
		};
		(state.enabled, display_choice, tracks_working)
	} else {
		let display_choice = if selection_changed { Some(stroke_fallback) } else { drawing.stroke.fill_choice.clone() };
		let tracks_working = if selection_changed { true } else { drawing.stroke.tracks_working_color };
		(Some(natural_stroke_enabled), display_choice, tracks_working)
	};
	if drawing.stroke.enabled != new_stroke.0 || drawing.stroke.fill_choice != new_stroke.1 || drawing.stroke.tracks_working_color != new_stroke.2 {
		drawing.stroke.enabled = new_stroke.0;
		drawing.stroke.fill_choice = new_stroke.1;
		drawing.stroke.tracks_working_color = new_stroke.2;
		changed = true;
	}

	changed
}

/// Full SelectionChanged update for a drawing tool: syncs fill/stroke colors and the stroke weight. Returns `true` if the layout needs refreshing.
pub fn sync_drawing_state(drawing: &mut DrawingToolState, natural_fill_enabled: bool, natural_stroke_enabled: bool, global: &DocumentToolData, document: &DocumentMessageHandler) -> bool {
	let selection_changed = selection_changed_since_last_sync(&mut drawing.last_synced_selection, document);
	let mut needs_refresh = sync_color_options(drawing, natural_fill_enabled, natural_stroke_enabled, global, document, selection_changed);

	let new_line_weight = match compute_weight_sync(document) {
		WeightSyncOutcome::Set(weight) => Some(weight),
		WeightSyncOutcome::Mixed => None,
		// On a real selection change, revert to the default; otherwise preserve.
		WeightSyncOutcome::NoStrokes | WeightSyncOutcome::NoSelection => {
			if selection_changed {
				Some(drawing.default_line_weight)
			} else {
				drawing.line_weight
			}
		}
	};
	if drawing.line_weight != new_line_weight {
		drawing.line_weight = new_line_weight;
		needs_refresh = true;
	}

	needs_refresh |= sync_stroke_options(drawing, document);

	needs_refresh
}

/// Reads the stroke proto-node inputs (align, cap, join, miter limit, paint order, dash lengths, dash offset) across the selection and updates
/// the matching fields on `drawing`. Each field becomes `None` (mixed) when selected strokes disagree. With no selection, fields are left as-is.
fn sync_stroke_options(drawing: &mut DrawingToolState, document: &DocumentMessageHandler) -> bool {
	let strokes: Vec<_> = document
		.network_interface
		.selected_nodes()
		.selected_layers_except_artboards(&document.network_interface)
		.filter_map(|layer| graph_modification_utils::get_stroke_options(layer, &document.network_interface))
		.collect();
	if strokes.is_empty() {
		return false;
	}

	fn unanimous<T: PartialEq + Clone>(values: impl IntoIterator<Item = T>) -> Option<T> {
		let mut iter = values.into_iter();
		let first = iter.next()?;
		iter.all(|v| v == first).then_some(first)
	}

	let new_align = unanimous(strokes.iter().map(|s| s.align));
	let new_cap = unanimous(strokes.iter().map(|s| s.cap));
	let new_join = unanimous(strokes.iter().map(|s| s.join));
	let new_miter = unanimous(strokes.iter().map(|s| s.miter_limit));
	let new_paint_order = unanimous(strokes.iter().map(|s| s.paint_order));
	let new_dash_lengths = unanimous(strokes.iter().map(|s| &s.dash_lengths)).cloned();
	let new_dash_offset = unanimous(strokes.iter().map(|s| s.dash_offset));

	let mut changed = false;

	if drawing.stroke_align != new_align {
		drawing.stroke_align = new_align;
		changed = true;
	}
	if drawing.stroke_cap != new_cap {
		drawing.stroke_cap = new_cap;
		changed = true;
	}
	if drawing.stroke_join != new_join {
		drawing.stroke_join = new_join;
		changed = true;
	}
	if drawing.miter_limit != new_miter {
		drawing.miter_limit = new_miter;
		changed = true;
	}
	if drawing.paint_order != new_paint_order {
		drawing.paint_order = new_paint_order;
		changed = true;
	}
	if drawing.dash_lengths != new_dash_lengths {
		drawing.dash_lengths = new_dash_lengths;
		changed = true;
	}
	if drawing.dash_offset != new_dash_offset {
		drawing.dash_offset = new_dash_offset;
		changed = true;
	}

	changed
}

/// Same as [`sync_color_options`] but for tools that only have a fill option (e.g., text). The fill follows the given working color when nothing is selected.
pub fn sync_fill_only(fill: &mut ToolColorOptions, natural_fill_enabled: bool, fill_color: Color, document: &DocumentMessageHandler, selection_changed: bool) -> bool {
	let fill_fallback = solid(fill_color);

	let new_fill = if let Some(state) = graph_modification_utils::selected_fill_state(document) {
		let active = state.enabled == Some(true);
		let (display_choice, tracks_working_color) = match &state.fill_choice {
			Some(choice) if active => (Some(choice.clone()), false),
			Some(_) if selection_changed => (Some(fill_fallback.clone()), true),
			Some(_) => (fill.fill_choice.clone(), fill.tracks_working_color),
			None => (None, true),
		};
		(state.enabled, display_choice, tracks_working_color)
	} else {
		let display_choice = if selection_changed { Some(fill_fallback) } else { fill.fill_choice.clone() };
		let tracks_working = if selection_changed { true } else { fill.tracks_working_color };
		(Some(natural_fill_enabled), display_choice, tracks_working)
	};

	if fill.enabled != new_fill.0 || fill.fill_choice != new_fill.1 || fill.tracks_working_color != new_fill.2 {
		fill.enabled = new_fill.0;
		fill.fill_choice = new_fill.1;
		fill.tracks_working_color = new_fill.2;
		true
	} else {
		false
	}
}

/// True if at least one (non-artboard) layer is currently selected.
pub fn has_selection(document: &DocumentMessageHandler) -> bool {
	document
		.network_interface
		.selected_nodes()
		.selected_layers_except_artboards(&document.network_interface)
		.next()
		.is_some()
}

/// Applies a user-picked fill (gradient or solid). With a selection, writes to the layers; with none, pushes a solid to the swap-routed working color slot.
pub fn apply_fill_color_pick(drawing: &mut DrawingToolState, fill_choice: FillChoice, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	apply_fill_only_color_pick(&mut drawing.fill, fill_choice, drawing.colors_swapped, document, responses);
}

/// Single-slot variant of [`apply_fill_color_pick`] (e.g. for text). `slot_is_primary` says which working color this slot binds to.
pub fn apply_fill_only_color_pick(fill: &mut ToolColorOptions, fill_choice: FillChoice, slot_is_primary: bool, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	fill.fill_choice = Some(fill_choice.clone());
	fill.enabled = Some(true);
	fill.tracks_working_color = false;
	if has_selection(document) {
		if document.network_interface.transaction_status() == TransactionStatus::Finished {
			responses.add(DocumentMessage::StartTransaction);
		}
		graph_modification_utils::set_fill_for_selected_layers(fill_choice, document, responses);
	} else if let FillChoice::Solid(color) = fill_choice {
		responses.add(ToolMessage::SelectWorkingColor { color, primary: slot_is_primary });
	}
}

/// Applies a user-picked stroke color. With a selection, writes to the layers; with none, pushes to the swap-routed working color slot.
pub fn apply_stroke_color_pick(drawing: &mut DrawingToolState, color: Option<Color>, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	drawing.stroke.fill_choice = Some(color.map_or(FillChoice::None, FillChoice::Solid));
	drawing.stroke.enabled = Some(true);
	drawing.stroke.tracks_working_color = false;
	if has_selection(document) {
		if document.network_interface.transaction_status() == TransactionStatus::Finished {
			responses.add(DocumentMessage::StartTransaction);
		}
		graph_modification_utils::set_stroke_color_for_selected_layers(color, drawing.effective_line_weight(), document, responses);
	} else if let Some(color) = color {
		responses.add(ToolMessage::SelectWorkingColor {
			color,
			primary: !drawing.colors_swapped,
		});
	}
}

/// Toggles the fill checkbox: re-applies the preserved color when enabled, removes the fill node when disabled.
pub fn apply_fill_enabled(drawing: &mut DrawingToolState, enabled: bool, global: &DocumentToolData, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	apply_fill_only_enabled(&mut drawing.fill, enabled, fill_working_color(global, drawing.colors_swapped), document, responses);
}

/// Single-slot variant of [`apply_fill_enabled`]. `working_color` is the fallback used when re-ticking or unticking from a mixed state.
pub fn apply_fill_only_enabled(fill: &mut ToolColorOptions, enabled: bool, working_color: Color, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	fill.enabled = Some(enabled);
	if has_selection(document) {
		responses.add(DocumentMessage::AddTransaction);
	}
	if enabled {
		// Mixed re-tick has no per-layer color to restore; fall back to the working color and keep tracking it.
		let fill_choice = fill.fill_choice.clone().unwrap_or_else(|| {
			fill.tracks_working_color = true;
			solid(working_color)
		});
		fill.fill_choice = Some(fill_choice.clone());
		graph_modification_utils::set_fill_for_selected_layers(fill_choice, document, responses);
	} else {
		// Unticking from mixed: capture the working color as the saved value so the swatch keeps following the link.
		if fill.fill_choice.is_none() {
			fill.fill_choice = Some(solid(working_color));
			fill.tracks_working_color = true;
		}
		graph_modification_utils::remove_fill_for_selected_layers(document, responses);
	}
}

/// Toggles the stroke checkbox: mirrors [`apply_fill_enabled`].
pub fn apply_stroke_enabled(drawing: &mut DrawingToolState, enabled: bool, global: &DocumentToolData, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	drawing.stroke.enabled = Some(enabled);
	if has_selection(document) {
		responses.add(DocumentMessage::AddTransaction);
	}
	if enabled {
		let stroke_choice = drawing.stroke.fill_choice.clone().unwrap_or_else(|| {
			drawing.stroke.tracks_working_color = true;
			solid(stroke_working_color(global, drawing.colors_swapped))
		});
		drawing.stroke.fill_choice = Some(stroke_choice.clone());
		graph_modification_utils::set_stroke_color_for_selected_layers(stroke_choice.as_solid(), drawing.effective_line_weight(), document, responses);
	} else {
		if drawing.stroke.fill_choice.is_none() {
			drawing.stroke.fill_choice = Some(solid(stroke_working_color(global, drawing.colors_swapped)));
			drawing.stroke.tracks_working_color = true;
		}
		graph_modification_utils::remove_stroke_for_selected_layers(document, responses);
	}
}

/// Applies a user-edited stroke weight to the selection, also persisting it as the no-selection default.
pub fn apply_line_weight(drawing: &mut DrawingToolState, line_weight: f64, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	drawing.line_weight = Some(line_weight);
	if !has_selection(document) {
		drawing.default_line_weight = line_weight;
	}
	graph_modification_utils::set_stroke_weight_for_selected_layers(line_weight, document, responses);
}

/// Propagates working colors to the tool's swatches. With no selection both slots refresh; with a selection, only slots tracking the working color.
pub fn apply_working_colors(drawing: &mut DrawingToolState, global: &DocumentToolData, document: &DocumentMessageHandler) {
	refresh_slot_working_color(&mut drawing.fill, fill_working_color(global, drawing.colors_swapped), document);
	refresh_slot_working_color(&mut drawing.stroke, stroke_working_color(global, drawing.colors_swapped), document);
}

/// Refreshes a single swatch from the given working color, subject to the rules in [`apply_working_colors`].
pub fn refresh_slot_working_color(slot: &mut ToolColorOptions, working_color: Color, document: &DocumentMessageHandler) {
	if slot.fill_choice.is_some() && (!has_selection(document) || slot.tracks_working_color) {
		slot.fill_choice = Some(solid(working_color));
	}
}

/// Resets the tool's swatches to the working colors. Called on tool deactivation and shape-mode changes.
pub fn reset_colors_on_deactivation(drawing: &mut DrawingToolState, global: &DocumentToolData) {
	drawing.fill.fill_choice = Some(solid(fill_working_color(global, drawing.colors_swapped)));
	drawing.stroke.fill_choice = Some(solid(stroke_working_color(global, drawing.colors_swapped)));
	drawing.fill.tracks_working_color = true;
	drawing.stroke.tracks_working_color = true;
}

/// Handles the "Swap Fill/Stroke" button. Stroke can only hold a solid color, so a gradient fill collapses to `None` when moved.
pub fn swap_fill_and_stroke(drawing: &mut DrawingToolState, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	drawing.colors_swapped = !drawing.colors_swapped;

	if has_selection(document) {
		responses.add(DocumentMessage::AddTransaction);
	}

	// The new fill takes the old stroke's value as-is; the new stroke takes the old fill (with any gradient collapsed to `None`,
	// since stroke can only hold a solid color). `None` (mixed) on either side propagates as `None` to the other.
	let new_fill = drawing.stroke.fill_choice.clone();
	let new_stroke = drawing.fill.fill_choice.as_ref().map(|c| c.as_solid().map_or(FillChoice::None, FillChoice::Solid));
	let (new_fill_tracks, new_stroke_tracks) = (drawing.stroke.tracks_working_color, drawing.fill.tracks_working_color);

	drawing.fill.fill_choice = new_fill.clone();
	drawing.stroke.fill_choice = new_stroke.clone();
	drawing.fill.tracks_working_color = new_fill_tracks;
	drawing.stroke.tracks_working_color = new_stroke_tracks;

	if has_selection(document) {
		// Apply to layers only when we have a concrete value (`None` means mixed, no single value to broadcast).
		if drawing.fill.is_active()
			&& let Some(choice) = new_fill
		{
			graph_modification_utils::set_fill_for_selected_layers(choice, document, responses);
		}
		if drawing.stroke.is_active()
			&& let Some(choice) = new_stroke
		{
			graph_modification_utils::set_stroke_color_for_selected_layers(choice.as_solid(), drawing.effective_line_weight(), document, responses);
		}
	}
}

/// Updates the cache and returns `true` if the current selection differs from the last-synced one. Cache stays sorted to keep comparisons cheap.
pub fn selection_changed_since_last_sync(last_synced: &mut Vec<LayerNodeIdentifier>, document: &DocumentMessageHandler) -> bool {
	let mut current: Vec<LayerNodeIdentifier> = document.network_interface.selected_nodes().selected_layers_except_artboards(&document.network_interface).collect();

	current.sort();

	let changed = current != *last_synced;
	*last_synced = current;
	changed
}

/// How the weight widget should update from inspecting selected layers' strokes.
pub enum WeightSyncOutcome {
	/// All strokes share this weight.
	Set(f64),
	/// Stroke weights differ (or some layers lack a stroke): show the mixed dash.
	Mixed,
	/// Selection has no strokes: reset to the tool's default on a real selection change, otherwise preserve.
	NoStrokes,
	/// No selection: preserve the current value.
	NoSelection,
}

/// Inspects the selection and returns how the weight widget should update.
pub fn compute_weight_sync(document: &DocumentMessageHandler) -> WeightSyncOutcome {
	let layers: Vec<_> = document.network_interface.selected_nodes().selected_layers_except_artboards(&document.network_interface).collect();

	if layers.is_empty() {
		return WeightSyncOutcome::NoSelection;
	}

	let stroke_weights: Vec<f64> = layers.iter().filter_map(|l| graph_modification_utils::get_stroke_width(*l, &document.network_interface)).collect();

	if stroke_weights.is_empty() {
		return WeightSyncOutcome::NoStrokes;
	}

	if stroke_weights.len() != layers.len() {
		return WeightSyncOutcome::Mixed;
	}

	let first = stroke_weights[0];
	let all_same = stroke_weights.iter().all(|&w| (w - first).abs() < f64::EPSILON * 100.);
	if all_same { WeightSyncOutcome::Set(first) } else { WeightSyncOutcome::Mixed }
}
