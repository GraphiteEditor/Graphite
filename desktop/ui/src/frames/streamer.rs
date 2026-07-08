use ipc_channel::ipc::{IpcSender, IpcSharedMemory};
use std::sync::{Arc, Mutex};

#[cfg(feature = "accelerated_paint")]
use super::plane;
use super::sequence::{FrameSequenceClaim, SequenceState};
use crate::consts::{FRAME_SEGMENT_GRANULARITY, FRAME_SEGMENT_POOL_SIZE};
use crate::remote::messages::EventMessage;

#[derive(Clone)]
pub(crate) struct FrameStreamer(Arc<StreamerInner>);

struct StreamerInner {
	events: Arc<Mutex<IpcSender<EventMessage>>>,
	sequence: Arc<SequenceState>,
	#[cfg(feature = "accelerated_paint")]
	plane: Option<plane::PlaneSender>,
	staged: Mutex<Staged>,
}

#[derive(Default)]
struct Staged {
	segments: Vec<IpcSharedMemory>,
	pending_adverts: Vec<(u32, IpcSharedMemory)>,
	buffer: Option<StagedBuffer>,
	#[cfg(feature = "accelerated_paint")]
	accelerated: Option<(FrameSequenceClaim, plane::StagedFrame)>,
}

struct StagedBuffer {
	claim: FrameSequenceClaim,
	segment: u32,
	width: u32,
	height: u32,
}

impl FrameStreamer {
	pub(crate) fn new(events: Arc<Mutex<IpcSender<EventMessage>>>, sequence: Arc<SequenceState>, #[cfg(feature = "accelerated_paint")] plane: Option<plane::PlaneSender>) -> Self {
		Self(Arc::new(StreamerInner {
			events,
			sequence,
			#[cfg(feature = "accelerated_paint")]
			plane,
			staged: Mutex::new(Staged::default()),
		}))
	}

	pub(crate) fn stage_buffer(&self, buffer: &[u8], width: u32, height: u32) {
		debug_assert_eq!(buffer.len(), width as usize * height as usize * 4);
		if buffer.is_empty() {
			return;
		}

		if buffer.chunks_exact(4).all(|pixel| pixel[3] == 0) {
			tracing::debug!("Skipping fully transparent {width}x{height} frame from CEF");
			return;
		}

		let Some(claim) = self.0.sequence.claim() else {
			return;
		};
		let segment = (claim.seq() % FRAME_SEGMENT_POOL_SIZE) as u32;

		let Ok(mut staged) = self.0.staged.lock() else {
			tracing::error!("Failed to lock the frame staging state");
			return;
		};
		let staged = &mut *staged;
		if staged.segments.len() < FRAME_SEGMENT_POOL_SIZE as usize {
			staged.segments.resize_with(FRAME_SEGMENT_POOL_SIZE as usize, || IpcSharedMemory::from_bytes(&[]));
		}

		let backing = &mut staged.segments[segment as usize];
		if backing.len() < buffer.len() {
			let capacity = buffer.len().next_multiple_of(FRAME_SEGMENT_GRANULARITY);
			*backing = IpcSharedMemory::from_byte(0, capacity);
			staged.pending_adverts.push((segment, backing.clone()));
		}

		unsafe { backing.deref_mut()[..buffer.len()].copy_from_slice(buffer) };

		#[cfg(target_os = "macos")]
		if !staged.pending_adverts.iter().any(|(index, _)| *index == segment) {
			staged.pending_adverts.push((segment, backing.clone()));
		}

		staged.buffer = Some(StagedBuffer { claim, segment, width, height });
	}

	#[cfg(feature = "accelerated_paint")]
	pub(crate) fn stage_texture(&self, info: &cef::AcceleratedPaintInfo) {
		let Some(plane) = &self.0.plane else {
			tracing::error!("Accelerated paint delivered without a frame plane");
			return;
		};

		let Some(claim) = self.0.sequence.claim() else {
			return;
		};
		let Some(frame) = plane.stage(info) else {
			return;
		};
		let Ok(mut staged) = self.0.staged.lock() else {
			tracing::error!("Failed to lock the frame staging state");
			return;
		};
		staged.accelerated = Some((claim, frame));
	}

	pub(crate) fn publish(&self) {
		let Ok(mut staged) = self.0.staged.lock() else {
			tracing::error!("Failed to lock the frame staging state");
			return;
		};
		let adverts = std::mem::take(&mut staged.pending_adverts);
		let software = staged.buffer.take();
		#[cfg(feature = "accelerated_paint")]
		let accelerated = staged.accelerated.take();
		drop(staged);

		#[cfg(feature = "accelerated_paint")]
		if let Some((claim, frame)) = accelerated
			&& let Some(plane) = &self.0.plane
		{
			match plane.send(claim.seq(), frame) {
				Ok(()) => claim.commit(),
				Err(e) => tracing::debug!("Failed to send accelerated frame to main process: {e}"),
			}
		}

		if adverts.is_empty() && software.is_none() {
			return;
		}
		let Ok(sender) = self.0.events.lock() else {
			tracing::error!("Failed to lock host message sender");
			return;
		};
		for (index, shm) in adverts {
			if let Err(e) = sender.send(EventMessage::AdvertiseFrameSegment { index, shm }) {
				tracing::debug!("Failed to send frame segment to main process: {e}");
			}
		}
		if let Some(StagedBuffer { claim, segment, width, height }) = software {
			match sender.send(EventMessage::SoftwareFrame {
				seq: claim.seq(),
				segment,
				width,
				height,
			}) {
				Ok(()) => claim.commit(),
				Err(e) => tracing::debug!("Failed to send frame to main process: {e}"),
			}
		}
	}
}
