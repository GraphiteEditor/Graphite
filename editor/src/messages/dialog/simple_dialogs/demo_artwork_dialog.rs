use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

/// A dialog to let the user browse a gallery of demo artwork that can be opened.
pub struct DemoArtworkDialog;

/// `(name, thumbnail, filename)`
pub const ARTWORK: [(&str, &str, &str); 4] = [
	("Valley of Spires", "ThumbnailValleyOfSpires", "valley-of-spires.graphite"),
	("Red Dress", "ThumbnailRedDress", "red-dress.graphite"),
	("Just a Potted Cactus", "ThumbnailJustAPottedCactus", "just-a-potted-cactus.graphite"),
	("Procedural String Lights", "ThumbnailProceduralStringLights", "procedural-string-lights.graphite"),
];

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
			.map(|(_, thumbnail, _)| ImageLabel::new(thumbnail.to_string()).width(Some("256px".into())).widget_holder())
			.collect();

		let buttons = ARTWORK
			.into_iter()
			.map(|(name, _, filename)| {
				TextButton::new(name)
					.min_width(256)
					.on_update(|_| {
						DialogMessage::CloseDialogAndThen {
							followups: vec![FrontendMessage::TriggerFetchAndOpenDocument {
								name: name.to_string(),
								filename: filename.to_string(),
							}
							.into()],
						}
						.into()
					})
					.widget_holder()
			})
			.collect();

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets: images }, LayoutGroup::Row { widgets: buttons }]))
	}
}
