use graphite_desktop_wrapper::NodeGraphExecutionResult;
use graphite_desktop_wrapper::messages::DesktopWrapperMessage;

pub(crate) enum AppEvent {
	UiUpdate(wgpu::Texture),
	CursorChange(winit::cursor::Cursor),
	ScheduleBrowserWork(std::time::Instant),
	WebCommunicationInitialized,
	DesktopWrapperMessage(DesktopWrapperMessage),
	NodeGraphExecutionResult(NodeGraphExecutionResult),
	CloseWindow,
}

#[derive(Clone)]
pub(crate) struct AppEventScheduler {
	pub(crate) proxy: winit::event_loop::EventLoopProxy,
	pub(crate) sender: std::sync::mpsc::Sender<AppEvent>,
}

impl AppEventScheduler {
	pub(crate) fn schedule(&self, event: AppEvent) {
		let _ = self.sender.send(event);
		self.proxy.wake_up();
	}
}

pub(crate) trait CreateAppEventSchedulerEventLoopExt {
	fn create_app_event_scheduler(&self, sender: std::sync::mpsc::Sender<AppEvent>) -> AppEventScheduler;
}

impl CreateAppEventSchedulerEventLoopExt for winit::event_loop::EventLoop {
	fn create_app_event_scheduler(&self, sender: std::sync::mpsc::Sender<AppEvent>) -> AppEventScheduler {
		AppEventScheduler { proxy: self.create_proxy(), sender }
	}
}
