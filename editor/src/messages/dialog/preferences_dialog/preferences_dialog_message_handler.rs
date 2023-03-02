use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::layout::utility_types::widgets::button_widgets::TextButton;
use crate::messages::layout::utility_types::widgets::input_widgets::{CheckboxInput, NumberInput, TextInput};
use crate::messages::layout::utility_types::widgets::label_widgets::{Separator, SeparatorDirection, SeparatorType, TextLabel};
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
		responses.push_back(
			LayoutMessage::SendLayout {
				layout: self.properties(preferences),
				layout_target,
			}
			.into(),
		)
	}

	fn properties(&self, preferences: &PreferencesMessageHandler) -> Layout {
		let zoom_with_scroll = vec![
			WidgetHolder::new(Widget::TextLabel(TextLabel {
				value: "Input".into(),
				min_width: 60,
				italic: true,
				..Default::default()
			})),
			WidgetHolder::new(Widget::TextLabel(TextLabel {
				value: "Zoom with Scroll".into(),
				table_align: true,
				..Default::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Unrelated,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::CheckboxInput(CheckboxInput {
				checked: preferences.zoom_with_scroll,
				tooltip: "Use the scroll wheel for zooming instead of vertically panning (not recommended for trackpads)".into(),
				on_update: WidgetCallback::new(|checkbox_input: &CheckboxInput| {
					PreferencesMessage::ModifyLayout {
						zoom_with_scroll: checkbox_input.checked,
					}
					.into()
				}),
				..Default::default()
			})),
		];

		let imaginate_server_hostname = vec![
			WidgetHolder::new(Widget::TextLabel(TextLabel {
				value: "Imaginate".into(),
				min_width: 60,
				italic: true,
				..Default::default()
			})),
			WidgetHolder::new(Widget::TextLabel(TextLabel {
				value: "Server Hostname".into(),
				table_align: true,
				..Default::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Unrelated,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::TextInput(TextInput {
				value: preferences.imaginate_server_hostname.clone(),
				min_width: 200,
				on_update: WidgetCallback::new(|text_input: &TextInput| PreferencesMessage::ImaginateServerHostname { hostname: text_input.value.clone() }.into()),
				..Default::default()
			})),
		];

		let imaginate_refresh_frequency = vec![
			WidgetHolder::new(Widget::TextLabel(TextLabel { min_width: 60, ..Default::default() })),
			WidgetHolder::new(Widget::TextLabel(TextLabel {
				value: "Refresh Frequency".into(),
				table_align: true,
				..Default::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Unrelated,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::NumberInput(NumberInput {
				unit: " seconds".into(),
				value: Some(preferences.imaginate_refresh_frequency),
				min: Some(0.),
				min_width: 200,
				on_update: WidgetCallback::new(|number_input: &NumberInput| PreferencesMessage::ImaginateRefreshFrequency { seconds: number_input.value.unwrap() }.into()),
				..Default::default()
			})),
		];

		let button_widgets = vec![
			WidgetHolder::new(Widget::TextButton(TextButton {
				label: "Ok".to_string(),
				min_width: 96,
				emphasized: true,
				on_update: WidgetCallback::new(|_| {
					DialogMessage::CloseDialogAndThen {
						followups: vec![PreferencesDialogMessage::Confirm.into()],
					}
					.into()
				}),
				..Default::default()
			})),
			WidgetHolder::new(Widget::TextButton(TextButton {
				label: "Reset to Defaults".to_string(),
				min_width: 96,
				on_update: WidgetCallback::new(|_| PreferencesMessage::ResetToDefaults.into()),
				..Default::default()
			})),
		];

		Layout::WidgetLayout(WidgetLayout::new(vec![
			LayoutGroup::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: "Editor Preferences".to_string(),
					bold: true,
					..Default::default()
				}))],
			},
			LayoutGroup::Row { widgets: zoom_with_scroll },
			LayoutGroup::Row { widgets: imaginate_server_hostname },
			LayoutGroup::Row { widgets: imaginate_refresh_frequency },
			LayoutGroup::Row { widgets: button_widgets },
		]))
	}
}
