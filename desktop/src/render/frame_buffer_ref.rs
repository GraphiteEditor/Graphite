use thiserror::Error;

pub(crate) struct FrameBufferRef<'a> {
	buffer: &'a [u8],
	width: usize,
	height: usize,
}
impl<'a> FrameBufferRef<'a> {
	pub(crate) fn new(buffer: &'a [u8], width: usize, height: usize) -> Result<Self, FrameBufferError> {
		let fb = Self { buffer, width, height };
		fb.validate_size()?;
		Ok(fb)
	}
	pub(crate) fn buffer(&self) -> &[u8] {
		self.buffer
	}

	pub(crate) fn width(&self) -> usize {
		self.width
	}

	pub(crate) fn height(&self) -> usize {
		self.height
	}

	fn validate_size(&self) -> Result<(), FrameBufferError> {
		if self.buffer.len() != self.width * self.height * 4 {
			Err(FrameBufferError::InvalidSize {
				buffer_size: self.buffer.len(),
				expected_size: self.width * self.height * 4,
				width: self.width,
				height: self.height,
			})
		} else {
			Ok(())
		}
	}
}
impl<'a> std::fmt::Debug for FrameBufferRef<'a> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("FrameBuffer")
			.field("width", &self.width)
			.field("height", &self.height)
			.field("len", &self.buffer.len())
			.finish()
	}
}

#[derive(Error, Debug)]
pub(crate) enum FrameBufferError {
	#[error("Invalid buffer size {buffer_size}, expected {expected_size} for width {width} multiplied with height {height} multiplied by 4 channels")]
	InvalidSize { buffer_size: usize, expected_size: usize, width: usize, height: usize },
}
