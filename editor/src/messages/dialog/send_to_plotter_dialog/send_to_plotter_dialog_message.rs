use crate::messages::prelude::*;

#[impl_message(Message, DialogMessage, SendToPlotterDialog)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum SendToPlotterDialogMessage {
	JobName { name: String },
	UpdateTimeEstimate { seconds: Option<f64> },

	Submit,
}
