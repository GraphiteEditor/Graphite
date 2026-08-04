use ipc_channel::ipc::{IpcSender, IpcSharedMemory};
use std::sync::Arc;

use super::FrameSurface;
#[cfg(feature = "accelerated_paint")]
use super::plane;
use super::sink::FrameSink;
use crate::UiEvent;
use crate::events::EventQueue;
use crate::remote::messages::HostControlMessage;

pub(crate) enum PendingFrame {
	Software {
		seq: u64,
		segment: u32,
		width: u32,
		height: u32,
	},
	#[cfg(all(target_os = "windows", feature = "accelerated_paint"))]
	Accelerated(plane::WireFrame),
}

impl PendingFrame {
	pub(crate) fn seq(&self) -> u64 {
		match self {
			PendingFrame::Software { seq, .. } => *seq,
			#[cfg(all(target_os = "windows", feature = "accelerated_paint"))]
			PendingFrame::Accelerated(frame) => frame.seq(),
		}
	}
}

pub(crate) struct SegmentTable(Vec<Option<IpcSharedMemory>>);

impl SegmentTable {
	pub(crate) fn new() -> Self {
		Self(Vec::new())
	}

	pub(crate) fn advertise(&mut self, index: u32, shm: IpcSharedMemory) {
		let index = index as usize;
		if self.0.len() <= index {
			self.0.resize_with(index + 1, || None);
		}
		self.0[index] = Some(shm);
	}

	fn frame(&self, seq: u64, segment: u32, width: u32, height: u32) -> Option<&[u8]> {
		let frame_bytes = width as usize * height as usize * 4;
		match self.0.get(segment as usize).and_then(Option::as_ref) {
			Some(shm) if shm.len() >= frame_bytes => Some(&shm[..frame_bytes]),
			Some(shm) => {
				tracing::error!("Frame {seq} needs {frame_bytes} bytes but segment {segment} holds {}", shm.len());
				None
			}
			None => {
				tracing::error!("Frame {seq} references unadvertised segment {segment}");
				None
			}
		}
	}
}

#[derive(Clone)]
pub(crate) struct FrameConsumer {
	surface: FrameSurface,
	events: EventQueue,
	sender: IpcSender<HostControlMessage>,
	sink: Arc<FrameSink>,
}

impl FrameConsumer {
	pub(crate) fn new(surface: FrameSurface, events: EventQueue, sender: IpcSender<HostControlMessage>) -> Self {
		Self {
			surface,
			events,
			sender,
			sink: Arc::new(FrameSink::new()),
		}
	}

	fn deliver(&self, seq: u64, install: impl FnOnce(&FrameSurface) -> Option<wgpu::Texture>) {
		self.sink.deliver(&self.sender, seq, || match install(&self.surface) {
			Some(texture) => {
				self.events.send(UiEvent::Frame(texture));
				true
			}
			None => false,
		});
	}

	pub(crate) fn deliver_pending(&self, frame: PendingFrame, segments: &SegmentTable) {
		match frame {
			PendingFrame::Software { seq, segment, width, height } => {
				self.deliver(seq, |surface| {
					segments.frame(seq, segment, width, height).and_then(|pixels| surface.upload_buffer(pixels, width, height))
				});
			}
			#[cfg(all(target_os = "windows", feature = "accelerated_paint"))]
			PendingFrame::Accelerated(frame) => self.deliver_accelerated(frame),
		}
	}

	#[cfg(feature = "accelerated_paint")]
	pub(crate) fn deliver_accelerated(&self, frame: plane::WireFrame) {
		let seq = frame.seq();
		self.deliver(seq, |surface| frame.import(surface));
	}
}

#[cfg(all(any(target_os = "linux", target_os = "macos"), feature = "accelerated_paint"))]
pub(crate) fn plane_receiver_loop(receiver: plane::PlaneReceiver, consumer: FrameConsumer) {
	loop {
		let mut frame = loop {
			match receiver.recv_blocking() {
				Ok(plane::RecvResult::Frame(frame)) => break frame,
				Ok(plane::RecvResult::WouldBlock) => continue,
				Ok(plane::RecvResult::Closed) => return,
				Err(e) => {
					tracing::error!("Accelerated frame plane failed: {e}");
					return;
				}
			}
		};
		// Drain any newer frames that have arrived since the blocking receive
		while let Ok(plane::RecvResult::Frame(newer)) = receiver.try_recv() {
			frame = newer;
		}
		consumer.deliver_accelerated(frame);
	}
}
