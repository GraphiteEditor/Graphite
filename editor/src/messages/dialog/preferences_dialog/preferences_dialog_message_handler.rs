use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, PropertyHolder, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::layout::utility_types::widgets::button_widgets::TextButton;
use crate::messages::layout::utility_types::widgets::input_widgets::TextInput;
use crate::messages::layout::utility_types::widgets::label_widgets::{Separator, SeparatorDirection, SeparatorType, TextLabel};
use crate::messages::prelude::*;

/// A dialog to allow users to customize Graphite editor options
#[derive(Debug, Clone, Default)]
pub struct PreferencesDialogMessageHandler {
	pub ai_artist_hostname: String,
}

impl MessageHandler<PreferencesDialogMessage, ()> for PreferencesDialogMessageHandler {
	fn process_message(&mut self, message: PreferencesDialogMessage, _data: (), responses: &mut VecDeque<Message>) {
		match message {
			PreferencesDialogMessage::AiArtistHostname(hostname) => self.ai_artist_hostname = hostname,

			PreferencesDialogMessage::Confirm => responses.push_front(
				PortfolioMessage::AiArtistSetServerHostname {
					hostname: self.ai_artist_hostname.clone(),
				}
				.into(),
			),
		}

		self.register_properties(responses, LayoutTarget::DialogDetails);
	}

	advertise_actions! {PreferencesDialogUpdate;}
}

impl PropertyHolder for PreferencesDialogMessageHandler {
	fn properties(&self) -> Layout {
		let ai_artist_hostname = vec![
			WidgetHolder::new(Widget::TextLabel(TextLabel {
				value: "AI Artist Hostname".into(),
				table_align: true,
				..Default::default()
			})),
			WidgetHolder::new(Widget::Separator(Separator {
				separator_type: SeparatorType::Unrelated,
				direction: SeparatorDirection::Horizontal,
			})),
			WidgetHolder::new(Widget::TextInput(TextInput {
				value: self.ai_artist_hostname.clone(),
				min_width: 200,
				on_update: WidgetCallback::new(|text_input: &TextInput| PreferencesDialogMessage::AiArtistHostname(text_input.value.clone()).into()),
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
					value: "Graphite Preferences".to_string(),
					bold: true,
					..Default::default()
				}))],
			},
			LayoutGroup::Row { widgets: ai_artist_hostname },
			LayoutGroup::Row { widgets: button_widgets },
		]))
	}
}
