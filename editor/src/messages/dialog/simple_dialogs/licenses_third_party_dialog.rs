use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

pub struct LicensesThirdPartyDialog {
	pub license_text: String,
}

impl DialogLayoutHolder for LicensesThirdPartyDialog {
	const ICON: &'static str = "License12px";
	const TITLE: &'static str = "Third-Party Software License Notices";

	fn layout_buttons(&self) -> Layout {
		let widgets = vec![TextButton::new("OK").emphasized(true).on_update(|_| FrontendMessage::DisplayDialogDismiss.into()).widget_holder()];

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

impl LayoutHolder for LicensesThirdPartyDialog {
	fn layout(&self) -> Layout {
		// Remove the header and begin with the line containing the first license section (we otherwise keep the title for standalone viewing of the licenses text file)
		let mut license_text = self.license_text.clone();
		for _ in 0..license_text.lines().position(|line| line.contains('_')).unwrap_or(0) {
			if let Some(end_of_line_index) = license_text.find('\n') {
				license_text.drain(..=end_of_line_index);
			} else {
				// No more newlines, fall back to the original text
				license_text = self.license_text.clone();
				break;
			}
		}

		// Two columns (one before, one after) the "_" characters, and one additional column to provide a space between the text and the scrollbar
		let non_wrapping_column_width = license_text.split('\n').map(|line| line.chars().filter(|&c| c == '_').count()).max().unwrap_or(0) + 2 + 1;

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row {
			widgets: vec![
				TextLabel::new(license_text)
					.monospace(true)
					.multiline(true)
					.min_width(format!("{non_wrapping_column_width}ch"))
					.widget_holder(),
			],
		}]))
	}
}
