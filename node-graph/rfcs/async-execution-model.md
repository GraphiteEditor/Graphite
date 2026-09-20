# Summary

Replace the boxed-future execution model with synchronous, poll-based evaluation. Nodes return a `GPoll<T>` state instead of a future. Async work runs to completion on an executor outside the graph. When it finishes, it bumps a per-source generation counter; the counter is injected into the context and hashed by memo nodes, so invalidating the downstream cone is an ordinary cache miss. Asynchrony becomes a context feature, and we reuse the fine-grained context nullification pass to scope the invalidation.

# Motivation

Right now every node's `eval` returns a `DynFuture` (`Pin<Box<dyn Future>>`), even for trivial arithmetic, and a whole graph evaluation is one large future that we drive once per render request. This costs us in several places:

- Every eval boxes a future, so pure nodes pay for async machinery they never use.
- There is no progressive rendering. One slow async operation stalls the entire evaluation, and there is no placeholder or partial result to show in the meantime.
- Driving async work through graph re-evaluation costs a full traversal per poll.
- Errors propagate as panics that we catch with `catch_unwind` at the root.
- Emitting the graph as Rust source against a minimal runtime cannot be built on a box per edge, so the current model blocks the precompiled future.

# Guide-level explanation

## Evaluation states

Node evaluation is synchronous and always returns immediately:

```rs
enum GPoll<T> {
    /// Waiting on at least one future, with no placeholder to compute with.
    Pending,
    Final(T),
    /// Computed from provisional data; recomputed once the source generation bumps.
    Partial(T),
    /// An error occurred; the boxed value is a stand-in, downstream keeps computing.
    Fallback(Box<(T, GraphError)>),
    /// An error occurred and no stand-in exists; the cone cannot compute this frame.
    Error(Box<GraphError>),
}
```

The variants split into "has a value" (`Final`, `Partial`, `Fallback`) and "has no value" (`Pending`, `Error`). Value-carrying statuses flow through computation: a node that receives `Partial` computes normally and downgrades its own output, and a node that receives `Fallback` computes on the stand-in and forwards the error. Valueless statuses stop the cone for this frame. Statuses combine with a meet: `Final ∧ Final = Final`, anything else drags the result down, and `Error` dominates `Pending`.

There is one wrinkle. `meet(Partial, Fallback)` collapses to `Fallback`, which on its own would make an error from a still-incomplete source look the same as a final document error. So finality is tracked separately as a `Finality` (`AllFinal ∧ Partial`) beside the status rather than as a sixth variant. The executor can then hold back provisional errors from the document without widening `GPoll`.

Layout is one tag plus one payload word for pointer-sized values (`GPoll<Arc<T>>` is 16 bytes). The stand-in and its error share a box, and `Error` boxes the error alone, so a value crosses node boundaries without reallocating. Progress percentages are not part of the value: they change at high frequency and must not invalidate the dataflow, so they go over the monitor side channel instead.

`GraphError` records the path from the error source upward as a list of input indices. Names would be ambiguous, since a node can have two inputs of the same type. Its kind is structured: `ErrorKind::Node(&'static str)` for node-declared failures, separate from operational failures that the executor handles itself rather than reporting as document errors.

## Writing nodes

Node authors write plain Rust functions. The macro generates the node struct, the trait impl, and all the status plumbing. Four return tiers are supported:

- `-> T`: a pure kernel over evaluated inputs. The macro meets the input statuses and the author never sees them.
- `-> Result<T, Interrupt>`: for kernels that evaluate lazy inputs themselves or that can fail. `Interrupt = Pending | Error(Box<GraphError>)`, with `From<GraphError>`, so semantic errors and input bail-outs share the `?` operator.
- `-> Result<T, (T, GraphError)>`: an error with an explicit stand-in value.
- `-> GPoll<T>`: full manual control.

Lazy inputs (`content: impl Node<Output = T>`) expose `eval(ctx) -> Result<T, Interrupt>`. The status mapping is the meet written imperatively. `Final(v)` is `Ok(v)`. `Partial(v)` and `Fallback(v, e)` are also `Ok(v)`, but they record partiality or the first error in a status cell owned by the generated eval (a stack-local threaded by reference into the input handles). `Pending` and `Error` are `Err(Interrupt::...)`, and the kernel bails with `?`, which is the only thing it could do without a value anyway. After the kernel returns, the generated eval folds the cell into the result and converts `Err(Interrupt)` back into `GPoll::Pending` or `GPoll::Error`. Trace indices are pushed by the handles, which know their input position, so author code does not touch them.

Because only inputs that were actually evaluated touch the cell, an untaken `switch` branch does not drag the status down. For nodes where placeholder data makes no sense, `#[node(no_partial)]` changes one line in the handle mapping: `Partial` maps to `Interrupt::Pending`. Errors produced while any input was non-`Final` are provisional and must not be reported as document errors.

Migrating an existing non-async node body is mechanical: delete `async`, replace `.eval(ctx).await` with `.eval(ctx)?`, return `Result<T, Interrupt>`.

## Async nodes

An `async fn` node is an external source. It extracts and clones everything it needs, spawns a `'static` future onto the runtime, and immediately returns a placeholder:

```rs
#[node(placeholder = get_placeholder)]
async fn get(url: String) -> String { ... }

fn get_placeholder(url: &str) -> String {
    format!("Waiting for response on GET {url}")
}
```

The generated struct holds a slot map keyed by the context hash. On eval: a completed slot returns `Final`; an in-flight slot returns `Partial(placeholder)`; no slot means insert a marker, spawn, and return the placeholder. The graph never suspends, so every evaluation runs to completion and produces a frame. Async bodies cannot touch the context because the future has to be `'static`. The generated code checks the input statuses before spawning, so no request goes out based on provisional data.

## Invalidation by generation

Each async node is a source identified by its stable node id. The id is injected like monitor paths are, so identity survives recompiles and deduplicated nodes share a source. The runtime maps source id to a shared generation counter, and the spawn wrapper captures its own counter:

```rs
future.await;                       // write result into the node's slot
*generations.lock()                 // bump under the table lock; absent means the
    .get_mut(&source)? += 1;        // source was dropped, so the epilogue is inert
dirty.store(true, Release);         // request a re-render (coalesced)
```

The slot map is `Arc<Mutex<HashMap<ContextHash, Option<GPoll<T>>>>>`. It is cloned into the spawned future rather than borrowed from the node, so a task that is still in flight when `BorrowTree` rebuilds the node writes into live storage. The `Option` is the in-flight marker: absent is "never spawned", `None` is "spawned, not landed", `Some` is the landed value. The mutex that guards the map also publishes the write to the next `eval`, so neither the slot nor the generation needs its own release/acquire protocol.

At the start of each evaluation the runtime injects a snapshot of `(SourceId, generation)` pairs into the root context, sorted by id so hashing is deterministic. Memos hash the context as usual, generations included. After a bump, exactly the memos downstream of that source miss and every other branch hits. We need no cache-tracking logic and no runtime graph walking for this, and the interpreter and a precompiled DAG behave the same way.

The re-render typically runs with the same context except for the bumped generation, so the async node finds its completed slot. For that to work, a source's own generation must not be part of its slot key; the compiler pass below guarantees this.

## Asynchrony as a context feature

Which generations a subgraph observes is determined at compile time by the same branch analysis as context nullification, except the feature domain is the set of upstream `SourceId`s instead of a fixed bitflag. Requirements propagate rootward: a node's set is the union of its children's, minus whatever it injects itself. At branch convergence, a dependency mismatch inserts a context modification node whose payload carries a generation retain set next to the feature bitflags. There is no new node kind and no separate pass. The edge into an async node retains only sources strictly upstream of it, which is what keeps its own generation out of its slot key.

## Caching

Memo nodes hash the retain-filtered context, generations included. `Final` and `Partial` are both cached; caching partials is safe because the finalizing bump changes the key. `Pending` and `Error` are never cached. The memo owns its entries and is a bounded LRU, because contexts that change every evaluation would otherwise grow it without bound.

## Runtime

Two layers, so the runtime stays small and the executor can be swapped:

```rs
/// Node-facing scope input.
trait Runtime { fn spawn(&self, source: SourceId, future: BoxFuture); }

/// Host-provided task executor with no graph knowledge.
trait Spawner { fn spawn(&self, task: BoxFuture); }

struct GraphRuntime<S: Spawner> {
    generations: Arc<Mutex<HashMap<SourceId, u64>>>,
    dirty: Arc<AtomicBool>,
    spawner: S,
}
```

`GraphRuntime` wraps each task with the completion epilogue. The `Spawner` is tokio or a thread pool on desktop, `spawn_local` on wasm, or the host polling a task list in the precompiled case. The runtime reaches async nodes as a scope input (fixed at construction time, not part of the context), and a graph with zero async sources compiles to no runtime at all.

The execution path itself is synchronous. Executor construction, update, and `execute` are plain fns, and hosts compile and evaluate without an async runtime of their own. All asynchrony lives in source kernels behind the spawner; `execute` surfaces the poll state (`GPoll`) and each host maps it at its boundary. The one genuine await is the GPU readback, which wasm cannot block on. Sync hosts block on that future, with a dedicated device-poll thread driving completion.

Wakers are not the graph-level notification mechanism. A waker resumes a suspended computation, and this graph never suspends; a re-evaluation is a fresh call. A wake also does not mean completion, and wakers do not survive composition across host executors. To avoid busy-waiting, the host uses one coarse notification (park/unpark or an event-loop message) shared by task wakes and the dirty epilogue.

Scheduling semantics: generations are read once per evaluation, so completions that land mid-frame affect the next one. The dirty flag is drained once per frame, so N completions produce one re-render. On recompile, `retain_sources` drops removed sources from the table; their epilogue still runs, but it finds no entry and bumps nothing, so it neither invalidates nor wakes the host. We don't cancel tasks, we just let them become inert.

# Reference-level explanation

## Node trait

```rs
pub trait Node<Input> {
    type Output;
    fn eval(&self, input: &Input) -> GPoll<Self::Output>;
}
```

Three decisions about the shape of this trait carry most of the design:

- **Plain `&self` receiver.** An `&'i self` receiver forces the forwarding impl `impl Node for &'i N`, which ties the node borrow to the eval lifetime and makes shared references unable to satisfy higher-ranked bounds. With `&self`, sharing and erasure compose. What we give up is that nodes cannot lend their own storage; values cross edges owned.
- **No lifetime parameter on the trait.** With `&self`, any lifetime in the output can only come from the input type, so nothing on the trait itself needs one. `GPoll` is fixed in the signature the same way `Poll` is fixed in `Future`.
- **Input by reference.** `Input` is the owned context type and calls pass `&Context`. A modifying node keeps one mutable local, mutates it between calls, and lends `&local` down. This is sound because inputs are second-class and never stored. It halved the erased-edge cost compared to by-value contexts: 48 bytes of ABI traffic became one pointer.

## Compiler pass

This extends `find_context_dependencies`. Per node, alongside the `ContextFeatures` bitflags, we track the set of upstream source ids. The existing convergence rule triggers insertion of a retain filter (a `context_modification` node with a generation-retain payload) the same way nullification nodes are inserted today. Infrastructure nodes (memo, monitor) are inserted as ordinary proto nodes, as main already does for memo and nullification. Their registry rows are generated from a central type census, one macro invocation over the supported-type list (TaggedValue-derived plus a hand-maintained remainder), instead of being written per type by hand. The wire itself carries no behavior: an edge handle is a typed erased node plus its `Type`, and every constructor is an ordinary registry row.

## Generated code and overhead

The codegen has three requirements: the stand-in and error share a box, the status axis is flattened into `GPoll` (together these bring `GPoll<u32>` from 48 bytes down to 16), and the status plumbing is `#[inline(always)]`, because a plain hint loses to the inliner cost model on the error drop-glue branches. With these in place, a fused pure region compiles to native arithmetic plus a final tag store. A nine-node add/mul graph evaluates behind `#[inline(never)]` as four adds and two stores, with all `Pending` checks and status meets const-folded away. A fixed-output type-erased edge (`dyn Node<Context, Output = u32>`) is one indirect tail-call with values unboxed.

Measured on the wired prototype: erased edges cost 1.8ns versus roughly 9ns for the per-eval box-and-downcast model.

# Drawbacks

- The cone downstream of an async node computes twice on the provisional path: once on the placeholder and again on the real value. This comes with progressive rendering; `#[node(no_partial)]` is the escape hatch.
- `'static` futures cannot lazily evaluate their inputs. Everything a future needs is evaluated and cloned before spawning.
- Migration is a cutover. The macro, registry, and executor flip together; node bodies migrate mechanically, but the landing is one large reviewed unit.
- Slot maps and LRU caches hold completed values per context hash. The eviction policy will need tuning.

# Rationale and alternatives

- **Inline pinned futures** (store each node's future unboxed, poll once per graph evaluation): rejected. It needs `Pin`-through-`Mutex` soundness arguments, bifurcates the trait, conflicts with `BorrowTree` moving nodes, and turns graph evaluations into polling iterations.
- **Poll-once compatibility shims** for existing `async fn` bodies: rejected in favor of the mechanical `?` edit. Genuine IO has to move to the source tier anyway (a poll-once body would silently re-issue requests every frame), and the cosmetic sites are few.
- **Bail on `Partial`/`Fallback`** (all statuses through `Err`): rejected as the default. It kills progressive rendering and stand-in propagation, and a value-carrying error type does not typecheck across inputs. It survives as the `no_partial` mapping.
- **`Cow`-shaped wire values** (each edge carries owned-or-borrowed): rejected by measurement. 2.27 to 5.08 ns/edge on the cheapest chain is a graph-wide tax, whereas coercion adapters only cost where they are inserted.
- **Waker as completion signal**: a wake does not mean completion, wakers do not compose across host executors, and there is no suspended continuation to resume.
- **Manual downstream cache clearing on completion**: needs runtime topology introspection and cache-tracking logic. Generation-in-hash gets the same effect through the compiler pass and works unchanged when precompiled.
- **Per-evaluation scope mutation for memo GC**: scope inputs are construction-time and should not become per-frame channels. A bounded LRU needs no external signal.

# Unresolved questions

- Partial epochs: futures that publish intermediate data (streams, progressive decode) want a weaker invalidation than completion.
- LRU capacity policy, and the exact policy for provisional errors.
- `Send`/wasm: spawned futures need `Send` on native only.

# Future possibilities

- Borrowed context with a stack-based lifetime instead of the current `Arc<>` allocation.
