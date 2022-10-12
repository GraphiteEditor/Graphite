use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::layout::utility_types::widgets::button_widgets::TextButton;
use crate::messages::layout::utility_types::widgets::input_widgets::TextInput;
use crate::messages::layout::utility_types::widgets::label_widgets::{Separator, SeparatorDirection, SeparatorType, TextLabel};
use crate::messages::prelude::*;

/// A dialog to allow users to customize Graphite editor options
#[derive(Debug, Clone, Default)]
pub struct PreferencesDialogMessageHandler {}

impl MessageHandler<PreferencesDialogMessage, &PreferencesMessageHandler> for PreferencesDialogMessageHandler {
	fn process_message(&mut self, message: PreferencesDialogMessage, preferences: &PreferencesMessageHandler, responses: &mut VecDeque<Message>) {
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
		let ai_artist_section_title = vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
			value: "AI Artist layers".into(),
			italic: true,
			..Default::default()
		}))];

		let ai_artist_section_hostname = vec![
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
				value: preferences.ai_artist_server_hostname.clone(),
				min_width: 200,
				on_update: WidgetCallback::new(|text_input: &TextInput| PreferencesMessage::AiArtistServerHostname { hostname: text_input.value.clone() }.into()),
				..Default::default()
			})),
		];

		let button_widgets = vec![WidgetHolder::new(Widget::TextButton(TextButton {
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
		}))];

		Layout::WidgetLayout(WidgetLayout::new(vec![
			LayoutGroup::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: "Editor Preferences".to_string(),
					bold: true,
					..Default::default()
				}))],
			},
			LayoutGroup::Row { widgets: ai_artist_section_title },
			LayoutGroup::Row { widgets: ai_artist_section_hostname },
			LayoutGroup::Row { widgets: button_widgets },
		]))
	}
}
