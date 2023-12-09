use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

/// A dialog to let the user browse a gallery of demo artwork that can be opened.
pub struct DemoArtworkDialog;

const ARTWORK: [(&str, &str); 2] = [("Valley of Spires", "ThumbnailValleyOfSpires"), ("Just a Potted Cactus", "ThumbnailJustAPottedCactus")];

impl DialogLayoutHolder for DemoArtworkDialog {
	const ICON: &'static str = "Image";
	const TITLE: &'static str = "Demo Artwork";

	fn layout_buttons(&self) -> Layout {
		let widgets = vec![TextButton::new("Close").emphasized(true).on_update(|_| FrontendMessage::DisplayDialogDismiss.into()).widget_holder()];

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

impl LayoutHolder for DemoArtworkDialog {
	fn layout(&self) -> Layout {
		let images = ARTWORK
			.into_iter()
			.map(|(_, thumbnail)| ImageLabel::new(thumbnail.to_string()).width(Some("256px".into())).widget_holder())
			.collect();

		let buttons = ARTWORK
			.into_iter()
			.map(|(label, _)| {
				TextButton::new(label)
					.min_width(256)
					.on_update(|_| {
						DialogMessage::CloseDialogAndThen {
							followups: vec![FrontendMessage::TriggerOpenDemoArtwork { name: label.to_string() }.into()],
						}
						.into()
					})
					.widget_holder()
			})
			.collect();

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets: images }, LayoutGroup::Row { widgets: buttons }]))
	}
}
