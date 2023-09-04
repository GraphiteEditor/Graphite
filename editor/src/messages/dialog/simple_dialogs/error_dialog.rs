use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

/// A dialog to notify users of a non-fatal error.
pub struct ErrorDialog {
	pub title: String,
	pub description: String,
}

impl DialogLayoutHolder for ErrorDialog {
	const ICON: &'static str = "Warning";
	const TITLE: &'static str = "Error";

	fn layout_buttons(&self) -> Layout {
		let widgets = vec![TextButton::new("OK").emphasized(true).on_update(|_| FrontendMessage::DisplayDialogDismiss.into()).widget_holder()];

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

impl LayoutHolder for ErrorDialog {
	fn layout(&self) -> Layout {
		Layout::WidgetLayout(WidgetLayout::new(vec![
			LayoutGroup::Row {
				widgets: vec![TextLabel::new(&self.title).bold(true).widget_holder()],
			},
			LayoutGroup::Row {
				widgets: vec![TextLabel::new(&self.description).multiline(true).widget_holder()],
			},
		]))
	}
}
