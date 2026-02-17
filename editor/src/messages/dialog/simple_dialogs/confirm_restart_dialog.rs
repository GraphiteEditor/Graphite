use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

/// A dialog for confirming the restart of the application when changing a Preference that requires a restart to take effect.
pub struct ConfirmRestartDialog {}

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
		Layout(vec![LayoutGroup::Row {
			widgets: vec![TextLabel::new("Some changes require restart to take effect").widget_instance()],
		}])
	}
}
