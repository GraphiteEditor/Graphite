use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

/// A dialog to let the user browse a gallery of demo artwork that can be opened.
pub struct DemoArtworkDialog;

impl LayoutHolder for DemoArtworkDialog {
	fn layout(&self) -> Layout {
		let artwork = [
			(
				"Valley of Spires",
				"ThumbnailValleyOfSpires",
				"https://raw.githubusercontent.com/GraphiteEditor/Graphite/master/demo-artwork/valley-of-spires.graphite",
			),
			(
				"Just a Potted Cactus",
				"ThumbnailJustAPottedCactus",
				"https://raw.githubusercontent.com/GraphiteEditor/Graphite/master/demo-artwork/just-a-potted-cactus.graphite",
			),
		];

		let image_widgets = artwork
			.into_iter()
			.map(|(_, thumbnail, _)| ImageLabel::new(thumbnail.to_string()).width(Some("256px".into())).widget_holder())
			.collect();

		let button_widgets = artwork
			.into_iter()
			.map(|(label, _, url)| {
				TextButton::new(label)
					.min_width(256)
					.on_update(|_| {
						DialogMessage::CloseDialogAndThen {
							followups: vec![FrontendMessage::TriggerFetchAndOpenDocument { url: url.to_string() }.into()],
						}
						.into()
					})
					.widget_holder()
			})
			.collect();

		Layout::WidgetLayout(WidgetLayout::new(vec![
			LayoutGroup::Row {
				widgets: vec![TextLabel::new("Demo Artwork".to_string()).bold(true).widget_holder()],
			},
			LayoutGroup::Row { widgets: image_widgets },
			LayoutGroup::Row { widgets: button_widgets },
		]))
	}
}
