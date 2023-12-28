use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

pub struct LicensesDialog {
	pub localized_commit_year: String,
}

impl DialogLayoutHolder for LicensesDialog {
	const ICON: &'static str = "License12px";
	const TITLE: &'static str = "Licenses";

	fn layout_buttons(&self) -> Layout {
		let widgets = vec![TextButton::new("OK").emphasized(true).on_update(|_| FrontendMessage::DisplayDialogDismiss.into()).widget_holder()];

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}

	fn layout_column_2(&self) -> Layout {
		let icons_license_link = "https://raw.githubusercontent.com/GraphiteEditor/Graphite/master/frontend/assets/LICENSE.md";
		let links = [
			("GraphiteLogo", "Graphite Logo", "https://graphite.rs/logo/"),
			("IconsGrid", "Graphite Icons", icons_license_link),
			("License", "Graphite License", "https://graphite.rs/license/"),
			("License", "Other Licenses", "/third-party-licenses.txt"),
		];
		let widgets = links
			.into_iter()
			.map(|(icon, label, url)| {
				TextButton::new(label)
					.icon(Some(icon.into()))
					.flush(true)
					.on_update(|_| FrontendMessage::TriggerVisitLink { url: url.into() }.into())
					.widget_holder()
			})
			.collect();

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Column { widgets }]))
	}
}

impl LayoutHolder for LicensesDialog {
	fn layout(&self) -> Layout {
		let description = concat!(
			"The Graphite logo and brand identity are copyright © [YEAR]\nGraphite Labs, LLC. See \"Graphite Logo\" for usage policy.",
			"\n\n",
			"The Graphite editor's icons and design assets are copyright\n© [YEAR] Graphite Labs, LLC. See \"Graphite Icons\" for details.",
			"\n\n",
			"Graphite code is copyright © [YEAR] Graphite contributors\nand is made available under the Apache 2.0 license. See\n\"Graphite License\" for details.",
			"\n\n",
			"Graphite is distributed with third-party open source code\ndependencies. See \"Other Licenses\" for details.",
		)
		.replace("[YEAR]", &self.localized_commit_year);

		Layout::WidgetLayout(WidgetLayout::new(vec![
			LayoutGroup::Row {
				widgets: vec![TextLabel::new("Graphite is free, open source software").bold(true).widget_holder()],
			},
			LayoutGroup::Row {
				widgets: vec![TextLabel::new(description).multiline(true).widget_holder()],
			},
		]))
	}
}
