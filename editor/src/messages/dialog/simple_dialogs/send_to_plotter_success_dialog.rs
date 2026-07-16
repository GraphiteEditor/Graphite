use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

/// A dialog to confirm that a job was successfully queued on the pen plotter print server.
pub struct SendToPlotterSuccessDialog {
	pub job_name: String,
}

impl DialogLayoutHolder for SendToPlotterSuccessDialog {
	const ICON: &'static str = "CheckboxChecked";
	const TITLE: &'static str = "Send to Plotter";

	fn layout_buttons(&self) -> Layout {
		let widgets = vec![TextButton::new("OK").emphasized(true).on_update(|_| FrontendMessage::DialogClose.into()).widget_instance()];

		Layout(vec![LayoutGroup::row(widgets)])
	}
}

impl LayoutHolder for SendToPlotterSuccessDialog {
	fn layout(&self) -> Layout {
		Layout(vec![
			LayoutGroup::row(vec![TextLabel::new("Sent to the plotter queue").bold(true).widget_instance()]),
			LayoutGroup::row(vec![
				TextLabel::new(format!(
					"The job \"{}\" was added to the queue.\nA booth attendant will start the plot from the dashboard.",
					self.job_name
				))
				.multiline(true)
				.widget_instance(),
			]),
		])
	}
}
