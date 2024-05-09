use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

/// A dialog to let the user browse a gallery of demo artwork that can be opened.
pub struct DemoArtworkDialog;

/// `(name, thumbnail, filename)`
pub const ARTWORK: [(&str, &str, &str); 5] = [
	("Valley of Spires", "ThumbnailValleyOfSpires", "valley-of-spires.graphite"),
	("Red Dress", "ThumbnailRedDress", "red-dress.graphite"),
	("Isometric Fountain", "ThumbnailIsometricFountain", "isometric-fountain.graphite"),
	("Procedural String Lights", "ThumbnailProceduralStringLights", "procedural-string-lights.graphite"),
	("Just a Potted Cactus", "ThumbnailJustAPottedCactus", "just-a-potted-cactus.graphite"),
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
		let mut rows_of_images_with_buttons: Vec<_> = ARTWORK
			.chunks(3)
			.into_iter()
			.map(|chunk| {
				let images = chunk
					.into_iter()
					.map(|(_, thumbnail, _)| ImageLabel::new(*thumbnail).width(Some("256px".into())).widget_holder())
					.collect();

				let buttons = chunk
					.into_iter()
					.map(|(name, _, filename)| {
						TextButton::new(*name)
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

				vec![LayoutGroup::Row { widgets: images }, LayoutGroup::Row { widgets: buttons }, LayoutGroup::Row { widgets: vec![] }]
			})
			.flatten()
			.collect();
		let _ = rows_of_images_with_buttons.pop();

		Layout::WidgetLayout(WidgetLayout::new(rows_of_images_with_buttons))
	}
}
