use std::sync::mpsc::Receiver;

#[derive(Clone, Copy)]
pub(crate) struct ViewInfo {
	width: u32,
	height: u32,
	scale: f64,
}

impl ViewInfo {
	pub(crate) fn new() -> Self {
		Self { width: 1, height: 1, scale: 1. }
	}

	pub(crate) fn apply_update(&mut self, update: ViewInfoUpdate) {
		match update {
			ViewInfoUpdate::Size { width, height } if width > 0 && height > 0 => {
				self.width = width;
				self.height = height;
			}
			ViewInfoUpdate::Scale(scale) if scale > 0. => {
				self.scale = scale;
			}
			_ => {}
		}
	}

	pub(crate) fn zoom(&self) -> f64 {
		self.scale.ln() / 1.2_f64.ln()
	}

	pub(crate) fn width(&self) -> u32 {
		self.width
	}

	pub(crate) fn height(&self) -> u32 {
		self.height
	}
}

impl Default for ViewInfo {
	fn default() -> Self {
		Self::new()
	}
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) enum ViewInfoUpdate {
	Size { width: u32, height: u32 },
	Scale(f64),
}

pub(super) struct ViewInfoReceiver {
	view_info: ViewInfo,
	receiver: Receiver<ViewInfoUpdate>,
}

impl ViewInfoReceiver {
	pub(super) fn new(receiver: Receiver<ViewInfoUpdate>) -> Self {
		Self { view_info: ViewInfo::new(), receiver }
	}

	/// Apply all pending updates and return the resulting view info.
	pub(super) fn current(&mut self) -> ViewInfo {
		for update in self.receiver.try_iter() {
			self.view_info.apply_update(update);
		}
		self.view_info
	}
}
