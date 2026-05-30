use graphene_application_io::resource::{LoadResource, Resource, ResourceFuture, ResourceHash, ResourceStorage};
use js_sys::Uint8Array;
use std::collections::{HashMap, HashSet, VecDeque};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{Blob, DomException, FileSystemDirectoryHandle, FileSystemFileHandle, FileSystemGetDirectoryOptions, FileSystemGetFileOptions, FileSystemWritableFileStream, WritableStream};

enum Mutation {
	Write { hash: ResourceHash, bytes: Arc<[u8]> },
	Delete { hash: ResourceHash },
}

struct Inner {
	directory: FileSystemDirectoryHandle,
	cache: HashMap<ResourceHash, Resource>,
	on_disk: HashSet<ResourceHash>,
	queue: VecDeque<Mutation>,
	worker_active: bool,
	persist_requested: bool,
}

pub struct OpfsResourceStorage {
	inner: Arc<Mutex<Inner>>,
}

// SAFETY: This is only compiled for browser wasm, where JS handles remain on the main thread.
unsafe impl Send for OpfsResourceStorage {}
unsafe impl Sync for OpfsResourceStorage {}

impl OpfsResourceStorage {
	pub async fn load(directory_name: &str) -> Result<Self, JsValue> {
		let directory = open_resource_directory(directory_name).await?;
		let on_disk = enumerate_hashes(&directory).await?;

		Ok(Self {
			inner: Arc::new(Mutex::new(Inner {
				directory,
				cache: HashMap::new(),
				on_disk,
				queue: VecDeque::new(),
				worker_active: false,
				persist_requested: false,
			})),
		})
	}
}

impl LoadResource for OpfsResourceStorage {
	fn load(&self, hash: ResourceHash) -> ResourceFuture<'_> {
		let inner = self.inner.clone();

		{
			let guard = inner.lock().unwrap();
			if let Some(resource) = guard.cache.get(&hash) {
				let resource = resource.clone();
				return Box::pin(async move { Some(resource) });
			}
			if !guard.on_disk.contains(&hash) {
				return Box::pin(async move { None });
			}
		}

		let (sender, receiver) = oneshot();
		spawn_local(async move {
			sender.send(read_from_opfs(inner, hash).await);
		});

		Box::pin(receiver)
	}
}

impl ResourceStorage for OpfsResourceStorage {
	fn store(&self, data: &[u8]) -> ResourceHash {
		let hash = ResourceHash::from(data);
		let mut guard = self.inner.lock().unwrap();

		let mut bytes = None;
		if !guard.cache.contains_key(&hash) {
			let resource_bytes = Arc::<[u8]>::from(data);
			guard.cache.insert(hash, Resource::new_unchecked(resource_bytes.clone(), hash));
			bytes = Some(resource_bytes);
		}

		if !guard.on_disk.contains(&hash) {
			let bytes = bytes.unwrap_or_else(|| Arc::<[u8]>::from(data));
			guard.on_disk.insert(hash);
			guard.queue.push_back(Mutation::Write { hash, bytes });
			kick_worker(&self.inner, &mut guard);
		}

		hash
	}

	fn contains(&self, hash: &ResourceHash) -> bool {
		let guard = self.inner.lock().unwrap();
		guard.cache.contains_key(hash) || guard.on_disk.contains(hash)
	}

	fn garbage_collect(&self, used: &[ResourceHash]) {
		let used: HashSet<ResourceHash> = used.iter().copied().collect();
		let mut guard = self.inner.lock().unwrap();

		guard.cache.retain(|hash, _| used.contains(hash));

		let unused: Vec<ResourceHash> = guard.on_disk.iter().copied().filter(|hash| !used.contains(hash)).collect();
		for hash in unused {
			guard.on_disk.remove(&hash);
			guard.queue.push_back(Mutation::Delete { hash });
		}

		if !guard.queue.is_empty() {
			kick_worker(&self.inner, &mut guard);
		}
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

async fn drain_queue(inner: Arc<Mutex<Inner>>) {
	loop {
		let (directory, mutation, persist_requested) = {
			let mut guard = inner.lock().unwrap();
			let Some(mutation) = guard.queue.pop_front() else {
				guard.worker_active = false;
				return;
			};

			let persist_requested = matches!(mutation, Mutation::Write { .. }) && !guard.persist_requested;
			if persist_requested {
				guard.persist_requested = true;
			}

			(guard.directory.clone(), mutation, persist_requested)
		};

		if persist_requested {
			request_persistence().await;
		}

		match mutation {
			Mutation::Write { hash, bytes } => {
				if let Err(error) = write_file(&directory, &hash, &bytes).await {
					log::error!("OPFS write for {hash} failed: {error:?}");
				}
			}
			Mutation::Delete { hash } => {
				if let Err(error) = delete_file(&directory, &hash).await {
					log::error!("OPFS delete for {hash} failed: {error:?}");
				}
			}
		}
	}
}

async fn request_persistence() {
	let Some(window) = web_sys::window() else {
		log::warn!("OPFS persist() skipped: no window");
		return;
	};
	let storage = window.navigator().storage();

	match storage.persist() {
		Ok(promise) => match JsFuture::from(promise).await {
			Ok(value) if value.as_bool() == Some(true) => {}
			Ok(_) => log::warn!("OPFS persistence was not granted; browser may evict resources under storage pressure"),
			Err(error) => log::warn!("OPFS persist() rejected: {error:?}"),
		},
		Err(error) => log::warn!("OPFS persist() threw: {error:?}"),
	}
}

async fn open_resource_directory(directory_name: &str) -> Result<FileSystemDirectoryHandle, JsValue> {
	let storage = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?.navigator().storage();
	let root: FileSystemDirectoryHandle = JsFuture::from(storage.get_directory()).await?.dyn_into()?;

	let options = FileSystemGetDirectoryOptions::new();
	options.set_create(true);

	JsFuture::from(root.get_directory_handle_with_options(directory_name, &options)).await?.dyn_into()
}

async fn enumerate_hashes(directory: &FileSystemDirectoryHandle) -> Result<HashSet<ResourceHash>, JsValue> {
	let iterator = directory.keys();
	let mut hashes = HashSet::new();

	loop {
		let next: js_sys::IteratorNext = JsFuture::from(iterator.next()?).await?.unchecked_into();
		if next.done() {
			break;
		}

		let Some(name) = next.value().as_string() else {
			log::warn!("Skipping non-string OPFS resource entry");
			continue;
		};

		match ResourceHash::try_from(name.as_str()) {
			Ok(hash) => {
				hashes.insert(hash);
			}
			Err(error) => log::warn!("Skipping non-resource OPFS entry {name:?}: {error}"),
		}
	}

	Ok(hashes)
}

async fn write_file(directory: &FileSystemDirectoryHandle, hash: &ResourceHash, bytes: &[u8]) -> Result<(), JsValue> {
	let options = FileSystemGetFileOptions::new();
	options.set_create(true);

	let name = file_name(hash);
	let handle: FileSystemFileHandle = JsFuture::from(directory.get_file_handle_with_options(&name, &options)).await?.dyn_into()?;
	let writable: FileSystemWritableFileStream = JsFuture::from(handle.create_writable()).await?.dyn_into()?;
	let stream: WritableStream = writable.clone().unchecked_into();
	let bytes = Uint8Array::from(bytes);

	if let Err(error) = JsFuture::from(writable.write_with_js_u8_array(&bytes)?).await {
		let _ = JsFuture::from(stream.abort()).await;
		return Err(error);
	}

	JsFuture::from(stream.close()).await?;
	Ok(())
}

async fn delete_file(directory: &FileSystemDirectoryHandle, hash: &ResourceHash) -> Result<(), JsValue> {
	let name = file_name(hash);
	match JsFuture::from(directory.remove_entry(&name)).await {
		Ok(_) => Ok(()),
		Err(error) if is_not_found(&error) => Ok(()),
		Err(error) => Err(error),
	}
}

async fn read_from_opfs(inner: Arc<Mutex<Inner>>, hash: ResourceHash) -> Option<Resource> {
	let directory = {
		let guard = inner.lock().unwrap();
		if let Some(resource) = guard.cache.get(&hash) {
			return Some(resource.clone());
		}
		if !guard.on_disk.contains(&hash) {
			return None;
		}
		guard.directory.clone()
	};

	let name = file_name(&hash);
	let handle: FileSystemFileHandle = match JsFuture::from(directory.get_file_handle(&name)).await {
		Ok(value) => match value.dyn_into() {
			Ok(handle) => handle,
			Err(value) => {
				log::error!("OPFS returned non-file handle for {hash}: {value:?}");
				return None;
			}
		},
		Err(error) if is_not_found(&error) => return None,
		Err(error) => {
			log::error!("OPFS getFileHandle for {hash} failed: {error:?}");
			return None;
		}
	};

	let file = match JsFuture::from(handle.get_file()).await {
		Ok(value) => value,
		Err(error) => {
			log::error!("OPFS getFile for {hash} failed: {error:?}");
			return None;
		}
	};
	let blob: Blob = match file.dyn_into() {
		Ok(blob) => blob,
		Err(value) => {
			log::error!("OPFS getFile returned non-Blob for {hash}: {value:?}");
			return None;
		}
	};
	let buffer = match JsFuture::from(blob.array_buffer()).await {
		Ok(buffer) => buffer,
		Err(error) => {
			log::error!("OPFS arrayBuffer for {hash} failed: {error:?}");
			return None;
		}
	};

	let bytes: Arc<[u8]> = Uint8Array::new(&buffer).to_vec().into();
	let actual = ResourceHash::from(bytes.as_ref());
	if actual != hash {
		log::error!("OPFS content-integrity failure: file {hash} hashes to {actual}");
		return None;
	}

	let resource = Resource::new_unchecked(bytes, hash);
	let mut guard = inner.lock().unwrap();
	if let Some(resource) = guard.cache.get(&hash) {
		return Some(resource.clone());
	}
	if !guard.on_disk.contains(&hash) {
		return None;
	}

	guard.cache.insert(hash, resource.clone());
	Some(resource)
}

fn file_name(hash: &ResourceHash) -> String {
	String::from(hash)
}

fn is_not_found(error: &JsValue) -> bool {
	error.dyn_ref::<DomException>().is_some_and(|error| error.name() == "NotFoundError")
}

fn oneshot() -> (OneshotSender, OneshotReceiver) {
	let state = Arc::new(Mutex::new(OneshotState::default()));
	(OneshotSender { state: state.clone() }, OneshotReceiver { state })
}

#[derive(Default)]
struct OneshotState {
	value: Option<Option<Resource>>,
	waker: Option<Waker>,
}

struct OneshotSender {
	state: Arc<Mutex<OneshotState>>,
}

impl OneshotSender {
	fn send(self, value: Option<Resource>) {
		let mut guard = self.state.lock().unwrap();
		guard.value = Some(value);
		if let Some(waker) = guard.waker.take() {
			waker.wake();
		}
	}
}

struct OneshotReceiver {
	state: Arc<Mutex<OneshotState>>,
}

impl std::future::Future for OneshotReceiver {
	type Output = Option<Resource>;

	fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
		let mut guard = self.state.lock().unwrap();
		if let Some(value) = guard.value.take() {
			return Poll::Ready(value);
		}
		guard.waker = Some(context.waker().clone());
		Poll::Pending
	}
}
