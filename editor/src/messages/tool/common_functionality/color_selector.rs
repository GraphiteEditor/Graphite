use crate::consts::DEFAULT_STROKE_WIDTH;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::utility_types::DocumentToolData;
use graphene_std::Color;
use graphene_std::vector::style::FillChoice;

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
			let stroke = graphene_std::vector::style::Stroke::new(Some(*color), weight);
			responses.add(GraphOperationMessage::StrokeSet { layer, stroke });
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
		let widget_value = self.fill_choice.clone().unwrap_or(FillChoice::None);
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
/// Bundles the weight, color, and selection-sync fields that would otherwise be duplicated across each tool's options struct.
/// The displayed fill/stroke colors track the global working colors.
pub struct DrawingToolState {
	/// The current stroke weight. `None` = mixed across selected layers.
	pub line_weight: Option<f64>,
	/// Persistent default weight, updated when the user edits the weight while no layer is selected.
	pub default_line_weight: f64,
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
}

/// Builds a `FillChoice::Solid` from a linear-space color, applying gamma conversion to display sRGB.
/// Common helper used throughout the color-syncing code where working colors (linear) flow into swatches that store gamma-encoded colors.
pub fn solid_gamma(color: Color) -> FillChoice {
	FillChoice::Solid(color.to_gamma_srgb())
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

/// Syncs fill and stroke options from the current selection, or to the (swap-routed) working colors when nothing is selected.
/// `selection_changed` is `true` when the document selection set differs from the last sync; when `false` (e.g., the same
/// selection just had a fill/stroke node toggled), display values for inactive states are preserved instead of being reset.
/// Returns `true` if anything changed (and the caller should refresh the layout).
pub fn sync_color_options(
	drawing: &mut DrawingToolState,
	natural_fill_enabled: bool,
	natural_stroke_enabled: bool,
	global: &DocumentToolData,
	document: &DocumentMessageHandler,
	selection_changed: bool,
) -> bool {
	let fill_fallback = solid_gamma(fill_working_color(global, drawing.colors_swapped));
	let stroke_fallback = solid_gamma(stroke_working_color(global, drawing.colors_swapped));

	let mut changed = false;

	// FILL

	let new_fill = if let Some(state) = graph_modification_utils::selected_fill_state(document) {
		// `display_choice` is the value stored in `fill_choice`. `None` means mixed (swatch renders a dash overlay).
		// A single-color selection is layer-derived (`tracks_working = false`). Mixed and fallback states track the working color live (`tracks_working = true`).
		let active = state.enabled == Some(true);
		let (display_choice, tracks_working) = match &state.fill_choice {
			Some(choice) if active => (Some(choice.clone()), false),
			Some(_) if selection_changed => (Some(fill_fallback.clone()), true),
			Some(_) => (drawing.fill.fill_choice.clone(), drawing.fill.tracks_working_color),
			None => (None, true),
		};
		(state.enabled, display_choice, tracks_working)
	} else {
		// No selection: on a real selection change (deselect), revert to the working color.
		// When already empty (e.g., "deselect all" with nothing selected), preserve the user's currently-displayed color.
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

/// Drives a drawing tool's full SelectionChanged update in one call: detects whether the selection changed, syncs fill/stroke
/// colors via [`sync_color_options`], then updates the stroke weight widget based on [`compute_weight_sync`]'s outcome.
/// Returns `true` if anything changed and the caller should refresh the layout.
pub fn sync_drawing_state(drawing: &mut DrawingToolState, natural_fill_enabled: bool, natural_stroke_enabled: bool, global: &DocumentToolData, document: &DocumentMessageHandler) -> bool {
	let selection_changed = selection_changed_since_last_sync(&mut drawing.last_synced_selection, document);
	let mut needs_refresh = sync_color_options(drawing, natural_fill_enabled, natural_stroke_enabled, global, document, selection_changed);

	let new_line_weight = match compute_weight_sync(document) {
		WeightSyncOutcome::Set(weight) => Some(weight),
		WeightSyncOutcome::Mixed => None,
		// On a real selection change, revert to the tool's default weight; otherwise preserve the current value.
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

	needs_refresh
}

/// Same as [`sync_color_options`] but for tools that only have a fill option (e.g., text). The fill follows the given working color when nothing is selected.
pub fn sync_fill_only(fill: &mut ToolColorOptions, natural_fill_enabled: bool, fill_color: Color, document: &DocumentMessageHandler, selection_changed: bool) -> bool {
	let fill_fallback = solid_gamma(fill_color);

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

/// Applies a user-picked fill color/gradient: updates the tool's displayed fill (re-enabling the checkbox), then either writes
/// it to selected layers or (when nothing is selected) pushes a solid pick to the global working color slot that drives the
/// fill swatch (secondary by default, primary when the tool's [`DrawingToolState::colors_swapped`] is set). Gradient and `None`
/// picks with no selection don't have a working-color destination, so they aren't propagated and revert on the next sync.
pub fn apply_fill_color_pick(drawing: &mut DrawingToolState, fill_choice: FillChoice, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	apply_fill_only_color_pick(&mut drawing.fill, fill_choice, drawing.colors_swapped, document, responses);
}

/// Bare [`ToolColorOptions`] counterpart of [`apply_fill_color_pick`] for tools that only carry a single fill slot (e.g. text).
/// `slot_is_primary` is the working-color slot the swatch is bound to: `true` when the fill routes to primary (text),
/// or for a [`DrawingToolState`]-backed tool pass `drawing.colors_swapped`.
pub fn apply_fill_only_color_pick(fill: &mut ToolColorOptions, fill_choice: FillChoice, slot_is_primary: bool, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	fill.fill_choice = Some(fill_choice.clone());
	fill.enabled = Some(true);
	// The user picked a specific value: no longer a working-color fallback.
	fill.tracks_working_color = false;
	if has_selection(document) {
		graph_modification_utils::set_fill_for_selected_layers(fill_choice, document, responses);
	} else if let FillChoice::Solid(color) = fill_choice {
		responses.add(ToolMessage::SelectWorkingColor { color, primary: slot_is_primary });
	}
}

/// Applies a user-picked stroke color: updates the tool's displayed stroke (re-enabling the checkbox), then either writes it
/// to selected layers or (when nothing is selected) pushes the pick to the global working color slot that drives the stroke
/// swatch (primary by default, secondary when the tool's [`DrawingToolState::colors_swapped`] is set).
pub fn apply_stroke_color_pick(drawing: &mut DrawingToolState, color: Option<Color>, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	drawing.stroke.fill_choice = Some(color.map_or(FillChoice::None, FillChoice::Solid));
	drawing.stroke.enabled = Some(true);
	// The user picked a specific value: no longer a working-color fallback.
	drawing.stroke.tracks_working_color = false;
	if has_selection(document) {
		graph_modification_utils::set_stroke_color_for_selected_layers(color, drawing.effective_line_weight(), document, responses);
	} else if let Some(color) = color {
		// Stroke maps to primary by default, or secondary when the link is swapped.
		responses.add(ToolMessage::SelectWorkingColor {
			color,
			primary: !drawing.colors_swapped,
		});
	}
}

/// Toggles the fill checkbox: when enabled, re-applies the preserved fill choice; when disabled, removes the fill node.
/// When unticking from a mixed selection, the saved fill_choice is replaced with the current working color and marked as a
/// working-color fallback, so the swatch follows the link while unticked rather than freezing on a per-layer color.
pub fn apply_fill_enabled(drawing: &mut DrawingToolState, enabled: bool, global: &DocumentToolData, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	apply_fill_only_enabled(&mut drawing.fill, enabled, fill_working_color(global, drawing.colors_swapped), document, responses);
}

/// Bare [`ToolColorOptions`] counterpart of [`apply_fill_enabled`] for tools that only carry a single fill slot (e.g. text).
/// `working_color` is the linear-space working color this slot tracks, used to fill in a fallback when re-ticking or unticking from a mixed state.
pub fn apply_fill_only_enabled(fill: &mut ToolColorOptions, enabled: bool, working_color: Color, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	fill.enabled = Some(enabled);
	if enabled {
		// Re-applying from a mixed state has no specific layer color to restore, so use the current working color and mark the
		// slot as a working-color fallback so it keeps tracking the link going forward.
		let fill_choice = fill.fill_choice.clone().unwrap_or_else(|| {
			fill.tracks_working_color = true;
			solid_gamma(working_color)
		});
		fill.fill_choice = Some(fill_choice.clone());
		graph_modification_utils::set_fill_for_selected_layers(fill_choice, document, responses);
	} else {
		// Unticking from a mixed state: no specific layer color to remember. Capture the current working color as the saved
		// value and mark it as a fallback, so the swatch follows the link while unticked and re-tick uses the live value.
		if fill.fill_choice.is_none() {
			fill.fill_choice = Some(solid_gamma(working_color));
			fill.tracks_working_color = true;
		}
		graph_modification_utils::remove_fill_for_selected_layers(document, responses);
	}
}

/// Toggles the stroke checkbox: when enabled, re-applies the preserved stroke color; when disabled, removes the stroke node.
/// When unticking from a mixed selection, the saved stroke is replaced with the current working color and marked as a
/// working-color fallback (mirroring [`apply_fill_enabled`]).
pub fn apply_stroke_enabled(drawing: &mut DrawingToolState, enabled: bool, global: &DocumentToolData, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	drawing.stroke.enabled = Some(enabled);
	if enabled {
		// Re-applying from a mixed state has no specific layer color to restore, so use the current working color and mark the
		// slot as a working-color fallback so it keeps tracking the link going forward.
		let stroke_choice = drawing.stroke.fill_choice.clone().unwrap_or_else(|| {
			drawing.stroke.tracks_working_color = true;
			solid_gamma(stroke_working_color(global, drawing.colors_swapped))
		});
		drawing.stroke.fill_choice = Some(stroke_choice.clone());
		graph_modification_utils::set_stroke_color_for_selected_layers(stroke_choice.as_solid(), drawing.effective_line_weight(), document, responses);
	} else {
		if drawing.stroke.fill_choice.is_none() {
			drawing.stroke.fill_choice = Some(solid_gamma(stroke_working_color(global, drawing.colors_swapped)));
			drawing.stroke.tracks_working_color = true;
		}
		graph_modification_utils::remove_stroke_for_selected_layers(document, responses);
	}
}

/// Applies a user-edited stroke weight: updates the tool's line weight, persists it as the no-selection default when nothing
/// is selected (so it survives selection cycles), and writes it to any selected layers.
pub fn apply_line_weight(drawing: &mut DrawingToolState, line_weight: f64, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	drawing.line_weight = Some(line_weight);
	if !has_selection(document) {
		drawing.default_line_weight = line_weight;
	}
	graph_modification_utils::set_stroke_weight_for_selected_layers(line_weight, document, responses);
}

/// Propagates the current (swap-routed) working colors to the tool's fill/stroke swatches. With no selection both slots always update.
/// With a selection, only slots marked as a working-color fallback (`tracks_working_color`) refresh, so the
/// saved/re-tick color follows the linked working color rather than going stale, while layer-derived colors are preserved.
pub fn apply_working_colors(drawing: &mut DrawingToolState, global: &DocumentToolData, document: &DocumentMessageHandler) {
	refresh_slot_working_color(&mut drawing.fill, fill_working_color(global, drawing.colors_swapped), document);
	refresh_slot_working_color(&mut drawing.stroke, stroke_working_color(global, drawing.colors_swapped), document);
}

/// Refreshes a single fill/stroke swatch's stored color from the given working color, subject to the same rules as [`apply_working_colors`]:
/// with no selection always refresh, with a selection only refresh if the slot is tracking the working color.
/// Skips mixed (`fill_choice = None`) slots, there's no stored value to refresh.
pub fn refresh_slot_working_color(slot: &mut ToolColorOptions, working_color: Color, document: &DocumentMessageHandler) {
	if slot.fill_choice.is_some() && (!has_selection(document) || slot.tracks_working_color) {
		slot.fill_choice = Some(solid_gamma(working_color));
	}
}

/// Resets the tool's displayed fill/stroke colors back to the (swap-routed) working colors.
/// Called on tool deactivation (Abort) and on shape-mode changes so the next activation starts fresh.
pub fn reset_colors_on_deactivation(drawing: &mut DrawingToolState, global: &DocumentToolData) {
	drawing.fill.fill_choice = Some(solid_gamma(fill_working_color(global, drawing.colors_swapped)));
	drawing.stroke.fill_choice = Some(solid_gamma(stroke_working_color(global, drawing.colors_swapped)));
	drawing.fill.tracks_working_color = true;
	drawing.stroke.tracks_working_color = true;
}

/// Handles the "Swap Fill/Stroke" button: toggles the tool's [`DrawingToolState::colors_swapped`] flag, swaps the displayed
/// fill and stroke locally so the next layout refresh shows the change, and applies the same swap to any selected layers.
/// Stroke can only hold a solid color, so a fill that was a gradient becomes `None` when it moves to stroke.
pub fn swap_fill_and_stroke(drawing: &mut DrawingToolState, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	drawing.colors_swapped = !drawing.colors_swapped;

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

/// Computes whether the current selection differs from the last-synced one, and updates the cache.
/// Returns `true` when the selection has actually changed (a different set of layers, or empty <-> non-empty).
pub fn selection_changed_since_last_sync(last_synced: &mut Vec<LayerNodeIdentifier>, document: &DocumentMessageHandler) -> bool {
	let current: Vec<LayerNodeIdentifier> = document.network_interface.selected_nodes().selected_layers_except_artboards(&document.network_interface).collect();

	let mut sorted_current = current.clone();
	sorted_current.sort();
	let mut sorted_last = last_synced.clone();
	sorted_last.sort();

	let changed = sorted_current != sorted_last;
	*last_synced = current;
	changed
}

/// Outcome of inspecting selected layers' stroke weights, used by tool control bars to decide between displaying a number,
/// rendering the "mixed" dash, or preserving the previous value.
pub enum WeightSyncOutcome {
	/// All selected layers (with strokes) share this weight: assign it to `line_weight`.
	Set(f64),
	/// Selected layers have differing stroke weights (or some lack a stroke): render the mixed dash.
	Mixed,
	/// All selected layers lack a stroke: on a real selection change, reset to the tool's default, otherwise preserve.
	NoStrokes,
	/// No layers are selected: preserve current value.
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
