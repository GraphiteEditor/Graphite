use crate::consts::{VIEWPORT_ZOOM_WHEEL_RATE, VIEWPORT_ZOOM_WHEEL_RATE_CHANGE};
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::utility_types::wires::GraphWireStyle;
use crate::messages::preferences::SelectionMode;
use crate::messages::prelude::*;

#[derive(ExtractField)]
pub struct PreferencesDialogMessageContext<'a> {
	pub preferences: &'a PreferencesMessageHandler,
}

/// A dialog to allow users to customize Graphite editor options
#[derive(Debug, Clone, Default, ExtractField)]
pub struct PreferencesDialogMessageHandler {}

#[message_handler_data]
impl MessageHandler<PreferencesDialogMessage, PreferencesDialogMessageContext<'_>> for PreferencesDialogMessageHandler {
	fn process_message(&mut self, message: PreferencesDialogMessage, responses: &mut VecDeque<Message>, context: PreferencesDialogMessageContext) {
		let PreferencesDialogMessageContext { preferences } = context;

		match message {
			PreferencesDialogMessage::Confirm => {}
		}

		self.send_dialog_to_frontend(responses, preferences);
	}

	advertise_actions! {PreferencesDialogUpdate;}
}

// This doesn't actually implement the `DialogLayoutHolder` trait like the other dialog message handlers.
// That's because we need to give `send_layout` the `preferences` argument, which is not part of the trait.
// However, it's important to keep the methods in sync with those from the trait for consistency.
impl PreferencesDialogMessageHandler {
	const ICON: &'static str = "Settings";
	const TITLE: &'static str = "Editor Preferences";

	fn layout(&self, preferences: &PreferencesMessageHandler) -> Layout {
		// ==========
		// NAVIGATION
		// ==========

		let navigation_header = vec![TextLabel::new("Navigation").italic(true).widget_holder()];

		let zoom_rate_tooltip = "Adjust how fast zooming occurs when using the scroll wheel or pinch gesture (relative to a default of 50)";
		let zoom_rate_label = vec![
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			TextLabel::new("Zoom Rate").tooltip(zoom_rate_tooltip).widget_holder(),
		];
		let zoom_rate = vec![
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			NumberInput::new(Some(map_zoom_rate_to_display(preferences.viewport_zoom_wheel_rate)))
				.tooltip(zoom_rate_tooltip)
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
				.widget_holder(),
		];

		let checkbox_id = CheckboxId::new();
		let zoom_with_scroll_tooltip = "Use the scroll wheel for zooming instead of vertically panning (not recommended for trackpads)";
		let zoom_with_scroll = vec![
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			CheckboxInput::new(preferences.zoom_with_scroll)
				.tooltip(zoom_with_scroll_tooltip)
				.on_update(|checkbox_input: &CheckboxInput| {
					PreferencesMessage::ModifyLayout {
						zoom_with_scroll: checkbox_input.checked,
					}
					.into()
				})
				.for_label(checkbox_id)
				.widget_holder(),
			TextLabel::new("Zoom with Scroll")
				.table_align(true)
				.tooltip(zoom_with_scroll_tooltip)
				.for_checkbox(checkbox_id)
				.widget_holder(),
		];

		// =======
		// EDITING
		// =======

		let editing_header = vec![TextLabel::new("Editing").italic(true).widget_holder()];

		let selection_label = vec![
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			TextLabel::new("Selection").widget_holder(),
		];

		let selection_mode = RadioInput::new(vec![
			RadioEntryData::new(SelectionMode::Touched.to_string())
				.label(SelectionMode::Touched.to_string())
				.tooltip(SelectionMode::Touched.tooltip_description())
				.on_update(move |_| {
					PreferencesMessage::SelectionMode {
						selection_mode: SelectionMode::Touched,
					}
					.into()
				}),
			RadioEntryData::new(SelectionMode::Enclosed.to_string())
				.label(SelectionMode::Enclosed.to_string())
				.tooltip(SelectionMode::Enclosed.tooltip_description())
				.on_update(move |_| {
					PreferencesMessage::SelectionMode {
						selection_mode: SelectionMode::Enclosed,
					}
					.into()
				}),
			RadioEntryData::new(SelectionMode::Directional.to_string())
				.label(SelectionMode::Directional.to_string())
				.tooltip(SelectionMode::Directional.tooltip_description())
				.on_update(move |_| {
					PreferencesMessage::SelectionMode {
						selection_mode: SelectionMode::Directional,
					}
					.into()
				}),
		])
		.selected_index(Some(preferences.selection_mode as u32))
		.widget_holder();
		let selection_mode = vec![
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			selection_mode,
		];

		// ============
		// EXPERIMENTAL
		// ============

		let experimental_header = vec![TextLabel::new("Experimental").italic(true).widget_holder()];

		let node_graph_section_tooltip = "Appearance of the wires running between node connections in the graph";
		let node_graph_wires_label = vec![
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			TextLabel::new("Node Graph Wires").tooltip(node_graph_section_tooltip).widget_holder(),
		];
		let graph_wire_style = RadioInput::new(vec![
			RadioEntryData::new(GraphWireStyle::Direct.to_string())
				.label(GraphWireStyle::Direct.to_string())
				.tooltip(GraphWireStyle::Direct.tooltip_description())
				.on_update(move |_| PreferencesMessage::GraphWireStyle { style: GraphWireStyle::Direct }.into()),
			RadioEntryData::new(GraphWireStyle::GridAligned.to_string())
				.label(GraphWireStyle::GridAligned.to_string())
				.tooltip(GraphWireStyle::GridAligned.tooltip_description())
				.on_update(move |_| PreferencesMessage::GraphWireStyle { style: GraphWireStyle::GridAligned }.into()),
		])
		.selected_index(Some(preferences.graph_wire_style as u32))
		.widget_holder();
		let graph_wire_style = vec![
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			graph_wire_style,
		];

		let checkbox_id = CheckboxId::new();
		let vello_tooltip = "Use the experimental Vello renderer (your browser must support WebGPU)";
		let use_vello = vec![
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			CheckboxInput::new(preferences.use_vello && preferences.supports_wgpu())
				.tooltip(vello_tooltip)
				.disabled(!preferences.supports_wgpu())
				.on_update(|checkbox_input: &CheckboxInput| PreferencesMessage::UseVello { use_vello: checkbox_input.checked }.into())
				.for_label(checkbox_id)
				.widget_holder(),
			TextLabel::new("Vello Renderer")
				.table_align(true)
				.tooltip(vello_tooltip)
				.disabled(!preferences.supports_wgpu())
				.for_checkbox(checkbox_id)
				.widget_holder(),
		];

		let checkbox_id = CheckboxId::new();
		let vector_mesh_tooltip =
			"Allow tools to produce vector meshes, where more than two segments can connect to an anchor point.\n\nCurrently this does not properly handle stroke joins and fills.";
		let vector_meshes = vec![
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			CheckboxInput::new(preferences.vector_meshes)
				.tooltip(vector_mesh_tooltip)
				.on_update(|checkbox_input: &CheckboxInput| PreferencesMessage::VectorMeshes { enabled: checkbox_input.checked }.into())
				.for_label(checkbox_id)
				.widget_holder(),
			TextLabel::new("Vector Meshes").table_align(true).tooltip(vector_mesh_tooltip).for_checkbox(checkbox_id).widget_holder(),
		];

		Layout::WidgetLayout(WidgetLayout::new(vec![
			LayoutGroup::Row { widgets: navigation_header },
			LayoutGroup::Row { widgets: zoom_rate_label },
			LayoutGroup::Row { widgets: zoom_rate },
			LayoutGroup::Row { widgets: zoom_with_scroll },
			LayoutGroup::Row { widgets: editing_header },
			LayoutGroup::Row { widgets: selection_label },
			LayoutGroup::Row { widgets: selection_mode },
			LayoutGroup::Row { widgets: experimental_header },
			LayoutGroup::Row { widgets: node_graph_wires_label },
			LayoutGroup::Row { widgets: graph_wire_style },
			LayoutGroup::Row { widgets: use_vello },
			LayoutGroup::Row { widgets: vector_meshes },
		]))
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
			TextButton::new("OK")
				.emphasized(true)
				.on_update(|_| {
					DialogMessage::CloseDialogAndThen {
						followups: vec![PreferencesDialogMessage::Confirm.into()],
					}
					.into()
				})
				.widget_holder(),
			TextButton::new("Reset to Defaults").on_update(|_| PreferencesMessage::ResetToDefaults.into()).widget_holder(),
		];

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
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
