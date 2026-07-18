use crate::consts::{PLOTTER_PAPER_SIZE_INCHES, PLOTTER_PEN_LIFT_SECONDS, PLOTTER_PEN_SPEED_INCHES_PER_SECOND, PLOTTER_SETUP_SECONDS};
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;
use graphene_std::renderer::plot_statistics::PlotStatistics;

#[derive(ExtractField)]
pub struct SendToPlotterDialogMessageContext<'a> {
	pub portfolio: &'a PortfolioMessageHandler,
}

/// How long the plotter is expected to take on the current document, shown in the dialog.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum PlotTimeEstimate {
	/// The estimate render is still in flight.
	#[default]
	Pending,
	/// The estimate could not be computed.
	Unavailable,
	/// Estimated plot duration in seconds.
	Seconds(f64),
}

/// A dialog to send the current document to the pen plotter print server as an SVG.
#[derive(Debug, Clone, Default, ExtractField)]
pub struct SendToPlotterDialogMessageHandler {
	pub job_name: String,
	pub estimate: PlotTimeEstimate,
}

#[message_handler_data]
impl MessageHandler<SendToPlotterDialogMessage, SendToPlotterDialogMessageContext<'_>> for SendToPlotterDialogMessageHandler {
	fn process_message(&mut self, message: SendToPlotterDialogMessage, responses: &mut VecDeque<Message>, context: SendToPlotterDialogMessageContext) {
		let SendToPlotterDialogMessageContext { portfolio } = context;

		match message {
			SendToPlotterDialogMessage::JobName { name } => self.job_name = name,

			SendToPlotterDialogMessage::UpdateTimeEstimate { seconds } => {
				self.estimate = match seconds {
					Some(seconds) => PlotTimeEstimate::Seconds(seconds),
					None => PlotTimeEstimate::Unavailable,
				};

				// Refresh only the dialog content, since re-displaying the whole dialog would reopen it if the user has already closed it
				self.send_layout(responses, LayoutTarget::DialogColumn1);
				return;
			}

			SendToPlotterDialogMessage::Submit => {
				// Fall back to the document name so the attendant can still tell jobs apart if the field was cleared
				let job_name = if self.job_name.trim().is_empty() {
					portfolio.active_document().map(|document| document.name.clone()).unwrap_or_default()
				} else {
					self.job_name.clone()
				};

				responses.add_front(PortfolioMessage::SubmitPlotterExport { job_name, estimate_only: false });
			}
		}

		self.send_dialog_to_frontend(responses);
	}

	advertise_actions!(SendToPlotterDialogUpdate;
	);
}

impl DialogLayoutHolder for SendToPlotterDialogMessageHandler {
	const ICON: &'static str = "File";
	const TITLE: &'static str = "Send to Plotter";

	fn layout_buttons(&self) -> Layout {
		let widgets = vec![
			TextButton::new("Send")
				.emphasized(true)
				.on_update(|_| {
					DialogMessage::CloseAndThen {
						followups: vec![SendToPlotterDialogMessage::Submit.into()],
					}
					.into()
				})
				.widget_instance(),
			TextButton::new("Cancel").on_update(|_| FrontendMessage::DialogClose.into()).widget_instance(),
		];

		Layout(vec![LayoutGroup::row(widgets)])
	}
}

impl LayoutHolder for SendToPlotterDialogMessageHandler {
	fn layout(&self) -> Layout {
		let job_name = vec![
			TextLabel::new("Job Name").table_align(true).min_width(100).widget_instance(),
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			TextInput::new(&self.job_name)
				.on_update(|text_input: &TextInput| SendToPlotterDialogMessage::JobName { name: text_input.value.clone() }.into())
				.min_width(200)
				.widget_instance(),
		];

		let estimate_text = match self.estimate {
			PlotTimeEstimate::Pending => "Estimating…".to_string(),
			PlotTimeEstimate::Unavailable => "Unknown".to_string(),
			PlotTimeEstimate::Seconds(seconds) => format_plot_duration(seconds),
		};
		let estimated_time = vec![
			TextLabel::new("Estimated Time").table_align(true).min_width(100).widget_instance(),
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			TextLabel::new(estimate_text).widget_instance(),
		];

		Layout(vec![LayoutGroup::row(job_name), LayoutGroup::row(estimated_time)])
	}
}

/// Estimates how long the plotter will take to draw the given SVG, in seconds.
///
/// The artwork's bounding box is scaled to fit the paper (rotated to landscape when wider than tall), then timed as a
/// fixed setup cost, pen-down drawing at a constant speed, and a fixed cost per pen lift (which also covers pen-up
/// travel, since the print server reorders paths to minimize it). The pen draws every path as its outline, so total
/// path length approximates the pen-down distance regardless of fills. Intricate fine detail like text tends to run
/// over this estimate because the machine cannot reach full speed on tiny curves.
pub fn estimated_plot_seconds(statistics: &PlotStatistics) -> f64 {
	let (paper_width, paper_height) = if statistics.width > statistics.height {
		(PLOTTER_PAPER_SIZE_INCHES.1, PLOTTER_PAPER_SIZE_INCHES.0)
	} else {
		PLOTTER_PAPER_SIZE_INCHES
	};
	let scale = if statistics.width > 0. && statistics.height > 0. {
		(paper_width / statistics.width).min(paper_height / statistics.height)
	} else {
		0.
	};

	let pen_down_inches = statistics.pen_down_distance * scale;

	PLOTTER_SETUP_SECONDS + pen_down_inches / PLOTTER_PEN_SPEED_INCHES_PER_SECOND + statistics.pen_lift_count as f64 * PLOTTER_PEN_LIFT_SECONDS
}

/// Formats an estimated duration like "About 3 min 20 sec", rounded to the nearest 5 seconds.
fn format_plot_duration(seconds: f64) -> String {
	let total = (((seconds / 5.).round() * 5.).max(5.)) as u64;
	let minutes = total / 60;
	let seconds = total % 60;

	if minutes == 0 {
		format!("About {seconds} sec")
	} else if seconds == 0 {
		format!("About {minutes} min")
	} else {
		format!("About {minutes} min {seconds} sec")
	}
}
