use graphene_application_io::{Resource, ResourceFuture, ResourceHash, ResourceStorage, Resources};
use js_sys::Uint8Array;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::task::{Poll, Waker};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::{IdbDatabase, IdbRequest, IdbTransactionMode};

const STORE_NAME: &str = "resources";
const SCHEMA_VERSION: u32 = 1;

struct Inner {
	database: IdbDatabase,
	cache: HashMap<ResourceHash, Resource>,
	on_disk: HashSet<ResourceHash>,
}

pub struct IndexedDbResourceStorage {
	inner: Arc<Mutex<Inner>>,
}

impl IndexedDbResourceStorage {
	pub async fn load(database_name: &str) -> Result<Self, JsValue> {
		let database = open_database(database_name).await?;
		let on_disk = read_all_keys(&database).await.unwrap_or_else(|error| {
			log::error!("Failed to enumerate existing resource keys: {error:?}");
			HashSet::new()
		});

		Ok(Self {
			inner: Arc::new(Mutex::new(Inner {
				database,
				cache: HashMap::new(),
				on_disk,
			})),
		})
	}
}

impl Resources for IndexedDbResourceStorage {
	fn load(&self, hash: ResourceHash) -> ResourceFuture {
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
			let result = fetch_and_verify(inner, hash).await;
			sender.send(result);
		});

		Box::pin(receiver)
	}
}

impl ResourceStorage for IndexedDbResourceStorage {
	fn read(&mut self, hash: &ResourceHash) -> Option<Resource> {
		self.inner.lock().unwrap().cache.get(hash).cloned()
	}

	fn write(&mut self, data: &[u8]) -> ResourceHash {
		let hash = ResourceHash::from(data);
		let resource = Resource::new(Arc::<[u8]>::from(data));

		let mut guard = self.inner.lock().unwrap();

		guard.cache.insert(hash, resource);

		if guard.on_disk.contains(&hash) {
			return hash;
		}

		if let Err(error) = put_bytes(&guard.database, &hash, data) {
			log::error!("Failed to enqueue IDB put for {hash}: {error:?}");
		} else {
			guard.on_disk.insert(hash);
		}

		hash
	}

	fn contains(&mut self, hash: &ResourceHash) -> bool {
		let guard = self.inner.lock().unwrap();
		guard.cache.contains_key(hash) || guard.on_disk.contains(hash)
	}

	fn garbage_collect(&mut self, used: &[ResourceHash]) {
		let used_set: HashSet<ResourceHash> = used.iter().copied().collect();
		let mut guard = self.inner.lock().unwrap();

		guard.cache.retain(|hash, _| used_set.contains(hash));

		let to_delete: Vec<ResourceHash> = guard.on_disk.iter().copied().filter(|hash| !used_set.contains(hash)).collect();

		if to_delete.is_empty() {
			return;
		}

		match delete_many(&guard.database, &to_delete) {
			Ok(()) => {
				for hash in &to_delete {
					guard.on_disk.remove(hash);
				}
			}
			Err(error) => log::error!("Failed to enqueue IDB delete batch ({} entries): {error:?}", to_delete.len()),
		}
	}
}

// SAFETY: wasm is single-threaded; the non-`Send`/`Sync` JS handles are never observed across threads.
unsafe impl Send for IndexedDbResourceStorage {}
unsafe impl Sync for IndexedDbResourceStorage {}

async fn open_database(database_name: &str) -> Result<IdbDatabase, JsValue> {
	let factory = web_sys::window()
		.ok_or_else(|| JsValue::from_str("no window"))?
		.indexed_db()?
		.ok_or_else(|| JsValue::from_str("IndexedDB unavailable"))?;

	let open_request = factory.open_with_u32(database_name, SCHEMA_VERSION)?;

	let upgrade_request = open_request.clone();
	let upgrade_handler = Closure::once_into_js(move |_event: web_sys::Event| {
		let Some(target) = upgrade_request.result().ok().and_then(|value| value.dyn_into::<IdbDatabase>().ok()) else {
			log::error!("Upgrade event fired without a usable IdbDatabase result");
			return;
		};

		let names = target.object_store_names();
		let mut present = false;
		for index in 0..names.length() {
			if names.get(index).as_deref() == Some(STORE_NAME) {
				present = true;
				break;
			}
		}

		if !present && let Err(error) = target.create_object_store(STORE_NAME) {
			log::error!("Failed to create resource object store: {error:?}");
		}
	});
	open_request.set_onupgradeneeded(Some(upgrade_handler.as_ref().unchecked_ref()));

	await_request(&open_request).await?;

	open_request.result()?.dyn_into::<IdbDatabase>()
}

async fn read_all_keys(database: &IdbDatabase) -> Result<HashSet<ResourceHash>, JsValue> {
	let transaction = database.transaction_with_str(STORE_NAME)?;
	let store = transaction.object_store(STORE_NAME)?;
	let request = store.get_all_keys()?;
	let value = await_request(&request).await?;

	let array: js_sys::Array = value.dyn_into()?;
	let mut keys = HashSet::with_capacity(array.length() as usize);

	for index in 0..array.length() {
		let Some(key) = array.get(index).as_string() else {
			log::warn!("Skipping non-string IDB key at position {index}");
			continue;
		};
		match ResourceHash::try_from(key.as_str()) {
			Ok(hash) => {
				keys.insert(hash);
			}
			Err(error) => log::warn!("Skipping unparseable IDB key {key:?}: {error}"),
		}
	}

	Ok(keys)
}

fn put_bytes(database: &IdbDatabase, hash: &ResourceHash, data: &[u8]) -> Result<(), JsValue> {
	let transaction = database.transaction_with_str_and_mode(STORE_NAME, IdbTransactionMode::Readwrite)?;
	let store = transaction.object_store(STORE_NAME)?;

	let key = JsValue::from_str(&String::from(hash));
	let value: JsValue = Uint8Array::from(data).into();
	store.put_with_key(&value, &key)?;
	Ok(())
}

fn delete_many(database: &IdbDatabase, hashes: &[ResourceHash]) -> Result<(), JsValue> {
	let transaction = database.transaction_with_str_and_mode(STORE_NAME, IdbTransactionMode::Readwrite)?;
	let store = transaction.object_store(STORE_NAME)?;

	for hash in hashes {
		let key = JsValue::from_str(&String::from(hash));
		if let Err(error) = store.delete(&key) {
			log::error!("Failed to enqueue IDB delete for {hash}: {error:?}");
		}
	}

	Ok(())
}

async fn fetch_and_verify(inner: Arc<Mutex<Inner>>, hash: ResourceHash) -> Option<Resource> {
	let request = {
		let guard = inner.lock().unwrap();
		let database = &guard.database;

		let transaction = match database.transaction_with_str(STORE_NAME) {
			Ok(transaction) => transaction,
			Err(error) => {
				log::error!("Failed to open IDB readonly transaction for {hash}: {error:?}");
				return None;
			}
		};
		let store = match transaction.object_store(STORE_NAME) {
			Ok(store) => store,
			Err(error) => {
				log::error!("Failed to access IDB object store for {hash}: {error:?}");
				return None;
			}
		};
		let key = JsValue::from_str(&String::from(&hash));
		match store.get(&key) {
			Ok(request) => request,
			Err(error) => {
				log::error!("Failed to issue IDB get for {hash}: {error:?}");
				return None;
			}
		}
	};

	let value = match await_request(&request).await {
		Ok(value) => value,
		Err(error) => {
			log::error!("IDB get for {hash} failed: {error:?}");
			return None;
		}
	};

	if value.is_undefined() || value.is_null() {
		inner.lock().unwrap().on_disk.remove(&hash);
		return None;
	}

	let array = match value.dyn_into::<Uint8Array>() {
		Ok(array) => array,
		Err(value) => {
			log::error!("IDB returned non-Uint8Array value for {hash}: {value:?}");
			return None;
		}
	};
	let bytes = array.to_vec();

	let actual = ResourceHash::from(bytes.as_slice());
	if actual != hash {
		log::error!("IDB content-integrity failure: key {hash} stores bytes that hash to {actual}, deleting");
		let mut guard = inner.lock().unwrap();
		guard.on_disk.remove(&hash);
		if let Err(error) = delete_many(&guard.database, &[hash]) {
			log::error!("Failed to delete corrupted entry {hash}: {error:?}");
		}
		return None;
	}

	let resource = Resource::new(Arc::<[u8]>::from(bytes.as_slice()));
	inner.lock().unwrap().cache.insert(hash, resource.clone());
	Some(resource)
}

async fn await_request(request: &IdbRequest) -> Result<JsValue, JsValue> {
	let state: Arc<Mutex<RequestState>> = Arc::new(Mutex::new(RequestState::default()));

	let success_state = state.clone();
	let success_request = request.clone();
	let on_success = Closure::once_into_js(move |_event: web_sys::Event| {
		let result = success_request.result();
		let mut guard = success_state.lock().unwrap();
		guard.outcome = Some(result);
		if let Some(waker) = guard.waker.take() {
			waker.wake();
		}
	});

	let error_state = state.clone();
	let error_request = request.clone();
	let on_error = Closure::once_into_js(move |_event: web_sys::Event| {
		let error = error_request.error().unwrap_or(None).map(JsValue::from).unwrap_or_else(|| JsValue::from_str("unknown IDB error"));
		let mut guard = error_state.lock().unwrap();
		guard.outcome = Some(Err(error));
		if let Some(waker) = guard.waker.take() {
			waker.wake();
		}
	});

	request.set_onsuccess(Some(on_success.as_ref().unchecked_ref()));
	request.set_onerror(Some(on_error.as_ref().unchecked_ref()));

	RequestFuture { state }.await
}

#[derive(Default)]
struct RequestState {
	outcome: Option<Result<JsValue, JsValue>>,
	waker: Option<Waker>,
}

struct RequestFuture {
	state: Arc<Mutex<RequestState>>,
}

impl std::future::Future for RequestFuture {
	type Output = Result<JsValue, JsValue>;

	fn poll(self: std::pin::Pin<&mut Self>, context: &mut std::task::Context<'_>) -> Poll<Self::Output> {
		let mut guard = self.state.lock().unwrap();
		if let Some(outcome) = guard.outcome.take() {
			return Poll::Ready(outcome);
		}
		guard.waker = Some(context.waker().clone());
		Poll::Pending
	}
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

	fn poll(self: std::pin::Pin<&mut Self>, context: &mut std::task::Context<'_>) -> Poll<Self::Output> {
		let mut guard = self.state.lock().unwrap();
		if let Some(value) = guard.value.take() {
			return Poll::Ready(value);
		}
		guard.waker = Some(context.waker().clone());
		Poll::Pending
	}
}
