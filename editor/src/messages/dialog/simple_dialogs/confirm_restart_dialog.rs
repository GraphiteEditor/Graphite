use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

/// A dialog for confirming the restart of the application when changing a preference that requires a restart to take effect.
pub struct ConfirmRestartDialog {
	pub changed_settings: Vec<String>,
}

impl DialogLayoutHolder for ConfirmRestartDialog {
	const ICON: &'static str = "Warning";
	const TITLE: &'static str = "Restart Required";

	fn layout_buttons(&self) -> Layout {
		let widgets = vec![
			TextButton::new("Restart Now")
				.emphasized(true)
				.on_update(|_| {
					DialogMessage::CloseAndThen {
						followups: vec![AppWindowMessage::Restart.into()],
					}
					.into()
				})
				.widget_instance(),
			TextButton::new("Later").on_update(|_| FrontendMessage::DialogClose.into()).widget_instance(),
		];

		Layout(vec![LayoutGroup::Row { widgets }])
	}
}

impl LayoutHolder for ConfirmRestartDialog {
	fn layout(&self) -> Layout {
		let changed_settings = "• ".to_string() + &self.changed_settings.join("\n• ");

		Layout(vec![
			LayoutGroup::Row {
				widgets: vec![TextLabel::new("Restart to apply changes?").bold(true).multiline(true).widget_instance()],
			},
			LayoutGroup::Row {
				widgets: vec![
					TextLabel::new(
						format!(
							"
							Settings that only take effect on next launch:\n\
							{changed_settings}\n\
							\n\
							This only takes a few seconds. Open documents,\n\
							even unsaved ones, will be automatically restored.
							"
						)
						.trim(),
					)
					.multiline(true)
					.widget_instance(),
				],
			},
		])
	}
}
