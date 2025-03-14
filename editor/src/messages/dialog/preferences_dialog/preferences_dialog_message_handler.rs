use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::node_graph::utility_types::GraphWireStyle;
use crate::messages::preferences::SelectionMode;
use crate::messages::prelude::*;

pub struct PreferencesDialogMessageData<'a> {
	pub preferences: &'a PreferencesMessageHandler,
}

/// A dialog to allow users to customize Graphite editor options
#[derive(Debug, Clone, Default)]
pub struct PreferencesDialogMessageHandler {}

impl MessageHandler<PreferencesDialogMessage, PreferencesDialogMessageData<'_>> for PreferencesDialogMessageHandler {
	fn process_message(&mut self, message: PreferencesDialogMessage, responses: &mut VecDeque<Message>, data: PreferencesDialogMessageData) {
		let PreferencesDialogMessageData { preferences } = data;

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
				.widget_holder(),
			TextLabel::new("Zoom with Scroll").table_align(true).tooltip(zoom_with_scroll_tooltip).widget_holder(),
		];

		let zoom_rate_tooltip = "Adjust how fast zooming occurs when using the scroll wheel";
		let zoom_rate = vec![
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			TextLabel::new("Zoom Rate: ").table_align(true).tooltip(zoom_rate_tooltip).widget_holder(),
			Separator::new(SeparatorType::Related).widget_holder(),
			NumberInput::new(Some(map_zoom_rate_to_display(preferences.viewport_zoom_wheel_rate)))
        .tooltip(zoom_rate_tooltip)
        .min(1.0)
        .max(100.0)
        .display_decimal_places(0)  // Display as whole numbers
        .on_update(|number_input: &NumberInput| {
            if let Some(display_value) = number_input.value {
                let actual_rate = map_display_to_zoom_rate(display_value);
                PreferencesMessage::ViewportZoomWheelRate { rate: actual_rate }.into()
            } else {
                PreferencesMessage::ViewportZoomWheelRate { rate: (1. / 600.) * 3. }.into()
            }
        })
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

		let vello_tooltip = "Use the experimental Vello renderer (your browser must support WebGPU)";
		let use_vello = vec![
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			CheckboxInput::new(preferences.use_vello && preferences.supports_wgpu())
				.tooltip(vello_tooltip)
				.disabled(!preferences.supports_wgpu())
				.on_update(|checkbox_input: &CheckboxInput| PreferencesMessage::UseVello { use_vello: checkbox_input.checked }.into())
				.widget_holder(),
			TextLabel::new("Vello Renderer")
				.table_align(true)
				.tooltip(vello_tooltip)
				.disabled(!preferences.supports_wgpu())
				.widget_holder(),
		];

		let vector_mesh_tooltip = "Allow tools to produce vector meshes, where more than two segments can connect to an anchor point.\n\nCurrently this does not properly handle line joins and fills.";
		let vector_meshes = vec![
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			CheckboxInput::new(preferences.vector_meshes)
				.tooltip(vector_mesh_tooltip)
				.on_update(|checkbox_input: &CheckboxInput| PreferencesMessage::VectorMeshes { enabled: checkbox_input.checked }.into())
				.widget_holder(),
			TextLabel::new("Vector Meshes").table_align(true).tooltip(vector_mesh_tooltip).widget_holder(),
		];

		// TODO: Reenable when Imaginate is restored
		// let imaginate_server_hostname = vec![
		// 	TextLabel::new("Imaginate").min_width(60).italic(true).widget_holder(),
		// 	TextLabel::new("Server Hostname").table_align(true).widget_holder(),
		// 	TextInput::new(&preferences.imaginate_server_hostname)
		// 		.min_width(200)
		// 		.on_update(|text_input: &TextInput| PreferencesMessage::ImaginateServerHostname { hostname: text_input.value.clone() }.into())
		// 		.widget_holder(),
		// ];
		// let imaginate_refresh_frequency = vec![
		// 	TextLabel::new("").min_width(60).widget_holder(),
		// 	TextLabel::new("Refresh Frequency").table_align(true).widget_holder(),
		// 	NumberInput::new(Some(preferences.imaginate_refresh_frequency))
		// 		.unit(" seconds")
		// 		.min(0.)
		// 		.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
		// 		.min_width(200)
		// 		.on_update(|number_input: &NumberInput| PreferencesMessage::ImaginateRefreshFrequency { seconds: number_input.value.unwrap() }.into())
		// 		.widget_holder(),
		// ];

		Layout::WidgetLayout(WidgetLayout::new(vec![
			LayoutGroup::Row { widgets: navigation_header },
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
			// LayoutGroup::Row { widgets: imaginate_server_hostname },
			// LayoutGroup::Row { widgets: imaginate_refresh_frequency },
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
// Function to map a value from one range to another using logarithmic scaling
pub fn map_log_range(value: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
	if value <= from_min {
		to_min
	} else if value >= from_max {
		to_max
	} else {
		// Calculate the logarithmic position between from_min and from_max
		let log_min = from_min.ln();
		let log_max = from_max.ln();
		let log_val = value.ln();

		// Map to to_min-to_max range
		let normalized = (log_val - log_min) / (log_max - log_min);
		to_min + (to_max - to_min) * normalized
	}
}

// Function to map a value from one range to another using linear scaling
pub fn map_linear_range(value: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
	if value <= from_min {
		to_min
	} else if value >= from_max {
		to_max
	} else {
		// Normalize to 0-1 range
		let normalized = (value - from_min) / (from_max - from_min);

		// Map to to_min-to_max range
		to_min + normalized * (to_max - to_min)
	}
}

// Map the actual zoom rate value to display value (1-100)
fn map_zoom_rate_to_display(rate: f64) -> f64 {
	map_log_range(rate, 0.0001, 0.05, 1.0, 100.0).round()
}

// Map the display value (1-100) back to the actual zoom rate value
fn map_display_to_zoom_rate(display: f64) -> f64 {
	let normalized = map_linear_range(display, 1.0, 100.0, 0.0, 1.0);

	let log_min = 0.0001_f64.ln();
	let log_max = 0.05_f64.ln();
	let log_val = log_min + normalized * (log_max - log_min);

	log_val.exp()
}
