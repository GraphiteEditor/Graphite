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
		let license_text = if let Some(first_underscore_line) = self.license_text.lines().position(|line| line.contains('_')) {
			// Find the byte position where the line with underscore starts
			let char_position = self.license_text.split('\n').take(first_underscore_line).map(|line| line.len() + '\n'.len_utf8()).sum();
			self.license_text[char_position..].to_string()
		} else {
			// This shouldn't be encountered, but if no underscore line is found, we use the full text as a safety fallback
			self.license_text.clone()
		};

		// Two characters (one before, one after) the sequence of underscore characters, plus one additional column to provide a space between the text and the scrollbar
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
