use ipc_channel::ipc::IpcSender;
use std::sync::{Arc, Mutex};

use windows::Win32::Foundation::{CloseHandle, DUPLICATE_CLOSE_SOURCE, DUPLICATE_SAME_ACCESS, DuplicateHandle, HANDLE};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcess, PROCESS_DUP_HANDLE};

use crate::frames::import::ContentRect;
use crate::frames::surface::FrameSurface;
use crate::remote::HostConfig;
use crate::remote::messages::EventMessage;

struct MainProcess(HANDLE);

// SAFETY: process handles may be used and closed from any thread.
unsafe impl Send for MainProcess {}
unsafe impl Sync for MainProcess {}

impl MainProcess {
	fn open(pid: u32) -> windows::core::Result<Self> {
		// SAFETY: plain OpenProcess call; on success the handle is ours to close.
		unsafe { OpenProcess(PROCESS_DUP_HANDLE, false, pid).map(Self) }
	}

	fn duplicate_into(&self, handle: HANDLE) -> windows::core::Result<u64> {
		let mut target = HANDLE::default();
		// SAFETY: both process handles are valid; `target` receives the duplicate.
		unsafe { DuplicateHandle(GetCurrentProcess(), handle, self.0, &mut target, 0, false, DUPLICATE_SAME_ACCESS)? };
		Ok(target.0 as u64)
	}

	fn close_in_main(&self, handle: u64) {
		let mut reclaimed = HANDLE::default();
		// SAFETY: `handle` came from `duplicate_into` and is valid.
		unsafe {
			if let Err(e) = DuplicateHandle(self.0, HANDLE(handle as _), GetCurrentProcess(), &mut reclaimed, 0, false, DUPLICATE_CLOSE_SOURCE) {
				tracing::warn!("Failed to reclaim a frame handle from the main process: {e}");
			}
			if !reclaimed.is_invalid() {
				let _ = CloseHandle(reclaimed);
			}
		}
	}
}

impl Drop for MainProcess {
	fn drop(&mut self) {
		// SAFETY: we own the process handle.
		unsafe {
			let _ = CloseHandle(self.0);
		}
	}
}

pub(crate) struct PlaneSender {
	main: Arc<MainProcess>,
	events: Arc<Mutex<IpcSender<EventMessage>>>,
}

impl PlaneSender {
	pub(crate) fn from_config(config: &HostConfig, events: Arc<Mutex<IpcSender<EventMessage>>>) -> Option<Self> {
		match MainProcess::open(config.main_pid) {
			Ok(main) => Some(Self { main: Arc::new(main), events }),
			Err(e) => {
				tracing::error!("Failed to open the main process for handle duplication, falling back to software frames: {e}");
				None
			}
		}
	}

	pub(crate) fn stage(&self, info: &cef::AcceleratedPaintInfo) -> Option<StagedFrame> {
		let coded_size = &info.extra.coded_size;
		if coded_size.width <= 0 || coded_size.height <= 0 {
			tracing::error!("Accelerated paint delivered an invalid coded size: {}x{}", coded_size.width, coded_size.height);
			return None;
		}

		let handle = match self.main.duplicate_into(HANDLE(info.shared_texture_handle)) {
			Ok(handle) => handle,
			Err(e) => {
				tracing::error!("Failed to duplicate the shared texture handle into the main process: {e}");
				return None;
			}
		};

		Some(StagedFrame {
			handle: HandleInMain { handle, main: self.main.clone() },
			width: coded_size.width as u32,
			height: coded_size.height as u32,
			format: *info.format.as_ref() as u32,
			content: ContentRect::try_from(info).ok(),
		})
	}

	pub(crate) fn send(&self, seq: u64, frame: StagedFrame) -> std::io::Result<()> {
		let message = EventMessage::AcceleratedFrame {
			seq,
			handle: frame.handle.handle,
			width: frame.width,
			height: frame.height,
			format: frame.format,
			content: frame.content,
		};
		let sender = self.events.lock().map_err(|_| std::io::Error::other("the host message sender lock is poisoned"))?;
		match sender.send(message) {
			Ok(()) => {
				// Dropping the handle would reclaim it. We must not drop.
				std::mem::forget(frame.handle);
				Ok(())
			}
			Err(e) => Err(std::io::Error::other(e.to_string())),
		}
	}
}

pub(crate) struct StagedFrame {
	handle: HandleInMain,
	width: u32,
	height: u32,
	format: u32,
	content: Option<ContentRect>,
}

struct HandleInMain {
	handle: u64,
	main: Arc<MainProcess>,
}

impl Drop for HandleInMain {
	fn drop(&mut self) {
		self.main.close_in_main(self.handle);
	}
}

pub(crate) struct WireFrame {
	seq: u64,
	handle: ReceivedHandle,
	width: u32,
	height: u32,
	format: u32,
	content: Option<ContentRect>,
}

impl WireFrame {
	pub(crate) fn new(seq: u64, handle: u64, width: u32, height: u32, format: u32, content: Option<ContentRect>) -> Self {
		Self {
			seq,
			handle: ReceivedHandle(handle),
			width,
			height,
			format,
			content,
		}
	}

	pub(crate) fn seq(&self) -> u64 {
		self.seq
	}

	pub(crate) fn import(self, surface: &FrameSurface) -> Option<wgpu::Texture> {
		let format = super::wire_color_type(self.format)?;
		let content = self.content.unwrap_or_default();
		surface.import_texture(crate::frames::import::d3d11::D3D11Importer::from_parts(self.handle.0, self.width, self.height, format), content)
	}
}

struct ReceivedHandle(u64);

impl Drop for ReceivedHandle {
	fn drop(&mut self) {
		// SAFETY: the host duplicated this handle into our process for us to own.
		if let Err(e) = unsafe { CloseHandle(HANDLE(self.0 as _)) } {
			tracing::warn!("Failed to close a remote frame handle: {e}");
		}
	}
}
