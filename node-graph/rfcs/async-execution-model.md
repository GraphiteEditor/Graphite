- RFC PR: [#4556](https://github.com/GraphiteEditor/Graphite/pull/4556)
- Implementation PR: [#4731](https://github.com/GraphiteEditor/Graphite/pull/4731)

# Summary

Replace the boxed-future execution model with synchronous, poll-based evaluation. Nodes return a `Poll<T>` state instead of a future. Async work runs on an executor outside of the graph, and when a task finishes it bumps a per-source generation counter. The counters are part of the `Context`, so the memo nodes downstream of a finished task see a new context and miss, while everything else stays cached. Deciding which part of the graph observes which counter reuses the context nullification pass from the [fine-grained context caching RFC](fine-grained-context-caching.md).

# Motivation

Currently every node's `eval` returns a `DynFuture` (`Pin<Box<dyn Future>>`), even a node that adds two numbers. A full graph evaluation is one large future which we drive once per render request. This has several costs:

- Every eval boxes a future, so pure nodes pay for async machinery they never use.
- There is no progressive rendering. One slow async operation stalls the whole evaluation and there is nothing to show in the meantime.
- Driving async work by re-evaluating the graph costs a full traversal per poll.
- Errors are propagated as panics which we catch with `catch_unwind` at the root.
- A precompiled graph (the graph emitted as Rust source and run against a small runtime) can't be built on a box per edge.

Most nodes are not asynchronous at all. Only a handful of them (web requests, resource loading, rasterization on the web) actually wait on something, so the model should make the common case cheap and give the few async nodes a way to report "not done yet".

# Guide-level explanation

## Evaluation states

Evaluating a node is synchronous and always returns immediately. Instead of a value, a node returns a `Poll<T>`:

```rs
enum Poll<T> {
    /// Waiting on at least one future, with no placeholder to compute with.
    Pending,
    Final(T),
    /// Computed from provisional data; recomputed once the source generation bumps.
    Partial(T),
    /// An error occurred; the boxed value is a stand-in, downstream keeps computing.
    Fallback(Box<(T, NodeError)>),
    /// An error occurred and no stand-in exists; the cone cannot compute this frame.
    Error(Box<NodeError>),
}
```

Three of the variants carry a value (`Final`, `Partial`, `Fallback`) and two don't (`Pending`, `Error`). A value keeps flowing through the graph: a node that receives `Partial` computes as usual and marks its own output as `Partial`, and a node that receives `Fallback` computes on the stand-in value and passes the error along. Without a value there is nothing to compute with, so `Pending` and `Error` stop evaluation of everything that depends on them for this frame.

When a node has several inputs, their states are combined with a meet: the result is `Final` only if all inputs are `Final`, and `Error` wins over `Pending`.

## Writing nodes

Node authors keep writing plain Rust functions, and the node macro generates the status handling. A function can return one of four things:

- `-> T` for a pure kernel. The macro combines the input states and the author never sees them.
- `-> Result<T, Interrupt>` for kernels that can fail or that evaluate lazy inputs themselves. `Interrupt` is either `Pending` or `Error(Box<NodeError>)` and implements `From<NodeError>`, so input bail-outs and the node's own errors both work with `?`.
- `-> Result<T, (T, NodeError)>` for an error with a stand-in value.
- `-> Poll<T>` for full manual control.

A lazy input (`content: impl Node<Output = T>`) is evaluated with `content.eval(ctx)?`. The `?` bails out of the kernel if the input is `Pending` or `Error`, which is the only sensible thing to do without a value. `Partial` and `Fallback` inputs return `Ok(value)`, and the macro remembers that one of the inputs was provisional or had failed.

```rs
#[node_macro::node(category("Debug"))]
fn error<T>(_: impl Ctx, value: Item<T>, fail: Item<bool>) -> Result<Item<T>, Interrupt> {
	match *fail.element() {
		true => Err(NodeError::new("debug error").into()),
		false => Ok(value),
	}
}
```

Only inputs that were actually evaluated affect the result, so the branch a `switch` did not take does not turn its output `Partial`. Nodes for which placeholder data makes no sense can opt out with `#[node(no_partial)]`, which treats a `Partial` input like a `Pending` one.

Migrating an existing node that is only async because it awaits its inputs is mechanical: remove `async`, replace `.eval(ctx).await` with `.eval(ctx)?`, and return `Result<T, Interrupt>`.

## Async nodes

An `async fn` node is a *source*: it starts work that runs outside of the graph. The macro evaluates its inputs, spawns the future onto the runtime, and returns a placeholder right away:

```rs
#[node_macro::node(category("Web Request"), placeholder = pending_text)]
async fn get_request(_: impl Ctx, url: Item<String>) -> Item<String> { ... }

fn pending_text(_url: &Item<String>) -> Item<String> {
	Item::default()
}
```

The placeholder function receives references to the node's inputs. While the request is in flight, the node returns `Partial(placeholder)`. Without a placeholder function it returns `Pending`. Once the request has finished, the node returns its result as `Final`.

The future has to be `'static`, so the async body cannot borrow the context or evaluate inputs lazily. It gets clones of everything it needs. If any input is still `Partial`, the node returns its placeholder without spawning, so no request is sent based on placeholder data.

## How a finished task reaches the graph

Every async node has a `SourceId`, which is a hash of its document node path. For each source, the runtime keeps a generation counter that is incremented when one of the source's tasks finishes. Before every evaluation, the editor puts a snapshot of all counters into the root `Context`.

Memo nodes already hash the context to decide whether they can reuse their cached value. With the generations in the context, a finished task changes the hash for exactly the memo nodes that depend on that source, and the next evaluation recomputes them with the real value. Everything else still hits its cache. Nothing walks the graph at runtime to find the affected nodes.

Which sources a part of the graph depends on is known at compile time, using the same analysis that decides which context features a branch needs. Where branches with different sources meet, the compiler inserts a context modification node that drops the generations of the other sources, just like it already drops unused features.

## Partial results in the editor

A `Partial` frame is shown in the viewport like a `Final` one, so the rest of the document stays visible while a request is pending. If the whole graph is `Pending`, the viewport keeps showing the last frame. An export waits until the result is `Final`.

Errors are shown on the node they came from. Errors that are only produced because some input was still a placeholder are never shown, since they would disappear once the real value arrives.

# Reference-level explanation

## Node trait

```rs
pub trait Node<Input: ?Sized> {
    type Output;
    fn eval(&self, input: &Input) -> Poll<Self::Output>;
}
```

Some decisions about the shape of this trait:

- **Plain `&self` receiver.** An `&'i self` receiver requires the forwarding impl `impl Node for &'i N`, which ties the borrow of the node to the eval lifetime, and shared references then can't satisfy higher-ranked bounds. With `&self`, sharing and type erasure compose. In exchange, nodes cannot lend their own storage and values cross edges owned.
- **No lifetime parameter on the trait.** With a plain `&self`, a lifetime in the output can only come from the input type, so the trait doesn't need one. `Poll` is part of the signature the same way `std::task::Poll` is part of `Future::poll`.
- **Input by reference.** `Input` is the owned context type and callers pass `&Context`. A node that modifies the context keeps one local copy, changes it between calls, and passes `&local` down. This is sound because nodes never store their input.

`Node::reset` is removed, and so is `Type::Future`, because no node returns a future anymore. The type-erased form is `dyn Node<dyn Any, Output = Box<dyn Any>>`: the input is downcast by reference and only the output value is boxed. Node constructors are plain functions.

## Status cell

The generated `eval` owns a `Status` cell on the stack. Lazy inputs are passed to the kernel as `LazyInput` handles that hold a reference to the cell and their input index, and value inputs are evaluated through the same cell before the kernel runs. `LazyInput::eval` maps the input state as follows:

- `Final(v)` returns `Ok(v)`.
- `Partial(v)` returns `Ok(v)` and marks the cell as partial.
- `Fallback(v, e)` returns `Ok(v)` and stores `e` if it is the first input error.
- `Pending` and `Error(e)` return `Err(Interrupt)`. The cell remembers the strongest interrupt.

After the kernel returns, the generated code folds the cell into the result. `Poll`-tier kernels get their lazy inputs as plain `&impl Node` handles that only add trace indices, and their returned state is merged with the cell.

Finality is tracked as a separate `Finality` (`Final | Partial`) in the cell instead of a sixth `Poll` variant. The reason is that `meet(Partial, Fallback)` would otherwise be `Fallback`, and an error that only exists because of a placeholder would look like a real document error. With the separate flag, partiality wins:

- A node with any `Partial` input returns `Partial`, and stand-in errors from its inputs are dropped.
- An error the node raises itself on provisional data is withheld. A stand-in error is dropped, and an `Err` from the kernel becomes `Pending`.
- An `Error` coming from an input still propagates, because it was computed from final data further upstream.

The layout is one tag and one payload word for pointer-sized values: `Poll<Arc<T>>` and `Poll<u32>` are both 16 bytes. The stand-in and its error share a box, and `Error` boxes the error alone. Progress percentages are deliberately not part of `Poll`. They change often and must not invalidate the dataflow, so they belong in the monitor side channel.

## Errors

`NodeError` wraps an `Arc<dyn Error + Send + Sync>`, so nodes can return any error type. It records a trace of input indices from the node that raised it up to the output. We use indices instead of names because a node can have two inputs of the same type. The handles push the indices, so node code never does.

If a node raises its own error while one of its inputs has already failed, the input's error is kept in the new error's `upstream` list.

The editor resolves a trace by walking the input indices through the executor's node tree, starting at the output node, and then maps the resulting proto node to its document node paths. `upstream` errors are resolved starting from the node that raised the outer error. Rank promotion adapters are inserted between nodes at build time and add their own entry to the trace, so the walk currently skips them by hand.

Panics are not part of this model. The editor runtime still wraps evaluation in `catch_unwind` and reports a panic separately from node errors.

## Async node code generation

For an `async fn` node, the macro adds two hidden inputs: the `RuntimeHandle` from the `runtime` scope and the node's `SourceId`, which is injected as a reflection input like the document node path is. The `SourceId` is a hash of the document node path, so it stays the same across recompiles.

The struct gets a `Slots<T>`, an `Arc<Mutex<HashMap<u64, Slot<T>>>>`:

```rs
enum Slot<T> {
	Ready(Poll<T>),
	Pending(AbortHandle),
}
```

The slots are keyed by a hash of the node's context with the node's own source removed from the generations, so the generation bump caused by its own task does not change the key. Evaluation goes like this:

1. Evaluate the inputs through the status cell. Bail on `Pending` or `Error`, and return the placeholder (or `Pending`) if any input is `Partial`.
2. A `Ready` slot returns its stored state. A `Pending` slot returns the placeholder.
3. Otherwise build the future and poll it once inline with a no-op waker. A future that is ready immediately is stored as `Ready` without spawning a task or bumping a generation.
4. Otherwise spawn it onto the runtime, store a `Pending` slot with the task's abort handle, and return the placeholder.

The spawned task holds a weak reference to the slot map. A node that `BorrowTree` keeps across a recompile still gets its result. When the last owner of a slot map is dropped, its `Pending` slots are dropped too, and that aborts their tasks. Since the mutex around the map also publishes the result to the next `eval`, the slot needs no additional synchronization.

Async nodes don't get the element-wise variants over lists that other nodes get.

## Generations and context nullification

The runtime keeps a `HashMap<SourceId, u64>` of generations. The task wrapper bumps the generation after the task has finished:

```rs
task.await;
if let Some(generation) = generations.lock().get_mut(&source) {
	*generation += 1;
}
```

A missing entry means the source was removed in a recompile, and then the bump does nothing.

Before every evaluation the editor takes a snapshot of the generations, sorted by `SourceId` so that hashing is deterministic, and puts it into the root context. `OwnedContextImpl` has a new `generations` field which is part of its hash.

`find_context_dependencies` now tracks a `ContextModification` for each node instead of only `ContextFeatures`:

```rs
pub struct ContextModification {
	pub features: ContextFeatures,
	pub sources: BTreeSet<SourceId>,
}
```

The `SourceId` value node of an async node contributes that source, and a node's set of sources is the union of its children's. Unlike features, sources are never removed on the way to the root, because only the root injects generations. When the branches of a node have different modifications, the existing rule inserts a `context_modification` node with the branch's `ContextModification` as payload. It nullifies the unused features as before and also drops the generations of all sources outside the set.

Context dependencies are no longer stored on document nodes. The preprocessor fills them in from the node registry metadata.

## Caching

A memo node keeps a single entry. It caches `Final`, `Partial`, and `Fallback` results and never caches `Pending` or `Error`. Caching `Partial` is fine because the generation bump that finalizes the value also changes the key.

The render output cache includes the generation snapshot in its tile key. Tiles store their `Poll` state. Partial tiles are composited into a `Partial` frame, and a `Pending` or `Error` tile stops the whole frame.

## Runtime

The runtime is split into a host-facing and a node-facing trait:

```rs
/// Host-provided task executor with no graph knowledge.
pub trait Spawn {
	fn spawn(&self, task: Task);
}

/// Node-facing scope input.
pub trait Runtime {
	fn spawn(&self, source: SourceId, task: Task) -> AbortHandle;
}
```

`Task` is a boxed `'static` future that is `Send` on native targets only. The editor owns an `AsyncRuntime`, which updates the live sources (`retain_sources`) and takes snapshots of the generations (`snapshot`). Nodes get a cloneable `RuntimeHandle` to the same state through the `runtime` scope input, which is always present. Graphs without async nodes never read it.

The host provides the `Spawn` implementation (a tokio runtime on desktop and `spawn_local` on the web) and a `Clock` (`Instant` on desktop, `performance.now()` on the web), which nodes can read through `RuntimeHandle::clock`. The editor wraps the spawner so that a finished task calls the runtime's wake callback (a channel send on desktop, a queued runtime run on the web). `Runtime::spawn` returns an abort handle; an aborted task bumps nothing.

Executor construction, update, and execution are plain functions, and `execute` returns `Poll<TaggedValue>`. The only await left in the editor runtime is the GPU readback for exports and the eyedropper, which can't block on the web. On desktop, the runtime runs on its own thread under `block_on`, and the readback polls the device until the copy is done.

We don't use wakers to notify the graph. A waker resumes a suspended computation, and graph evaluations never suspend: every evaluation is a fresh call. A wake also doesn't mean that the task has finished, and wakers don't compose across host executors.

## Scheduling

- Generations are read once per evaluation. A task that finishes during an evaluation takes effect in the next one.
- The bump flag is checked once per runtime run, so N finished tasks cause one re-render of the last viewport request.
- The host marks frames with `begin_frame` (each animation frame on the web, the runtime thread's loop on desktop). A re-evaluation caused by finished tasks only starts while the frame is within `Scheduling::frame_budget` (16 ms by default), otherwise it waits for the next frame. Without this, a chain of immediately finishing tasks on the web would run entirely within one microtask checkpoint and the browser would never paint. Graph updates and explicit execution requests are never deferred.
- On recompile, `retain_sources` removes sources that no longer exist (aborting their tasks) and adds new ones at generation zero. Nodes that were removed drop their slots, which aborts their tasks.

The editor maps the top-level `Poll` to an `ExecutionResult`: `Pending`, `Value`, `Fallback(value, errors)`, `Error(errors)`, or `Panic`. A `Pending` result keeps the last frame and its errors on screen. An export treats `Partial` like `Pending`: it is parked and re-run on every wake after a bump until the result is final, and the runtime doesn't process other requests until then.

## Overhead

Three properties keep the status handling cheap: the stand-in and its error share a box, the status is a single enum instead of a value plus a separate state (which brings `Poll<u32>` from 48 down to 16 bytes), and the status functions are `#[inline(always)]`. A plain `#[inline]` hint loses to the inliner's cost model because of the drop glue in the error branches. This lets statically composed nodes fold the `Pending` checks and meets away. In the interpreter, a type-erased edge costs an indirect call, an input downcast, and one boxed output, but there is no boxed future per evaluation anymore.

# Drawbacks

- Everything downstream of an async node is computed twice: once with the placeholder and once with the real value. That is the price of progressive rendering, and `#[node(no_partial)]` avoids it where it isn't wanted.
- `'static` futures cannot evaluate their inputs lazily. Everything a future needs is evaluated and cloned before it is spawned.
- The migration is a single cutover. The macro, the registry, and the executor change together, so even though node bodies migrate mechanically, it lands as one large PR.
- Slot maps keep every finished result per context hash for as long as the node lives. Nothing evicts them yet.
- An async node without a placeholder serializes its requests inside nodes that evaluate a lazy input several times, like `repeat`. The first in-flight iteration returns `Pending`, the `?` leaves the loop, and the later iterations are only spawned after the earlier ones have finished, one re-render each. With a placeholder, every iteration returns `Partial` and all futures are spawned in the first frame.

# Rationale and alternatives

- **Inline pinned futures.** We could store each node's future unboxed and poll it once per graph evaluation. That needs soundness arguments for `Pin` through a `Mutex`, splits the trait in two, conflicts with `BorrowTree` moving nodes around, and still turns every graph evaluation into a polling iteration.
- **Poll-once compatibility shims** for existing `async fn` bodies. Real IO has to move into source nodes anyway, since a body that is polled once per frame would send its request again every frame. The remaining async bodies only awaited their inputs and are easy to migrate by hand.
- **Bail on `Partial` and `Fallback` too**, treating every non-final state as `Err`. This would remove progressive rendering and stand-in values, and a single error type carrying a value doesn't work across inputs of different types. It remains available per node as `no_partial`.
- **Wakers as completion signal.** A wake doesn't mean completion, wakers don't compose across host executors, and there is no suspended computation to resume.
- **Clearing downstream caches when a task finishes.** This needs the graph topology at runtime and extra logic to track caches. Putting the generation into the hash has the same effect, is set up entirely by the compiler pass, and works the same way for a precompiled graph.

# Unresolved questions

- Futures that publish intermediate results (streams, progressive image decoding) would want a weaker kind of invalidation than "finished".
- How slot maps should evict old entries, and whether memo nodes need more than one entry for contexts that alternate between evaluations.
- Spawned futures need `Send` on native targets but can't be `Send` on the web.
- Whether async nodes should require a placeholder, or whether looping nodes should evaluate all iterations before bailing on `Pending`, so that requests inside a loop are spawned in parallel.

# Future possibilities

- A borrowed context with a stack-based lifetime instead of the current `Arc` allocation.
- Type-erased edges with unboxed values. In the prototype, these cost 1.8ns per edge compared to roughly 9ns for the box-and-downcast model.
- Resolving error traces through rank promotion adapters inside the runtime instead of in the editor.
