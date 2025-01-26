use crate::messages::layout::utility_types::widget_prelude::*;
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
		let selection_section = vec![TextLabel::new("Selection").italic(true).widget_holder()];
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

		let zoom_with_scroll_tooltip = "Use the scroll wheel for zooming instead of vertically panning (not recommended for trackpads)";
		let input_section = vec![TextLabel::new("Input").italic(true).widget_holder()];
		let zoom_with_scroll = vec![
			CheckboxInput::new(preferences.zoom_with_scroll)
				.tooltip(zoom_with_scroll_tooltip)
				.on_update(|checkbox_input: &CheckboxInput| {
					PreferencesMessage::ModifyLayout {
						zoom_with_scroll: checkbox_input.checked,
					}
					.into()
				})
				.widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			TextLabel::new("Zoom with Scroll").table_align(true).tooltip(zoom_with_scroll_tooltip).widget_holder(),
		];

		let vello_tooltip = "Use the experimental Vello renderer (your browser must support WebGPU)";
		let renderer_section = vec![TextLabel::new("Experimental").italic(true).widget_holder()];
		let use_vello = vec![
			CheckboxInput::new(preferences.use_vello && preferences.supports_wgpu())
				.tooltip(vello_tooltip)
				.disabled(!preferences.supports_wgpu())
				.on_update(|checkbox_input: &CheckboxInput| PreferencesMessage::UseVello { use_vello: checkbox_input.checked }.into())
				.widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			TextLabel::new("Vello Renderer")
				.table_align(true)
				.tooltip(vello_tooltip)
				.disabled(!preferences.supports_wgpu())
				.widget_holder(),
		];

		// TODO: Reenable when Imaginate is restored
		// let imaginate_server_hostname = vec![
		// 	TextLabel::new("Imaginate").min_width(60).italic(true).widget_holder(),
		// 	TextLabel::new("Server Hostname").table_align(true).widget_holder(),
		// 	Separator::new(SeparatorType::Unrelated).widget_holder(),
		// 	TextInput::new(&preferences.imaginate_server_hostname)
		// 		.min_width(200)
		// 		.on_update(|text_input: &TextInput| PreferencesMessage::ImaginateServerHostname { hostname: text_input.value.clone() }.into())
		// 		.widget_holder(),
		// ];
		// let imaginate_refresh_frequency = vec![
		// 	TextLabel::new("").min_width(60).widget_holder(),
		// 	TextLabel::new("Refresh Frequency").table_align(true).widget_holder(),
		// 	Separator::new(SeparatorType::Unrelated).widget_holder(),
		// 	NumberInput::new(Some(preferences.imaginate_refresh_frequency))
		// 		.unit(" seconds")
		// 		.min(0.)
		// 		.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
		// 		.min_width(200)
		// 		.on_update(|number_input: &NumberInput| PreferencesMessage::ImaginateRefreshFrequency { seconds: number_input.value.unwrap() }.into())
		// 		.widget_holder(),
		// ];

		let vector_mesh_tooltip = "Allow tools to produce vector meshes, where more than two segments can connect to an anchor point.\n\nCurrently this does not properly handle line joins and fills.";
		let vector_meshes = vec![
			CheckboxInput::new(preferences.vector_meshes)
				.tooltip(vector_mesh_tooltip)
				.on_update(|checkbox_input: &CheckboxInput| PreferencesMessage::VectorMeshes { enabled: checkbox_input.checked }.into())
				.widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			TextLabel::new("Vector Meshes").table_align(true).tooltip(vector_mesh_tooltip).widget_holder(),
		];

		Layout::WidgetLayout(WidgetLayout::new(vec![
			LayoutGroup::Row { widgets: selection_section },
			LayoutGroup::Row { widgets: vec![selection_mode] },
			LayoutGroup::Row { widgets: input_section },
			LayoutGroup::Row { widgets: zoom_with_scroll },
			LayoutGroup::Row { widgets: renderer_section },
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
