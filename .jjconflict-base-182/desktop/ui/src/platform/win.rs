use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::JobObjects::{
	AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation, SetInformationJobObject,
};
use windows::core::PCWSTR;

pub(crate) struct KillOnCloseJob(HANDLE);

// SAFETY: job object handles may be used and closed from any thread.
unsafe impl Send for KillOnCloseJob {}
unsafe impl Sync for KillOnCloseJob {}

impl KillOnCloseJob {
	pub(crate) fn assign(child: &std::process::Child) -> windows::core::Result<Self> {
		use std::os::windows::io::AsRawHandle;
		unsafe {
			let job = CreateJobObjectW(None, PCWSTR::null())?;
			let job = Self(job);
			let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
			info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
			SetInformationJobObject(
				job.0,
				JobObjectExtendedLimitInformation,
				&info as *const _ as *const core::ffi::c_void,
				std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
			)?;
			AssignProcessToJobObject(job.0, HANDLE(child.as_raw_handle()))?;
			Ok(job)
		}
	}
}

impl Drop for KillOnCloseJob {
	fn drop(&mut self) {
		unsafe {
			let _ = CloseHandle(self.0);
		}
	}
}
