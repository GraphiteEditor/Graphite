//! OPFS (Origin Private File System) backend for browser wasm.

use crate::{AsyncContainer, ByteHolder, ContainerError, Result, validate_path, validate_prefix, with_trailing_slash};
use futures::channel::oneshot;
use js_sys::Uint8Array;
use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{
	Blob, DomException, FileSystemCreateWritableOptions, FileSystemDirectoryHandle, FileSystemFileHandle, FileSystemGetDirectoryOptions, FileSystemGetFileOptions, FileSystemWritableFileStream,
	WritableStream,
};

enum Mutation {
	Write {
		path: String,
		bytes: Vec<u8>,
	},
	Append {
		path: String,
		bytes: Vec<u8>,
	},
	Delete {
		path: String,
	},
	/// In-band flush barrier. The worker fulfills the sender once it dequeues this, which (by FIFO
	/// order) signals that every mutation enqueued before it has been applied to disk.
	Barrier(oneshot::Sender<()>),
}

struct Inner {
	directory: FileSystemDirectoryHandle,
	/// Tracks paths currently believed to be on disk (post-pending-writes). Lets `exists_non_blocking`
	/// answer synchronously since OPFS has no sync existence API. This is an optimistic prediction:
	/// a path is inserted when a write is queued and stays even if that background write later fails, so
	/// it can briefly over-report existence. The set is rebuilt from real disk state on the next `open`.
	on_disk: HashSet<String>,
	queue: VecDeque<Mutation>,
	worker_active: bool,
}

pub struct OpfsBackend {
	inner: Arc<Mutex<Inner>>,
}

// Safety: only built for browser wasm where JS handles never leave the main thread.
unsafe impl Send for OpfsBackend {}
unsafe impl Sync for OpfsBackend {}

impl OpfsBackend {
	/// Open (or create) `directory_name` under the OPFS root.
	pub async fn open(directory_name: &str) -> Result<Self> {
		let directory = open_directory(directory_name).await.map_err(js_err)?;
		let on_disk = enumerate_paths(&directory, "").await.map_err(js_err)?;
		Ok(Self {
			inner: Arc::new(Mutex::new(Inner {
				directory,
				on_disk,
				queue: VecDeque::new(),
				worker_active: false,
			})),
		})
	}

	fn directory(&self) -> FileSystemDirectoryHandle {
		self.inner.lock().unwrap().directory.clone()
	}

	/// Wait until all currently-queued non-blocking mutations have been applied to disk. Used by the
	/// awaited read paths so they observe queued writes. Plants a barrier at the back of the queue and
	/// awaits the worker reaching it, rather than draining inline, so the single worker stays the only
	/// mutator and FIFO order guarantees everything ahead of the barrier is already on disk.
	async fn flush(&self) {
		let (sender, receiver) = oneshot::channel();
		{
			let mut guard = self.inner.lock().unwrap();
			guard.queue.push_back(Mutation::Barrier(sender));
			kick_worker(&self.inner, &mut guard);
		}
		// The sender is only dropped without sending if the worker is torn down mid-drain; either way
		// there is nothing left to wait for, so a receive error is treated as "already flushed".
		let _ = receiver.await;
	}
}

impl AsyncContainer for OpfsBackend {
	async fn read(&self, path: &str) -> Result<ByteHolder> {
		validate_path(path)?;

		// Non-blocking writes only land on disk once the queue drains, so flush first and then treat
		// disk as authoritative. Draining (rather than folding the queue against an awaited base read)
		// avoids racing the background worker, which could otherwise double-apply a queued append.
		self.flush().await;

		let bytes = read_file(&self.directory(), path).await.map_err(js_err)?;
		Ok(ByteHolder::Owned(bytes))
	}

	async fn write(&self, path: &str, bytes: &[u8]) -> Result<()> {
		validate_path(path)?;
		// Flush first so this awaited write lands after any non-blocking mutation already queued for the
		// same path, then apply directly so the real disk error still propagates to the caller.
		self.flush().await;
		write_file(&self.directory(), path, bytes).await.map_err(js_err)?;
		self.inner.lock().unwrap().on_disk.insert(path.to_string());
		Ok(())
	}

	async fn append(&self, path: &str, bytes: &[u8]) -> Result<()> {
		validate_path(path)?;
		self.flush().await;
		append_file(&self.directory(), path, bytes).await.map_err(js_err)?;
		self.inner.lock().unwrap().on_disk.insert(path.to_string());
		Ok(())
	}

	async fn write_sized(&self, path: &str, size: usize, fill: &mut dyn FnMut(&mut [u8]) -> Result<()>) -> Result<()> {
		validate_path(path)?;
		let mut buffer = vec![0; size];
		fill(&mut buffer)?;
		self.flush().await;
		write_file(&self.directory(), path, &buffer).await.map_err(js_err)?;
		self.inner.lock().unwrap().on_disk.insert(path.to_string());
		Ok(())
	}

	async fn list(&self, prefix: &str) -> Result<Vec<String>> {
		validate_prefix(prefix)?;
		self.flush().await;
		list_entries(&self.directory(), prefix, EntryKind::File).await.map_err(js_err)
	}

	async fn list_dirs(&self, prefix: &str) -> Result<Vec<String>> {
		validate_prefix(prefix)?;
		self.flush().await;
		list_entries(&self.directory(), prefix, EntryKind::Directory).await.map_err(js_err)
	}

	async fn exists(&self, path: &str) -> bool {
		if validate_path(path).is_err() {
			return false;
		}
		self.flush().await;
		file_exists(&self.directory(), path).await
	}

	async fn remove(&self, path: &str) -> Result<()> {
		validate_path(path)?;
		self.flush().await;
		remove_file(&self.directory(), path).await.map_err(js_err)?;
		self.inner.lock().unwrap().on_disk.remove(path);
		Ok(())
	}

	fn store_non_blocking(&self, path: &str, bytes: &[u8]) -> Result<()> {
		validate_path(path)?;
		let mut guard = self.inner.lock().unwrap();
		guard.on_disk.insert(path.to_string());
		guard.queue.push_back(Mutation::Write {
			path: path.to_string(),
			bytes: bytes.to_vec(),
		});
		kick_worker(&self.inner, &mut guard);
		Ok(())
	}

	fn append_non_blocking(&self, path: &str, bytes: &[u8]) -> Result<()> {
		validate_path(path)?;
		let mut guard = self.inner.lock().unwrap();
		guard.on_disk.insert(path.to_string());
		guard.queue.push_back(Mutation::Append {
			path: path.to_string(),
			bytes: bytes.to_vec(),
		});
		kick_worker(&self.inner, &mut guard);
		Ok(())
	}

	fn remove_non_blocking(&self, path: &str) -> Result<()> {
		validate_path(path)?;
		let mut guard = self.inner.lock().unwrap();
		guard.on_disk.remove(path);
		guard.queue.push_back(Mutation::Delete { path: path.to_string() });
		kick_worker(&self.inner, &mut guard);
		Ok(())
	}

	fn exists_non_blocking(&self, path: &str) -> bool {
		validate_path(path).is_ok() && self.inner.lock().unwrap().on_disk.contains(path)
	}
}

fn kick_worker(inner: &Arc<Mutex<Inner>>, guard: &mut Inner) {
	if guard.worker_active {
		return;
	}
	guard.worker_active = true;
	let inner = inner.clone();
	spawn_local(drain_queue(inner));
}

/// Apply every queued mutation to disk, in FIFO order, until the queue is empty, then mark the worker
/// idle. Spawned once via [`kick_worker`] and runs as the sole mutator; the awaited read paths wait on
/// a [`Mutation::Barrier`] rather than draining themselves, so there is never more than one drainer.
async fn drain_queue(inner: Arc<Mutex<Inner>>) {
	loop {
		let (directory, mutation) = {
			let mut guard = inner.lock().unwrap();
			let Some(mutation) = guard.queue.pop_front() else {
				guard.worker_active = false;
				return;
			};
			(guard.directory.clone(), mutation)
		};

		match mutation {
			Mutation::Write { path, bytes } => {
				if let Err(error) = write_file(&directory, &path, &bytes).await {
					log::error!("OPFS background write for {path} failed: {error:?}");
				}
			}
			Mutation::Append { path, bytes } => {
				if let Err(error) = append_file(&directory, &path, &bytes).await {
					log::error!("OPFS background append for {path} failed: {error:?}");
				}
			}
			Mutation::Delete { path } => {
				if let Err(error) = remove_file(&directory, &path).await {
					log::error!("OPFS background delete for {path} failed: {error:?}");
				}
			}
			// A receive error on the waiter side just means the reader gave up; nothing to apply.
			Mutation::Barrier(sender) => {
				let _ = sender.send(());
			}
		}
	}
}

fn js_err(error: JsValue) -> ContainerError {
	ContainerError::Backend(format!("{error:?}"))
}

/// Resolve `directory_path` (a `/`-separated relative path) under the OPFS root, creating each
/// segment. OPFS rejects directory names containing `/`, so a multi-segment path like
/// `documents/<id>` must be descended one segment at a time rather than passed whole.
async fn open_directory(directory_path: &str) -> std::result::Result<FileSystemDirectoryHandle, JsValue> {
	let storage = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?.navigator().storage();
	let mut current: FileSystemDirectoryHandle = JsFuture::from(storage.get_directory()).await?.dyn_into()?;

	for segment in directory_path.split('/').filter(|segment| !segment.is_empty()) {
		let options = FileSystemGetDirectoryOptions::new();
		options.set_create(true);
		current = JsFuture::from(current.get_directory_handle_with_options(segment, &options)).await?.dyn_into()?;
	}

	Ok(current)
}

/// Descend the `/`-separated path against `root` and return the directory handle plus the final segment.
async fn descend<'a>(root: &FileSystemDirectoryHandle, relative: &'a str, create_dirs: bool) -> std::result::Result<(FileSystemDirectoryHandle, &'a str), JsValue> {
	let mut current = root.clone();
	let mut segments = relative.split('/').filter(|s| !s.is_empty()).collect::<Vec<_>>();
	let file = segments.pop().ok_or_else(|| JsValue::from_str("empty path"))?;

	for segment in segments {
		let options = FileSystemGetDirectoryOptions::new();
		options.set_create(create_dirs);
		current = JsFuture::from(current.get_directory_handle_with_options(segment, &options)).await?.dyn_into()?;
	}
	Ok((current, file))
}

async fn read_file(root: &FileSystemDirectoryHandle, path: &str) -> std::result::Result<Vec<u8>, JsValue> {
	let (directory, name) = descend(root, path, false).await?;

	let handle: FileSystemFileHandle = JsFuture::from(directory.get_file_handle(name)).await?.dyn_into()?;
	let file_value = JsFuture::from(handle.get_file()).await?;
	let blob: Blob = file_value.dyn_into()?;
	let buffer = JsFuture::from(blob.array_buffer()).await?;
	Ok(Uint8Array::new(&buffer).to_vec())
}

async fn write_file(root: &FileSystemDirectoryHandle, path: &str, bytes: &[u8]) -> std::result::Result<(), JsValue> {
	let (directory, name) = descend(root, path, true).await?;

	let options = FileSystemGetFileOptions::new();
	options.set_create(true);

	let handle: FileSystemFileHandle = JsFuture::from(directory.get_file_handle_with_options(name, &options)).await?.dyn_into()?;
	let writable: FileSystemWritableFileStream = JsFuture::from(handle.create_writable()).await?.dyn_into()?;
	let stream: WritableStream = writable.clone().unchecked_into();

	// Wrap in an async block so any failure, including a synchronous JS throw from `write_with_js_u8_array`,
	// aborts the stream before returning instead of leaving a dangling locked writable.
	let write = async {
		let array = Uint8Array::from(bytes);
		JsFuture::from(writable.write_with_js_u8_array(&array)?).await?;
		Ok::<(), JsValue>(())
	}
	.await;

	if let Err(error) = write {
		let _ = JsFuture::from(stream.abort()).await;
		return Err(error);
	}

	JsFuture::from(stream.close()).await?;
	Ok(())
}

async fn append_file(root: &FileSystemDirectoryHandle, path: &str, bytes: &[u8]) -> std::result::Result<(), JsValue> {
	let (directory, name) = descend(root, path, true).await?;

	let file_options = FileSystemGetFileOptions::new();
	file_options.set_create(true);
	let handle: FileSystemFileHandle = JsFuture::from(directory.get_file_handle_with_options(name, &file_options)).await?.dyn_into()?;

	// Determine the current end-of-file so we can seek there before writing.
	let file_value = JsFuture::from(handle.get_file()).await?;
	let blob: Blob = file_value.dyn_into()?;
	let offset = blob.size();

	// `keepExistingData: true` preserves bytes outside the written range; without it OPFS truncates to the written length.
	let writable_options = FileSystemCreateWritableOptions::new();
	writable_options.set_keep_existing_data(true);
	let writable: FileSystemWritableFileStream = JsFuture::from(handle.create_writable_with_options(&writable_options)).await?.dyn_into()?;
	let stream: WritableStream = writable.clone().unchecked_into();

	// Wrap in an async block so a synchronous JS throw from `seek_with_f64`/`write_with_js_u8_array`
	// aborts the stream instead of bypassing the abort via `?`.
	let seek_and_write = async {
		JsFuture::from(writable.seek_with_f64(offset)?).await?;
		let array = Uint8Array::from(bytes);
		JsFuture::from(writable.write_with_js_u8_array(&array)?).await?;
		Ok::<(), JsValue>(())
	}
	.await;

	if let Err(error) = seek_and_write {
		let _ = JsFuture::from(stream.abort()).await;
		return Err(error);
	}

	JsFuture::from(stream.close()).await?;
	Ok(())
}

async fn remove_file(root: &FileSystemDirectoryHandle, path: &str) -> std::result::Result<(), JsValue> {
	let (directory, name) = descend(root, path, false).await?;
	JsFuture::from(directory.remove_entry(name)).await?;
	Ok(())
}

async fn file_exists(root: &FileSystemDirectoryHandle, path: &str) -> bool {
	let Ok((directory, name)) = descend(root, path, false).await else {
		return false;
	};
	JsFuture::from(directory.get_file_handle(name)).await.is_ok()
}

#[derive(Clone, Copy)]
enum EntryKind {
	File,
	Directory,
}

async fn list_entries(root: &FileSystemDirectoryHandle, prefix: &str, want: EntryKind) -> std::result::Result<Vec<String>, JsValue> {
	// `.` and the empty string both name the container root.
	let prefix = if prefix == "." { "" } else { prefix };

	let directory = if prefix.is_empty() {
		root.clone()
	} else {
		let mut current = root.clone();
		for segment in prefix.split('/').filter(|s| !s.is_empty()) {
			let options = FileSystemGetDirectoryOptions::new();
			options.set_create(false);
			current = match JsFuture::from(current.get_directory_handle_with_options(segment, &options)).await {
				Ok(value) => value.dyn_into()?,
				Err(error) if is_not_found(&error) => return Ok(Vec::new()),
				Err(error) => return Err(error),
			};
		}
		current
	};

	let entries = directory.entries();
	let iterator: js_sys::AsyncIterator = entries.unchecked_into();
	let mut results = Vec::new();
	let prefix_with_slash = with_trailing_slash(prefix);

	let want_kind = match want {
		EntryKind::File => web_sys::FileSystemHandleKind::File,
		EntryKind::Directory => web_sys::FileSystemHandleKind::Directory,
	};

	loop {
		let next: js_sys::IteratorNext = JsFuture::from(iterator.next()?).await?.unchecked_into();
		if next.done() {
			break;
		}
		let pair: js_sys::Array = next.value().unchecked_into();
		let Some(name) = pair.get(0).as_string() else { continue };
		let handle: web_sys::FileSystemHandle = pair.get(1).unchecked_into();
		if handle.kind() != want_kind {
			continue;
		}
		results.push(format!("{prefix_with_slash}{name}"));
	}
	Ok(results)
}

/// Walk every file under `prefix` (recursively) and collect their full container paths.
/// Used at open time to populate the in-memory tracking set.
async fn enumerate_paths(root: &FileSystemDirectoryHandle, prefix: &str) -> std::result::Result<HashSet<String>, JsValue> {
	let mut paths = HashSet::new();
	let mut to_visit = vec![prefix.to_string()];

	while let Some(current_prefix) = to_visit.pop() {
		for file in list_entries(root, &current_prefix, EntryKind::File).await? {
			paths.insert(file);
		}
		for dir in list_entries(root, &current_prefix, EntryKind::Directory).await? {
			to_visit.push(dir);
		}
	}

	Ok(paths)
}

fn is_not_found(error: &JsValue) -> bool {
	error.dyn_ref::<DomException>().is_some_and(|error| error.name() == "NotFoundError")
}
