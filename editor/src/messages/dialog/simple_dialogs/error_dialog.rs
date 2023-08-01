use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

/// A dialog to notify users of a non-fatal error.
pub struct ErrorDialog {
	pub title: String,
	pub description: String,
}

impl PropertyHolder for ErrorDialog {
	fn properties(&self) -> Layout {
		Layout::WidgetLayout(WidgetLayout::new(vec![
			LayoutGroup::Row {
				widgets: vec![TextLabel::new(&self.title).bold(true).widget_holder()],
			},
			LayoutGroup::Row {
				widgets: vec![TextLabel::new(&self.description).multiline(true).widget_holder()],
			},
			LayoutGroup::Row {
				widgets: vec![TextButton::new("OK")
					.emphasized(true)
					.min_width(96)
					.on_update(|_| FrontendMessage::DisplayDialogDismiss.into())
					.widget_holder()],
			},
		]))
	}
}
