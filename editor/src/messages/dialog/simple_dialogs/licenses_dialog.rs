use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

pub struct LicensesDialog {
	pub localized_commit_year: String,
}

impl DialogLayoutHolder for LicensesDialog {
	const ICON: &'static str = "License12px";
	const TITLE: &'static str = "Licenses";

	fn layout_buttons(&self) -> Layout {
		let widgets = vec![TextButton::new("OK").emphasized(true).on_update(|_| FrontendMessage::DisplayDialogDismiss.into()).widget_instance()];

		Layout(vec![LayoutGroup::Row { widgets }])
	}

	fn layout_column_2(&self) -> Layout {
		#[allow(clippy::type_complexity)]
		let button_definitions: &[(&str, &str, fn() -> Message)] = &[
			("Code", "Source Code License", || {
				FrontendMessage::TriggerVisitLink {
					url: "https://graphite.art/license#source-code".into(),
				}
				.into()
			}),
			("GraphiteLogo", "Branding License", || {
				FrontendMessage::TriggerVisitLink {
					url: "https://graphite.art/license#branding".into(),
				}
				.into()
			}),
			("IconsGrid", "Dependency Licenses", || FrontendMessage::TriggerDisplayThirdPartyLicensesDialog.into()),
		];
		let widgets = button_definitions
			.iter()
			.map(|&(icon, label, message_factory)| TextButton::new(label).icon(Some((icon).into())).flush(true).on_update(move |_| message_factory()).widget_instance())
			.collect();

		Layout(vec![LayoutGroup::Column { widgets }])
	}
}

impl LayoutHolder for LicensesDialog {
	fn layout(&self) -> Layout {
		let year = &self.localized_commit_year;
		let description = format!(
			"
			Graphite source code is copyright © {year} Graphite contrib-\nutors and is available under the Apache License 2.0. See\n\"Source Code License\" for details.\n\
			\n\
			The Graphite logo, icons, and visual identity are copyright ©\n{year} Graphite Labs, LLC. See \"Branding License\" for details.\n\
			\n\
			Graphite is distributed with third-party open source code\ndependencies. See \"Dependency Licenses\" for details.
			"
		);
		let description = description.trim();

		Layout(vec![
			LayoutGroup::Row {
				widgets: vec![TextLabel::new("Graphite is free, open source software").bold(true).widget_instance()],
			},
			LayoutGroup::Row {
				widgets: vec![TextLabel::new(description).multiline(true).widget_instance()],
			},
		])
	}
}
