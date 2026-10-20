use crate::application::{Editor, Environment};
use crate::dispatcher::Dispatcher;
use crate::messages::prelude::*;
use crate::node_graph_executor::{NodeGraphExecutor, NodeRuntime};
use crate::task::{Spawn, Spawner};
use document_container::store::DocumentStore;
use graph_craft::application_io::PlatformApplicationIo;
use graph_craft::application_io::resource::ResourceStorage;
use std::sync::Arc;

pub type ReadyEditorBuilder = EditorBuilder<Wake, Environment, Arc<dyn ResourceStorage>, Arc<dyn DocumentStore>>;
pub type ReadyRuntimeBuilder = RuntimeBuilder<Wake, PlatformApplicationIo>;

pub struct Builder<E = (), R = (), S = (), N = ()> {
	editor: E,
	runtime: R,
	spawner: S,
	random_seed: N,
}

impl Builder {
	pub fn new() -> Self {
		Self {
			editor: (),
			runtime: (),
			spawner: (),
			random_seed: (),
		}
	}
}

impl Default for Builder {
	fn default() -> Self {
		Self::new()
	}
}

impl<R, S, N> Builder<(), R, S, N> {
	pub fn with_editor(self, editor: ReadyEditorBuilder) -> Builder<ReadyEditorBuilder, R, S, N> {
		Builder {
			editor,
			runtime: self.runtime,
			spawner: self.spawner,
			random_seed: self.random_seed,
		}
	}
}

impl<E, S, N> Builder<E, (), S, N> {
	pub fn with_runtime(self, runtime: ReadyRuntimeBuilder) -> Builder<E, ReadyRuntimeBuilder, S, N> {
		Builder {
			editor: self.editor,
			runtime,
			spawner: self.spawner,
			random_seed: self.random_seed,
		}
	}
}

impl<E, R, N> Builder<E, R, (), N> {
	pub fn with_spawner(self, spawner: impl Spawn + Send + Sync + 'static) -> Builder<E, R, Spawner, N> {
		Builder {
			editor: self.editor,
			runtime: self.runtime,
			spawner: Arc::new(spawner),
			random_seed: self.random_seed,
		}
	}
}

impl<E, R, S> Builder<E, R, S, ()> {
	pub fn with_random_seed(self, random_seed: u64) -> Builder<E, R, S, u64> {
		Builder {
			editor: self.editor,
			runtime: self.runtime,
			spawner: self.spawner,
			random_seed,
		}
	}
}

impl Builder<ReadyEditorBuilder, ReadyRuntimeBuilder, Spawner, u64> {
	pub fn build(self) -> (Editor, NodeRuntime) {
		let Self {
			editor,
			runtime,
			spawner,
			random_seed,
		} = self;
		let EditorBuilder {
			wake: editor_wake,
			environment,
			resource_storage,
			document_store,
		} = editor;
		let RuntimeBuilder {
			wake: runtime_wake,
			mut application_io,
		} = runtime;

		graphene_std::uuid::set_uuid_seed(random_seed);

		let (executor, mut node_runtime) = NodeGraphExecutor::new(editor_wake.clone());

		let future_message_handler = FutureMessageHandler::new(spawner.clone(), editor_wake);
		let dispatcher = Dispatcher::new(environment, resource_storage, document_store, executor, future_message_handler);

		application_io.inject_resource_proxy(dispatcher.message_handlers.resource_storage_message_handler.resources());
		node_runtime.replace_application_io(application_io);
		node_runtime.replace_task_executor(spawner, runtime_wake);

		(Editor { dispatcher }, node_runtime)
	}
}

pub struct EditorBuilder<W = (), E = (), R = (), D = ()> {
	wake: W,
	environment: E,
	resource_storage: R,
	document_store: D,
}

impl EditorBuilder {
	pub fn new() -> Self {
		Self {
			wake: (),
			environment: (),
			resource_storage: (),
			document_store: (),
		}
	}
}

impl Default for EditorBuilder {
	fn default() -> Self {
		Self::new()
	}
}

impl<E, R, D> EditorBuilder<(), E, R, D> {
	pub fn with_wake(self, wake: impl Fn() + Send + Sync + 'static) -> EditorBuilder<Wake, E, R, D> {
		EditorBuilder {
			wake: Arc::new(wake),
			environment: self.environment,
			resource_storage: self.resource_storage,
			document_store: self.document_store,
		}
	}
}

impl<W, R, D> EditorBuilder<W, (), R, D> {
	pub fn with_environment(self, environment: Environment) -> EditorBuilder<W, Environment, R, D> {
		EditorBuilder {
			wake: self.wake,
			environment,
			resource_storage: self.resource_storage,
			document_store: self.document_store,
		}
	}
}

impl<W, E, D> EditorBuilder<W, E, (), D> {
	pub fn with_resource_storage(self, resource_storage: Arc<dyn ResourceStorage>) -> EditorBuilder<W, E, Arc<dyn ResourceStorage>, D> {
		EditorBuilder {
			wake: self.wake,
			environment: self.environment,
			resource_storage,
			document_store: self.document_store,
		}
	}
}

impl<W, E, R> EditorBuilder<W, E, R, ()> {
	pub fn with_document_store(self, document_store: Arc<dyn DocumentStore>) -> EditorBuilder<W, E, R, Arc<dyn DocumentStore>> {
		EditorBuilder {
			wake: self.wake,
			environment: self.environment,
			resource_storage: self.resource_storage,
			document_store,
		}
	}
}

pub struct RuntimeBuilder<W = (), A = ()> {
	wake: W,
	application_io: A,
}

impl RuntimeBuilder {
	pub fn new() -> Self {
		Self { wake: (), application_io: () }
	}
}

impl Default for RuntimeBuilder {
	fn default() -> Self {
		Self::new()
	}
}

impl<A> RuntimeBuilder<(), A> {
	/// Called from any thread when a graph task has completed, to run the runtime again as soon as possible.
	pub fn with_wake(self, wake: impl Fn() + Send + Sync + 'static) -> RuntimeBuilder<Wake, A> {
		RuntimeBuilder {
			wake: Arc::new(wake),
			application_io: self.application_io,
		}
	}
}

impl<W> RuntimeBuilder<W, ()> {
	pub fn with_application_io(self, application_io: PlatformApplicationIo) -> RuntimeBuilder<W, PlatformApplicationIo> {
		RuntimeBuilder { wake: self.wake, application_io }
	}
}
