use crate::consts::{VIEWPORT_ZOOM_WHEEL_RATE, VIEWPORT_ZOOM_WHEEL_RATE_CHANGE};
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::utility_types::wires::GraphWireStyle;
use crate::messages::preferences::SelectionMode;
use crate::messages::prelude::*;
use graphene_std::render_node::{EditorPreferences, wgpu_available};

#[derive(ExtractField)]
pub struct PreferencesDialogMessageContext<'a> {
	pub preferences: &'a PreferencesMessageHandler,
}

/// A dialog to allow users to customize Graphite editor options
#[derive(Debug, Clone, Default, ExtractField)]
pub struct PreferencesDialogMessageHandler {
	unmodified_preferences: Option<PreferencesMessageHandler>,
}

#[message_handler_data]
impl MessageHandler<PreferencesDialogMessage, PreferencesDialogMessageContext<'_>> for PreferencesDialogMessageHandler {
	fn process_message(&mut self, message: PreferencesDialogMessage, responses: &mut VecDeque<Message>, context: PreferencesDialogMessageContext) {
		let PreferencesDialogMessageContext { preferences } = context;
		match message {
			PreferencesDialogMessage::MayRequireRestart => {
				if self.unmodified_preferences.is_none() {
					self.unmodified_preferences = Some(preferences.clone());
				}
			}
			PreferencesDialogMessage::Confirm => {
				if let Some(unmodified_preferences) = &self.unmodified_preferences
					&& unmodified_preferences.needs_restart(preferences)
				{
					responses.add(DialogMessage::RequestConfirmRestartDialog);
				} else {
					responses.add(DialogMessage::Close);
				}
			}
			PreferencesDialogMessage::Update => {}
		}
	}

	advertise_actions!(PreferencesDialogUpdate;
	);
}

// This doesn't actually implement the `DialogLayoutHolder` trait like the other dialog message handlers.
// That's because we need to give `send_layout` the `preferences` argument, which is not part of the trait.
// However, it's important to keep the methods in sync with those from the trait for consistency.
impl PreferencesDialogMessageHandler {
	const ICON: &'static str = "Settings";
	const TITLE: &'static str = "Editor Preferences";

	fn layout(&self, preferences: &PreferencesMessageHandler) -> Layout {
		let mut rows = Vec::new();

		// ==========
		// NAVIGATION
		// ==========
		{
			let header = vec![TextLabel::new("Navigation").italic(true).widget_instance()];

			let zoom_rate_description = "
				Adjust how fast zooming occurs when using the scroll wheel or pinch gesture.\n\
				\n\
				*Default: 50.*
				"
			.trim();
			let zoom_rate_label = vec![
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				TextLabel::new("Zoom Rate").tooltip_label("Zoom Rate").tooltip_description(zoom_rate_description).widget_instance(),
			];
			let zoom_rate = vec![
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				NumberInput::new(Some(map_zoom_rate_to_display(preferences.viewport_zoom_wheel_rate)))
					.tooltip_label("Zoom Rate")
					.tooltip_description(zoom_rate_description)
					.mode_range()
					.int()
					.min(1.)
					.max(100.)
					.on_update(|number_input: &NumberInput| {
						if let Some(display_value) = number_input.value {
							let actual_rate = map_display_to_zoom_rate(display_value);
							PreferencesMessage::ViewportZoomWheelRate { rate: actual_rate }.into()
						} else {
							PreferencesMessage::ViewportZoomWheelRate { rate: VIEWPORT_ZOOM_WHEEL_RATE }.into()
						}
					})
					.widget_instance(),
			];

			let checkbox_id = CheckboxId::new();
			let zoom_with_scroll_description = "
				Use the scroll wheel for zooming instead of vertically panning (not recommended for trackpads).\n\
				\n\
				*Default: Off.*
				"
			.trim();
			let zoom_with_scroll = vec![
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				CheckboxInput::new(preferences.zoom_with_scroll)
					.tooltip_label("Zoom with Scroll")
					.tooltip_description(zoom_with_scroll_description)
					.on_update(|checkbox_input: &CheckboxInput| {
						PreferencesMessage::ModifyLayout {
							zoom_with_scroll: checkbox_input.checked,
						}
						.into()
					})
					.for_label(checkbox_id)
					.widget_instance(),
				TextLabel::new("Zoom with Scroll")
					.tooltip_label("Zoom with Scroll")
					.tooltip_description(zoom_with_scroll_description)
					.for_checkbox(checkbox_id)
					.widget_instance(),
			];

			rows.extend_from_slice(&[header, zoom_rate_label, zoom_rate, zoom_with_scroll]);
		}

		// =======
		// EDITING
		// =======
		{
			let header = vec![TextLabel::new("Editing").italic(true).widget_instance()];

			let selection_label_description = "
				Choose how targets are selected within dragged rectangular and lasso areas.\n\
				\n\
				*Default: Touched.*
				"
			.trim();
			let selection_label = vec![
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				TextLabel::new("Selection")
					.tooltip_label("Selection")
					.tooltip_description(selection_label_description)
					.widget_instance(),
			];

			let selection_mode = RadioInput::new(vec![
				RadioEntryData::new(SelectionMode::Touched.to_string())
					.label(SelectionMode::Touched.to_string())
					.tooltip_label(SelectionMode::Touched.to_string())
					.tooltip_description(SelectionMode::Touched.tooltip_description())
					.on_update(move |_| {
						PreferencesMessage::SelectionMode {
							selection_mode: SelectionMode::Touched,
						}
						.into()
					}),
				RadioEntryData::new(SelectionMode::Enclosed.to_string())
					.label(SelectionMode::Enclosed.to_string())
					.tooltip_label(SelectionMode::Enclosed.to_string())
					.tooltip_description(SelectionMode::Enclosed.tooltip_description())
					.on_update(move |_| {
						PreferencesMessage::SelectionMode {
							selection_mode: SelectionMode::Enclosed,
						}
						.into()
					}),
				RadioEntryData::new(SelectionMode::Directional.to_string())
					.label(SelectionMode::Directional.to_string())
					.tooltip_label(SelectionMode::Directional.to_string())
					.tooltip_description(SelectionMode::Directional.tooltip_description())
					.on_update(move |_| {
						PreferencesMessage::SelectionMode {
							selection_mode: SelectionMode::Directional,
						}
						.into()
					}),
			])
			.selected_index(Some(preferences.selection_mode as u32))
			.widget_instance();
			let selection_mode = vec![
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				selection_mode,
			];

			rows.extend_from_slice(&[header, selection_label, selection_mode]);
		}

		// =========
		// INTERFACE
		// =========
		#[cfg(not(target_family = "wasm"))]
		{
			let header = vec![TextLabel::new("Interface").italic(true).widget_instance()];

			let scale_description = "
				Adjust the scale of the entire user interface.\n\
				\n\
				*Default: 100%.*
				"
			.trim();
			let scale_label = vec![
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				TextLabel::new("Scale").tooltip_label("Scale").tooltip_description(scale_description).widget_instance(),
			];
			let scale = vec![
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				NumberInput::new(Some(ui_scale_to_display(preferences.ui_scale)))
					.tooltip_label("Scale")
					.tooltip_description(scale_description)
					.mode_range()
					.int()
					.min(ui_scale_to_display(crate::consts::UI_SCALE_MIN))
					.max(ui_scale_to_display(crate::consts::UI_SCALE_MAX))
					.unit("%")
					.on_update(|number_input: &NumberInput| {
						if let Some(display_value) = number_input.value {
							let scale = map_display_to_ui_scale(display_value);
							PreferencesMessage::UIScale { scale }.into()
						} else {
							PreferencesMessage::UIScale {
								scale: crate::consts::UI_SCALE_DEFAULT,
							}
							.into()
						}
					})
					.widget_instance(),
			];

			rows.extend_from_slice(&[header, scale_label, scale]);
		}

		// ============
		// EXPERIMENTAL
		// ============
		{
			let header = vec![TextLabel::new("Experimental").italic(true).widget_instance()];

			let node_graph_section_description = "
				Configure the appearance of the wires running between node connections in the graph.\n\
				\n\
				*Default: Direct.*
				"
			.trim();
			let node_graph_wires_label = vec![
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				TextLabel::new("Node Graph Wires")
					.tooltip_label("Node Graph Wires")
					.tooltip_description(node_graph_section_description)
					.widget_instance(),
			];
			let graph_wire_style = RadioInput::new(vec![
				RadioEntryData::new(GraphWireStyle::Direct.to_string())
					.label(GraphWireStyle::Direct.to_string())
					.tooltip_label(GraphWireStyle::Direct.to_string())
					.tooltip_description(GraphWireStyle::Direct.tooltip_description())
					.on_update(move |_| PreferencesMessage::GraphWireStyle { style: GraphWireStyle::Direct }.into()),
				RadioEntryData::new(GraphWireStyle::GridAligned.to_string())
					.label(GraphWireStyle::GridAligned.to_string())
					.tooltip_label(GraphWireStyle::GridAligned.to_string())
					.tooltip_description(GraphWireStyle::GridAligned.tooltip_description())
					.on_update(move |_| PreferencesMessage::GraphWireStyle { style: GraphWireStyle::GridAligned }.into()),
			])
			.selected_index(Some(preferences.graph_wire_style as u32))
			.widget_instance();
			let graph_wire_style = vec![
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				graph_wire_style,
			];

			let checkbox_id = CheckboxId::new();
			let brush_tool_description = "
				Enable the Brush tool to support basic raster-based layer painting.\n\
				\n\
				This legacy experimental tool has performance and quality limitations and is slated for replacement in future versions of Graphite that will have a renewed focus on raster graphics editing.\n\
				\n\
				Content created with the Brush tool may not be compatible with future versions of Graphite.\n\
				\n\
				*Default: Off.*
				"
			.trim();
			let brush_tool = vec![
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				Separator::new(SeparatorStyle::Unrelated).widget_instance(),
				CheckboxInput::new(preferences.brush_tool)
					.tooltip_label("Brush Tool")
					.tooltip_description(brush_tool_description)
					.on_update(|checkbox_input: &CheckboxInput| PreferencesMessage::BrushTool { enabled: checkbox_input.checked }.into())
					.for_label(checkbox_id)
					.widget_instance(),
				TextLabel::new("Brush Tool")
					.tooltip_label("Brush Tool")
					.tooltip_description(brush_tool_description)
					.for_checkbox(checkbox_id)
					.widget_instance(),
			];

			rows.extend_from_slice(&[header, node_graph_wires_label, graph_wire_style, brush_tool]);
		}

		// =============
		// COMPATIBILITY
		// =============
		{
			let wgpu_available = wgpu_available().unwrap_or(false);
			let is_desktop = cfg!(not(target_family = "wasm"));
			if wgpu_available || is_desktop {
				let header = vec![TextLabel::new("Compatibility").italic(true).widget_instance()];
				rows.push(header);
			}

			if wgpu_available {
				let render_tile_resolution_description = "
					Maximum X or Y resolution per render tile. Larger tiles may improve performance but can cause flickering or missing content in complex artwork if set too high.\n\
					\n\
					*Default: 1280 px.*
					"
				.trim();
				let render_tile_resolution_label = vec![
					Separator::new(SeparatorStyle::Unrelated).widget_instance(),
					Separator::new(SeparatorStyle::Unrelated).widget_instance(),
					TextLabel::new("Render Tile Resolution")
						.tooltip_label("Render Tile Resolution")
						.tooltip_description(render_tile_resolution_description)
						.widget_instance(),
				];
				let render_tile_resolution = vec![
					Separator::new(SeparatorStyle::Unrelated).widget_instance(),
					Separator::new(SeparatorStyle::Unrelated).widget_instance(),
					NumberInput::new(Some(preferences.max_render_region_size as f64))
						.tooltip_label("Render Tile Resolution")
						.tooltip_description(render_tile_resolution_description)
						.mode_range()
						.int()
						.min(256.)
						.max(4096.)
						.increment_step(256.)
						.unit(" px")
						.on_update(|number_input: &NumberInput| {
							let size = number_input.value.unwrap_or(EditorPreferences::default().max_render_region_size as f64) as u32;
							PreferencesMessage::MaxRenderRegionSize { size }.into()
						})
						.widget_instance(),
				];

				rows.extend_from_slice(&[render_tile_resolution_label, render_tile_resolution]);
			}

			if is_desktop {
				let ui_acceleration_description = "
					Use the CPU to draw the Graphite user interface (areas outside of the canvas) instead of the GPU. This does not affect the rendering of artwork in the canvas, which remains hardware accelerated.\n\
					\n\
					Disabling UI acceleration may slightly degrade performance, so this should be used as a workaround only if issues are observed with displaying the UI. This setting may become enabled automatically if Graphite launches, detects that it cannot draw the UI normally, and restarts in compatibility mode.\n\
					\n\
					*Default: Off.*
					"
				.trim();

				let checkbox_id = CheckboxId::new();
				let ui_acceleration = vec![
					Separator::new(SeparatorStyle::Unrelated).widget_instance(),
					Separator::new(SeparatorStyle::Unrelated).widget_instance(),
					CheckboxInput::new(preferences.disable_ui_acceleration)
						.tooltip_label("Disable UI Acceleration")
						.tooltip_description(ui_acceleration_description)
						.on_update(|number_input: &CheckboxInput| Message::Batched {
							messages: Box::new([
								PreferencesDialogMessage::MayRequireRestart.into(),
								PreferencesMessage::DisableUIAcceleration {
									disable_ui_acceleration: number_input.checked,
								}
								.into(),
							]),
						})
						.for_label(checkbox_id)
						.widget_instance(),
					TextLabel::new("Disable UI Acceleration")
						.tooltip_label("Disable UI Acceleration")
						.tooltip_description(ui_acceleration_description)
						.for_checkbox(checkbox_id)
						.widget_instance(),
				];

				rows.push(ui_acceleration);
			}
		}

		Layout(rows.into_iter().map(|r| LayoutGroup::Row { widgets: r }).collect())
	}

	pub fn send_layout(&self, responses: &mut VecDeque<Message>, layout_target: LayoutTarget, preferences: &PreferencesMessageHandler) {
		responses.add(LayoutMessage::SendLayout {
			layout: self.layout(preferences),
			layout_target,
		})
	}

	fn layout_column_2(&self) -> Layout {
		Layout::default()
	}

	fn send_layout_column_2(&self, responses: &mut VecDeque<Message>, layout_target: LayoutTarget) {
		responses.add(LayoutMessage::SendLayout {
			layout: self.layout_column_2(),
			layout_target,
		});
	}

	fn layout_buttons(&self) -> Layout {
		let widgets = vec![
			TextButton::new("OK").emphasized(true).on_update(|_| PreferencesDialogMessage::Confirm.into()).widget_instance(),
			TextButton::new("Reset to Defaults").on_update(|_| PreferencesMessage::ResetToDefaults.into()).widget_instance(),
		];

		Layout(vec![LayoutGroup::Row { widgets }])
	}

	fn send_layout_buttons(&self, responses: &mut VecDeque<Message>, layout_target: LayoutTarget) {
		responses.add(LayoutMessage::SendLayout {
			layout: self.layout_buttons(),
			layout_target,
		});
	}

	pub fn send_dialog_to_frontend(&self, responses: &mut VecDeque<Message>, preferences: &PreferencesMessageHandler) {
		self.send_layout(responses, LayoutTarget::DialogColumn1, preferences);
		self.send_layout_column_2(responses, LayoutTarget::DialogColumn2);
		self.send_layout_buttons(responses, LayoutTarget::DialogButtons);
		responses.add(FrontendMessage::DisplayDialog {
			icon: Self::ICON.into(),
			title: Self::TITLE.into(),
		});
	}
}

/// Maps display values (1-100) to actual zoom rates.
fn map_display_to_zoom_rate(display: f64) -> f64 {
	// Calculate the relative distance from the reference point (50)
	let distance_from_reference = display - 50.;
	let scaling_factor = (VIEWPORT_ZOOM_WHEEL_RATE_CHANGE * distance_from_reference / 50.).exp();
	VIEWPORT_ZOOM_WHEEL_RATE * scaling_factor
}

/// Maps actual zoom rates back to display values (1-100).
fn map_zoom_rate_to_display(rate: f64) -> f64 {
	// Calculate the scaling factor from the reference rate
	let scaling_factor = rate / VIEWPORT_ZOOM_WHEEL_RATE;
	let distance_from_reference = 50. * scaling_factor.ln() / VIEWPORT_ZOOM_WHEEL_RATE_CHANGE;
	let display = 50. + distance_from_reference;
	display.clamp(1., 100.).round()
}

/// Maps display values in percent to actual ui scale.
#[cfg(not(target_family = "wasm"))]
fn map_display_to_ui_scale(display: f64) -> f64 {
	display / 100.
}

/// Maps actual ui scale back to display values in percent.
#[cfg(not(target_family = "wasm"))]
fn ui_scale_to_display(scale: f64) -> f64 {
	scale * 100.
}
