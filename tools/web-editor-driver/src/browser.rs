use base64::Engine;
use glam::{DVec2, UVec2};
use graphite_editor_control_protocol::{MouseButton, Rectangle};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::time::{Duration, Instant};

use crate::paths;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const CLOSE_TIMEOUT: Duration = Duration::from_secs(10);
const EXIT_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// The steps the Playwright process can carry out, matching the `Request` type in `playwright/browser.ts`.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
enum Request<'a> {
	Launch {
		url: &'a str,
		size: UVec2,
		scale: f64,
		headed: bool,
		ready_selector: &'a str,
		timeout: u64,
	},
	Close,
	Resize {
		size: UVec2,
	},
	MouseMove {
		to: DVec2,
	},
	MouseDown {
		button: MouseButton,
		click_count: u32,
	},
	MouseUp {
		button: MouseButton,
		click_count: u32,
	},
	Wheel {
		delta: DVec2,
	},
	KeyDown {
		key: &'a str,
	},
	KeyUp {
		key: &'a str,
	},
	Type {
		text: &'a str,
	},
	Screenshot {
		#[serde(skip_serializing_if = "Option::is_none")]
		clip: Option<Rectangle>,
	},
	BoundingBox {
		selector: &'a str,
	},
	Locate {
		#[serde(skip_serializing_if = "Option::is_none")]
		selector: Option<&'a str>,
		#[serde(skip_serializing_if = "Option::is_none")]
		text: Option<&'a str>,
		field_selector: &'a str,
	},
	NextFrame {
		timeout: u64,
	},
	TakeEvents,
}

#[derive(Serialize)]
struct Envelope<'a> {
	id: u64,
	request: Request<'a>,
}

#[derive(Deserialize)]
struct Reply {
	id: u64,
	#[serde(default)]
	value: serde_json::Value,
	error: Option<String>,
}

#[derive(Deserialize)]
pub struct RawLocated {
	#[serde(rename = "box")]
	pub bounds: Rectangle,
	pub text: String,
	pub value: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageEvents {
	pub console_errors: Vec<String>,
	pub crashed: bool,
}

type ReplySender = mpsc::Sender<Result<serde_json::Value, String>>;

fn exited_error() -> String {
	"The Playwright process has exited, and the session log may say why".to_string()
}

/// The browser being driven, reached through the Node process that holds Playwright.
pub struct Browser {
	process: Mutex<Child>,
	stdin: Mutex<ChildStdin>,
	// Becomes `None` once the process has exited, so no request can wait on a reply that will never come
	pending: Arc<Mutex<Option<HashMap<u64, ReplySender>>>>,
	next_id: AtomicU64,
}

impl Browser {
	/// Starts the Playwright process, which shares this process's error output.
	pub fn start() -> Result<Self, String> {
		let script = paths::playwright_directory().join("browser.ts");
		if !paths::playwright_directory().join("node_modules").join("playwright-core").exists() {
			return Err("Playwright is not installed, so run `cargo run drive install` first".to_string());
		}

		let mut process = Command::new("node")
			.arg(script)
			.env(paths::BROWSERS_PATH_VARIABLE, paths::browsers_directory())
			.current_dir(paths::playwright_directory())
			.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.stderr(Stdio::inherit())
			.spawn()
			.map_err(|error| format!("Failed to start Node.js, which runs Playwright: {error}"))?;

		let (Some(stdin), Some(stdout)) = (process.stdin.take(), process.stdout.take()) else {
			return Err("The Playwright process has no input or output to talk over".to_string());
		};

		let pending = Arc::new(Mutex::new(Some(HashMap::<u64, ReplySender>::new())));
		let reader_pending = pending.clone();
		std::thread::spawn(move || {
			for line in BufReader::new(stdout).lines() {
				let Ok(line) = line else { break };
				let Ok(reply) = serde_json::from_str::<Reply>(&line) else {
					eprintln!("The Playwright process sent a line that is not a reply: {line}");
					continue;
				};

				let sender = reader_pending.lock().unwrap_or_else(PoisonError::into_inner).as_mut().and_then(|pending| pending.remove(&reply.id));
				if let Some(sender) = sender {
					let _ = sender.send(match reply.error {
						Some(error) => Err(error),
						None => Ok(reply.value),
					});
				}
			}

			// Dropping the senders fails every request still waiting
			reader_pending.lock().unwrap_or_else(PoisonError::into_inner).take();
		});

		Ok(Self {
			process: Mutex::new(process),
			stdin: Mutex::new(stdin),
			pending,
			next_id: AtomicU64::new(1),
		})
	}

	fn request<T: DeserializeOwned>(&self, request: Request) -> Result<T, String> {
		self.request_within(request, REQUEST_TIMEOUT)
	}

	// A page stuck in a loop never acknowledges input, so waiting is bounded to keep it from holding up the session forever
	fn request_within<T: DeserializeOwned>(&self, request: Request, timeout: Duration) -> Result<T, String> {
		let (id, receiver) = self.send(request)?;
		let value = match receiver.recv_timeout(timeout) {
			Ok(reply) => reply?,
			Err(mpsc::RecvTimeoutError::Timeout) => {
				if let Some(pending) = self.pending.lock().unwrap_or_else(PoisonError::into_inner).as_mut() {
					pending.remove(&id);
				}
				return Err("The page stopped responding".to_string());
			}
			Err(mpsc::RecvTimeoutError::Disconnected) => return Err(exited_error()),
		};
		serde_json::from_value(value).map_err(|error| format!("The Playwright process sent an unexpected reply: {error}"))
	}

	// Sends a request, returning its ID and where its reply will arrive
	fn send(&self, request: Request) -> Result<(u64, mpsc::Receiver<Result<serde_json::Value, String>>), String> {
		let id = self.next_id.fetch_add(1, Ordering::Relaxed);
		let (sender, receiver) = mpsc::channel();

		match self.pending.lock().unwrap_or_else(PoisonError::into_inner).as_mut() {
			Some(pending) => pending.insert(id, sender),
			None => return Err(exited_error()),
		};

		let mut line = serde_json::to_string(&Envelope { id, request }).map_err(|error| error.to_string())?;
		line.push('\n');
		let written = self.stdin.lock().unwrap_or_else(PoisonError::into_inner).write_all(line.as_bytes());
		if written.is_err() {
			return Err(exited_error());
		}

		Ok((id, receiver))
	}

	pub fn launch(&self, url: &str, size: UVec2, scale: f64, headed: bool, ready_selector: &str, timeout: Duration) -> Result<(), String> {
		let request = Request::Launch {
			url,
			size,
			scale,
			headed,
			ready_selector,
			timeout: timeout.as_millis() as u64,
		};

		// Loading the page and waiting for it to be ready may each take the whole timeout
		self.request_within(request, timeout * 2 + REQUEST_TIMEOUT)
	}

	/// Closes the browser, ending the Playwright process if it does not exit in time.
	pub fn close(&self) {
		let deadline = Instant::now() + CLOSE_TIMEOUT;

		let closed = self
			.send(Request::Close)
			.and_then(|(_, receiver)| receiver.recv_timeout(CLOSE_TIMEOUT).map_err(|_| "It did not answer in time".to_string())?);
		if let Err(error) = closed {
			eprintln!("Failed to close the browser: {error}");
		}

		let mut process = self.process.lock().unwrap_or_else(PoisonError::into_inner);
		while Instant::now() < deadline {
			if !matches!(process.try_wait(), Ok(None)) {
				return;
			}
			std::thread::sleep(EXIT_POLL_INTERVAL);
		}
		let _ = process.kill();
	}

	pub fn resize(&self, size: UVec2) -> Result<(), String> {
		self.request(Request::Resize { size })
	}

	pub fn mouse_move(&self, to: DVec2) -> Result<(), String> {
		self.request(Request::MouseMove { to })
	}

	pub fn mouse_down(&self, button: MouseButton, click_count: u32) -> Result<(), String> {
		self.request(Request::MouseDown { button, click_count })
	}

	pub fn mouse_up(&self, button: MouseButton, click_count: u32) -> Result<(), String> {
		self.request(Request::MouseUp { button, click_count })
	}

	pub fn wheel(&self, delta: DVec2) -> Result<(), String> {
		self.request(Request::Wheel { delta })
	}

	pub fn key_down(&self, key: &str) -> Result<(), String> {
		self.request(Request::KeyDown { key })
	}

	pub fn key_up(&self, key: &str) -> Result<(), String> {
		self.request(Request::KeyUp { key })
	}

	pub fn type_text(&self, text: &str) -> Result<(), String> {
		self.request(Request::Type { text })
	}

	/// Captures the page, or an area of it, as a PNG.
	pub fn screenshot(&self, clip: Option<Rectangle>) -> Result<Vec<u8>, String> {
		let encoded: String = self.request(Request::Screenshot { clip })?;
		base64::engine::general_purpose::STANDARD
			.decode(encoded)
			.map_err(|error| format!("The screenshot was not valid Base64: {error}"))
	}

	/// The area of the first element matching a CSS selector, if any matches and it is displayed.
	pub fn bounding_box(&self, selector: &str) -> Result<Option<Rectangle>, String> {
		self.request(Request::BoundingBox { selector })
	}

	/// The displayed elements matching a CSS selector, containing some text, or both, with the value of each one's field, as the field selector finds it.
	pub fn locate(&self, selector: Option<&str>, text: Option<&str>, field_selector: &str) -> Result<Vec<RawLocated>, String> {
		self.request(Request::Locate { selector, text, field_selector })
	}

	/// Waits for the page to draw its next frame, or for the timeout if it has stopped drawing.
	pub fn next_frame(&self, timeout: Duration) -> Result<(), String> {
		let timeout = timeout.as_millis() as u64;
		self.request(Request::NextFrame { timeout })
	}

	/// Whether the page has crashed, and the errors it has logged since this was last asked.
	pub fn take_events(&self) -> Result<PageEvents, String> {
		self.request(Request::TakeEvents)
	}
}
