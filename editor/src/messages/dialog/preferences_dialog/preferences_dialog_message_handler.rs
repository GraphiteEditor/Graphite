use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

/// A dialog to allow users to customize Graphite editor options
#[derive(Debug, Clone, Default)]
pub struct PreferencesDialogMessageHandler {}

impl MessageHandler<PreferencesDialogMessage, &PreferencesMessageHandler> for PreferencesDialogMessageHandler {
	fn process_message(&mut self, message: PreferencesDialogMessage, responses: &mut VecDeque<Message>, preferences: &PreferencesMessageHandler) {
		match message {
			PreferencesDialogMessage::Confirm => {}
		}

		self.register_properties(responses, LayoutTarget::DialogDetails, preferences);
	}

	advertise_actions! {PreferencesDialogUpdate;}
}

impl PreferencesDialogMessageHandler {
	pub fn register_properties(&self, responses: &mut VecDeque<Message>, layout_target: LayoutTarget, preferences: &PreferencesMessageHandler) {
		responses.add(LayoutMessage::SendLayout {
			layout: self.properties(preferences),
			layout_target,
		})
	}

	fn properties(&self, preferences: &PreferencesMessageHandler) -> Layout {
		let zoom_with_scroll = vec![
			TextLabel::new("Input").min_width(60).italic(true).widget_holder(),
			TextLabel::new("Zoom with Scroll").table_align(true).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			CheckboxInput::new(preferences.zoom_with_scroll)
				.tooltip("Use the scroll wheel for zooming instead of vertically panning (not recommended for trackpads)")
				.on_update(|checkbox_input: &CheckboxInput| {
					PreferencesMessage::ModifyLayout {
						zoom_with_scroll: checkbox_input.checked,
					}
					.into()
				})
				.widget_holder(),
		];

		let imaginate_server_hostname = vec![
			TextLabel::new("Imaginate").min_width(60).italic(true).widget_holder(),
			TextLabel::new("Server Hostname").table_align(true).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			TextInput::new(&preferences.imaginate_server_hostname)
				.min_width(200)
				.on_update(|text_input: &TextInput| PreferencesMessage::ImaginateServerHostname { hostname: text_input.value.clone() }.into())
				.widget_holder(),
		];

		let imaginate_refresh_frequency = vec![
			TextLabel::new("").min_width(60).widget_holder(),
			TextLabel::new("Refresh Frequency").table_align(true).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			NumberInput::new(Some(preferences.imaginate_refresh_frequency))
				.unit(" seconds")
				.min(0.)
				.min_width(200)
				.on_update(|number_input: &NumberInput| PreferencesMessage::ImaginateRefreshFrequency { seconds: number_input.value.unwrap() }.into())
				.widget_holder(),
		];

		let button_widgets = vec![
			TextButton::new("Ok")
				.min_width(96)
				.emphasized(true)
				.on_update(|_| {
					DialogMessage::CloseDialogAndThen {
						followups: vec![PreferencesDialogMessage::Confirm.into()],
					}
					.into()
				})
				.widget_holder(),
			TextButton::new("Reset to Defaults")
				.min_width(96)
				.on_update(|_| PreferencesMessage::ResetToDefaults.into())
				.widget_holder(),
		];

		Layout::WidgetLayout(WidgetLayout::new(vec![
			LayoutGroup::Row {
				widgets: vec![TextLabel::new("Editor Preferences").bold(true).widget_holder()],
			},
			LayoutGroup::Row { widgets: zoom_with_scroll },
			LayoutGroup::Row { widgets: imaginate_server_hostname },
			LayoutGroup::Row { widgets: imaginate_refresh_frequency },
			LayoutGroup::Row { widgets: button_widgets },
		]))
	}
}
