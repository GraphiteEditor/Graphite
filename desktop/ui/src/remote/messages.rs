use ipc_channel::ipc::{IpcSender, IpcSharedMemory};
use serde::{Deserialize, Serialize};

use crate::Cursor;
use crate::context::InitError;
use crate::input::InputEvent;
use crate::view::ViewInfoUpdate;

#[derive(Serialize, Deserialize)]
pub(crate) enum HostControlMessage {
	Input(Vec<InputEvent>),
	UpdateViewInfo(ViewInfoUpdate),
	RefreshViewInfo,
	SendWebMessage(Vec<u8>),
	FrameAck { seq: u64 },
	Shutdown,
}

#[derive(Serialize, Deserialize)]
pub(crate) enum EventMessage {
	Hello {
		pid: u32,
		control_sender: IpcSender<HostControlMessage>,
		acceleration: bool,
	},
	BrowserCreated,
	InitFailed(InitError),
	WebCommunicationInitialized,
	WebMessage(Vec<u8>),
	CursorChange(Cursor),
	AdvertiseFrameSegment {
		index: u32,
		shm: IpcSharedMemory,
	},
	SoftwareFrame {
		seq: u64,
		segment: u32,
		width: u32,
		height: u32,
	},
	#[cfg(all(target_os = "windows", feature = "accelerated_paint"))]
	AcceleratedFrame {
		seq: u64,
		handle: u64,
		width: u32,
		height: u32,
		format: u32,
	},
	ShutdownComplete,
}
