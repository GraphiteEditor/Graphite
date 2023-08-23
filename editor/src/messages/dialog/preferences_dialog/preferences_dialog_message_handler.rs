use graph_craft::imaginate_input::ImaginateServerBackend;

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

		self.send_layout(responses, LayoutTarget::DialogDetails, preferences);
	}

	advertise_actions! {PreferencesDialogUpdate;}
}

impl PreferencesDialogMessageHandler {
	pub fn send_layout(&self, responses: &mut VecDeque<Message>, layout_target: LayoutTarget, preferences: &PreferencesMessageHandler) {
		responses.add(LayoutMessage::SendLayout {
			layout: self.layout(preferences),
			layout_target,
		})
	}

	fn layout(&self, preferences: &PreferencesMessageHandler) -> Layout {
		let zoom_with_scroll = vec![
			TextLabel::new("Input").min_width(100).italic(true).widget_holder(),
			TextLabel::new("Zoom with Scroll").min_width(120).widget_holder(),
			CheckboxInput::new(preferences.zoom_with_scroll)
				.tooltip("Use the scroll wheel for zooming instead of vertically panning (not recommended for trackpads)")
				.on_update(|checkbox_input: &CheckboxInput| {
					PreferencesMessage::InputZoomWithScroll {
						zoom_with_scroll: checkbox_input.checked,
					}
					.into()
				})
				.widget_holder(),
		];

		let imaginate_server_backend = vec![
			TextLabel::new("Imaginate").min_width(100).italic(true).widget_holder(),
			TextLabel::new("Server Backend").min_width(120).widget_holder(),
			RadioInput::new(vec![
				RadioEntryData::new("Hosted")
					.on_update(|_| {
						PreferencesMessage::ImaginateServerBackend {
							backend: ImaginateServerBackend::Hosted,
						}
						.into()
					})
					.tooltip("Hosted and paid for by the Graphite project. Please consider visiting the website to donate in order to keep this service running."),
				RadioEntryData::new("Self-Hosted")
					.on_update(|_| {
						PreferencesMessage::ImaginateServerBackend {
							backend: ImaginateServerBackend::SelfHosted,
						}
						.into()
					})
					.tooltip("Run your own server locally or on a remote machine. See the documentation for more information. This option provides several additional features."),
			])
			.selected_index(match preferences.imaginate_server_backend {
				ImaginateServerBackend::Hosted => 0,
				ImaginateServerBackend::SelfHosted => 1,
			})
			.expand_to_fit_width(true)
			.widget_holder(),
			Separator::new(SeparatorType::Related).widget_holder(),
			IconButton::new("Info", 24)
				.tooltip("Self-Hosting Documentation")
				.on_update(|_| {
					FrontendMessage::TriggerVisitLink {
						url: "https://github.com/GraphiteEditor/Graphite/discussions/1089".to_string(),
					}
					.into()
				})
				.widget_holder(),
		];

		let imaginate_server_hostname = vec![
			TextLabel::new("").min_width(100).italic(true).widget_holder(),
			TextLabel::new("Server Hostname")
				.min_width(120)
				.disabled(preferences.imaginate_server_backend == ImaginateServerBackend::Hosted)
				.widget_holder(),
			TextInput::new(&preferences.imaginate_server_hostname)
				.min_width(200)
				.on_update(|text_input: &TextInput| PreferencesMessage::ImaginateServerHostname { hostname: text_input.value.clone() }.into())
				.disabled(preferences.imaginate_server_backend == ImaginateServerBackend::Hosted)
				.widget_holder(),
		];

		let imaginate_refresh_frequency = vec![
			TextLabel::new("").min_width(100).widget_holder(),
			TextLabel::new("Refresh Frequency")
				.min_width(120)
				.disabled(preferences.imaginate_server_backend == ImaginateServerBackend::Hosted)
				.widget_holder(),
			NumberInput::new(Some(preferences.imaginate_refresh_frequency))
				.unit(" seconds")
				.min(0.)
				.max((1u64 << std::f64::MANTISSA_DIGITS) as f64)
				.min_width(200)
				.on_update(|number_input: &NumberInput| PreferencesMessage::ImaginateRefreshFrequency { seconds: number_input.value.unwrap() }.into())
				.disabled(preferences.imaginate_server_backend == ImaginateServerBackend::Hosted)
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
			LayoutGroup::Row { widgets: imaginate_server_backend },
			LayoutGroup::Row { widgets: imaginate_server_hostname },
			LayoutGroup::Row { widgets: imaginate_refresh_frequency },
			LayoutGroup::Row { widgets: button_widgets },
		]))
	}
}
