# Summary

Replace the boxed-future execution model with synchronous, poll-based evaluation. Nodes return a `GPoll<T>` state instead of futures. Async work runs to completion on an executor outside the graph; completion bumps a per-source generation counter which is injected into the context and hashed by memo nodes, so cache invalidation along the downstream cone is an ordinary cache miss. Asynchrony is modeled as a context feature, reusing the fine-grained context nullification pass to scope invalidation. All mechanisms described here have been validated in a standalone prototype; figures quoted below are measurements from it.

# Motivation

In the current model every node's `eval` returns a `DynFuture` (`Pin<Box<dyn Future>>`), even for trivial arithmetic, and the whole graph evaluation is one large future driven per render request. Costs:

- **Allocation per edge**: every eval boxes a future; pure nodes pay async machinery they never use.
- **No progressive rendering**: one slow async operation stalls the entire evaluation; there is no placeholder or partial result.
- **Wasted evaluations**: driving async work through graph re-evaluation costs a full traversal per poll.
- **Poor error semantics**: errors propagate as panics caught with `catch_unwind` at the root.
- **Blocks the precompiled future**: emitting the graph as Rust source running against a minimal runtime cannot be built on a box per edge.

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

The variants split into "has a value" (`Final`, `Partial`, `Fallback`) and "has no value" (`Pending`, `Error`). Value-carrying statuses flow through computation: a node receiving `Partial` computes normally and downgrades its own output; a node receiving `Fallback` computes on the stand-in and forwards the error. Valueless statuses stop the cone for this frame. Statuses combine with a meet (`Final ∧ Final = Final`, anything else drags the result down; `Error` dominates `Pending`).

`meet(Partial, Fallback)` collapses to `Fallback`, which alone would make an error from a still-incomplete source indistinguishable from a final document error. Finality therefore rides a separate `Finality` (`AllFinal ∧ Partial`) tracked beside the status rather than a sixth variant, so the executor can withhold provisional errors from the document without widening `GPoll`.

Layout: one tag plus one payload word for pointer-sized values (`GPoll<Arc<T>>` is 16 bytes); the stand-in and its error share a box, and `Error` boxes the error alone, so it crosses node boundaries without reallocating. Progress percentages are deliberately not part of the value: they change at high frequency and must not invalidate the dataflow, so they ride the monitor side channel.

`GraphError` records the path from the error source upward as a list of input indices (names are ambiguous; a node can have two inputs of the same type). Its kind is structured: `ErrorKind::Node(&'static str)` for node-declared failures versus operational failures such as `ErrorKind::ArenaExhausted`, which the executor answers by resetting or growing the arena and re-rendering, never by reporting a document error.

## Writing nodes

Node authors write plain Rust functions; the macro generates the node struct, the trait impl, and all status plumbing. The function must compile standalone; the generated eval calls it. Four return tiers:

- `-> T`: pure kernel over evaluated inputs. The macro meets input statuses; the author never sees them.
- `-> Result<T, Interrupt>`: for kernels that evaluate lazy inputs themselves or fail. `Interrupt = Pending | Error(Box<GraphError>)`, with `From<GraphError>`, so semantic errors and input bail-outs share the `?` operator.
- `-> Result<T, (T, GraphError)>`: an error with an explicit stand-in value.
- `-> GPoll<T>`: full manual control (memo-class nodes).

Lazy inputs (`content: impl Node<Output = T>`) expose `eval(ctx) -> Result<T, Interrupt>`. The status mapping is the meet in imperative form: `Final(v)` is `Ok(v)`; `Partial(v)` and `Fallback(v, e)` are also `Ok(v)`, recording partiality or the first error in a status cell owned by the generated eval (a stack-local threaded by reference into the input handles); `Pending` and `Error` are `Err(Interrupt::...)`, and the kernel bails with `?`, the only thing it could do without a value. After the kernel returns, the generated eval folds the cell into the result, and converts `Err(Interrupt)` back into `GPoll::Pending`/`Error`. Trace indices are pushed by the handles, which know their input position, never by author code.

Because only actually-evaluated inputs touch the cell, an untaken `switch` branch does not drag the status down. `#[node(no_partial)]`, for nodes where placeholder data is semantically nonsense, is a one-line change in the handle mapping: `Partial` maps to `Interrupt::Pending`. Errors produced while any input was non-`Final` are provisional and must not be reported as document errors.

A migrated node body changes mechanically: delete `async`, replace `.eval(ctx).await` with `.eval(ctx)?`, return `Result<T, Interrupt>`.

## Async nodes

An `async fn` node is an external source. It extracts and clones everything it needs, spawns a `'static` future onto the runtime, and immediately returns a placeholder:

```rs
#[node(placeholder = get_placeholder)]
async fn get(url: String) -> String { ... }

fn get_placeholder(url: &str) -> String {
    format!("Waiting for response on GET {url}")
}
```

The generated struct holds a slot map keyed by the context hash. On eval: completed slot, return `Final`; in-flight slot, return `Partial(placeholder)`; no slot, insert a marker, spawn, return the placeholder. The graph never suspends; every evaluation runs to completion and produces a frame. Async bodies cannot touch the context (the future must be `'static`); the generated code checks input statuses before spawning, so no request is issued from provisional data.

## Invalidation by generation

Each async node is a source identified by its stable node id (injected like monitor paths, so identity survives recompiles and deduplicated nodes share a source). The runtime maps source id to a shared generation counter; the spawn wrapper captures its own counter:

```rs
future.await;                       // write result into the node's slot
*generations.lock()                 // bump under the table lock; absent means the
    .get_mut(&source)? += 1;        // source was dropped, so the epilogue is inert
dirty.store(true, Release);         // request a re-render (coalesced)
```

The slot map is `Arc<Mutex<HashMap<ContextHash, Option<GPoll<T>>>>>`, cloned into the spawned future rather than borrowed from the node, so a task still in flight when `BorrowTree` rebuilds the node writes into live storage. `Option` carries the in-flight marker: absent is "never spawned", `None` is "spawned, not landed", `Some` is the landed value. The same mutex that guards the map publishes the write to the next `eval`, so neither the slot nor the generation needs a separate release/acquire protocol.

At the start of each evaluation the runtime injects a snapshot of `(SourceId, generation)` pairs into the root context, sorted by id so hashing is deterministic. Memos hash the context as usual, generations included, so after a bump exactly the memos downstream of that source miss, and every other branch hits. No cache-tracking logic, no runtime graph walking, identical behavior in the interpreter and a precompiled DAG.

The re-render typically runs with the same context except the bumped generation, so the async node finds its completed slot: a source's own generation must not be part of its slot key, which the compiler pass below guarantees.

## Asynchrony as a context feature

Which generations a subgraph observes is determined at compile time by the same branch analysis as context nullification; the feature domain is the set of upstream `SourceId`s instead of a fixed bitflag. Requirements propagate rootward (a node's set is the union of its children's, minus what it injects). At branch convergence a dependency mismatch inserts a context modification node whose payload carries a generation retain set alongside the feature bitflags: no new node kind, no separate pass. The edge into an async node retains only sources strictly upstream of it, which excludes its own generation from its slot key by construction.

## Caching

Memo nodes hash the (retain-filtered) context, generations included. `Final` and `Partial` are both cached; partials are safe because the finalizing bump changes the key. `Pending` and `Error` are never cached. Two memo nodes exist with different contracts. The frame memo is the cheap one the compiler inserts at nullification boundaries: it publishes a promoted span into the persistent arena region keyed on the lane-normalized context, hits serve straight out of the region until the region's epoch flush, and a flush makes an entry unreachable, never wrong, so compiler-inserted caching costs a re-publish rather than a deep copy. The explicit memoize node is two-tier: the same span fast path over deep copies that survive flushes, for content whose recomputation is what the author is paying to avoid. The arena split, promotion, and the laws they rest on are covered in the attribute RFC's arena section.

## Runtime

Two layers, so the runtime stays minimal and the executor swappable:

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

`GraphRuntime` wraps each task with the completion epilogue. The `Spawner` is tokio or a thread pool on desktop, `spawn_local` on wasm, or the host polling a task list in the precompiled case. The runtime reaches async nodes as a scope input (construction-time, not part of the context); a graph with zero async sources compiles to no runtime at all.

The execution path itself is synchronous: executor construction, update, and `execute` are plain fns, and hosts compile and evaluate without an async runtime of their own. All asynchrony lives in source kernels behind the spawner; `execute` surfaces the poll state (`GPoll`) and each host maps it at its boundary. The one genuine await is the GPU readback, which wasm cannot block on; sync hosts block on that future, with a dedicated device-poll thread driving completion.

Wakers are deliberately not the graph-level notification mechanism: wakers resume suspended computations and this graph never suspends; a re-evaluation is a fresh call. Wake also does not mean completion, and wakers do not survive composition across host executors. The host avoids busy-waiting with one coarse notification (park/unpark or event-loop message) shared by task wakes and the dirty epilogue.

Scheduling semantics: generations are read once per evaluation (completions landing mid-frame affect the next one); the dirty flag is drained once per frame (N completions, one re-render); on recompile, `retain_sources` drops removed sources from the table, so their epilogue still runs but finds no entry and bumps nothing, neither invalidating nor waking the host. Tasks are not cancelled; they are made inert.

# Reference-level explanation

## Node trait

```rs
pub trait Node<Input> {
    /// Serves the node's record through the caller's claim; the proof is
    /// mintable only by the claim's closing methods.
    fn serve<'e, 'l>(&self, input: &Input, slot: FrameClaim<'e, 'l>) -> GPoll<Served<'e>>;
    /// Rank polymorphism extension point, scalar (`Free`) by default.
    fn extent(&self, input: &Input, level: Level) -> GPoll<Extent> { ... }
    /// Batched evaluation; the default body is the per-lane spec loop.
    fn eval_batch<'a, 'e>(&'a self, input: &'a Input, range: Range<u64>,
                          scratch: Option<&'a mut [MaybeUninit<u64>]>,
                          frames: &Frames<'e>) -> BatchStatus<'a>
    where Input: InjectIndex + Copy + ExtractArena<ArenaRef = &'e Arena> { ... }
}
```

With the record model there is no `Output` associated type: every node
serves a record whose layout is wiring state, the element at offset 0,
and `serve` is the one required method. The caller mints the claim out
of its own frame space at the node's declared layout, so a served
record is of that layout by construction, and kernels evaluate their
inputs through generated handles that own the claims (the `eval` name
survives on those handles and on the batch path).

Three load-bearing shape decisions:

- **Plain `&self` receiver.** An `&'i self` receiver forces the forwarding impl `impl Node for &'i N`, tying the node borrow to the eval lifetime, which makes shared references unable to satisfy higher-ranked bounds. With `&self`, sharing, erasure, and stack-scoped derived contexts compose. The price: nodes cannot lend their own storage; all lending rides the eval lifetime through the arena carried in the context (a node-resident value costs one clone into the arena per arena generation).
- **No lifetime parameter on the trait.** With `&self`, any lifetime in the output can only come from the input type, so lending impls quantify over it: `impl<'e> Node<Context<'e>> for Lend { type Output = &'e T; }`. `GPoll` is fixed in the signature the way `Poll` is fixed in `Future`.
- **Input by reference.** `Input` is the owned context type; calls pass `&Context`. A modifying node keeps one mutable local, mutates it between calls (per-lane index stepping is one u64 store), and lends `&local` down; sound because inputs are second-class (never stored). This halved erased-edge cost versus by-value contexts (48 bytes of ABI traffic became one pointer).

The batching law: `serve` is the sole semantic interface, and any `eval_batch` override must be indistinguishable from the default per-lane loop; overrides are optimization, never semantics. `extent` reports a node's domain (`Free` for index-invariant, `Exactly(n)` for bounded), with uncertainty riding the `GPoll` status axis like any other value.

## Context

Contexts are borrow-based `Copy` values. Eval is synchronous, so context lifetime is strictly stack-shaped, and every axis lives where its modification frequency is natural. The production shape (48 bytes, built 2026-07-25 in `core-types/src/context.rs`):

```rs
struct ContextImpl<'e> {
    index: IndexLink<'e>,                   // per lane: inline head, mutable in place
    position: Option<&'e PositionLink<'e>>, // per push: frame-resident cons cell
    varargs: Option<&'e VarArgLink<'e>>,    // per scope: chain of &[DynSlot] links
    footprint: Option<&'e Footprint>,       // per transform: borrowed from the modifier's frame
    scope: &'e EvalScope<'e>,               // per frame/scope: timing, generations, arena, cached hash
}

struct IndexLink<'e> { index: u64, outer: Option<&'e IndexLink<'e>> }
```

Leveled axes are cons cells: the field holds the innermost level, `outer` chains through the frames of the structure nodes that pushed the enclosing levels. Only the index keeps its head inline (and non-optional; an unindexed evaluation reads as canonical lane 0), because it is the one per-lane-mutable axis: `set_index` is a branchless u64 store, which is what lets the trait's generic default `eval_batch` run without per-lane context rebuilds. Measured against a by-ref head, where every lane must materialize a fresh context because an erased callee may legally retain the pointer: 1.5 vs 3.6 ns/lane on the erased per-lane path, and an identical 0.11 ns/lane once the batch body is visible to the compiler (`benches/context_shape.rs`).

Derivation has exactly two moves. Replace copies the context and overwrites a field; the original leaves the chain, so its contribution vanishes with the copy standing in (no masking machinery). Push spills the current head to the pusher's frame and chains a new head onto it. Mixing both stacks a shadowed copy and a push, two frame values.

`EvalScope` groups everything scope-constant (timing, pointer position, the generation table, the arena) behind one pointer, together with a hash computed at construction. Modifiers of scope axes spill a new scope to their frame with the hash recomputed; in particular the retain filter stores only the filtered-generations hash, so contexts carry no retain slice and memo visits stop walking the generation list. The general discipline: hash at write time, mix at read time. Replace-not-compose remains sound because the nullification pass computes each edge's set absolutely. Context hashing must use a seeded fast hasher (zero-initialized fx absorbs leading zero words, which produced a real wrong-value memo hit in the prototype; hash-only memo keys make collisions correctness bugs). Hash values owe no compatibility to the owned context; the invariants are pinned by the property suite in `context.rs` (axis sensitivity, level order, axis-boundary disambiguation, retain scoping).

Varargs are frame-lent references (`DynSlot = &dyn AnyHash`), never boxes: the injecting node owns the values and lends a slice from its frame, and nested scope injections chain, concatenating innermost-first.

The trait never names this type. Capabilities are extract-style bounds on the generic input (`Ctx`, `ExtractIndex`, `InjectIndex`, `ExtractArena`), matching production's context traits. Borrow-returning extracts carry the eval lifetime through an associated type (`ExtractArena<ArenaRef = &'e Arena>`, written `ExtractArena<'e>` in the macro's sugar): a lifetime in an associated-type equality is a constrained impl parameter, while a trait lifetime parameter in a plain predicate is rejected (E0207), so generated impls stay generic over the input type even for lending nodes. The authoring rule is one sentence: a named lifetime in a node fn is the eval lifetime; it enters through an extract bound and exits through `&'e` in the signature. Everything else, including "nodes cannot lend their own storage", is ordinary borrow checking of the author's standalone function.

## Compiler pass

Extends `find_context_dependencies`. Per node, alongside the `ContextFeatures` bitflags, track the set of upstream source ids. The existing convergence rule triggers insertion of a retain filter (a `context_modification` node with a generation-retain payload) exactly as nullification nodes are inserted today. Infrastructure (memo, lend, clone_out, monitor) is inserted as ordinary proto nodes, exactly as main inserts memo and nullification nodes; their registry rows are generated from a central type census (one macro invocation over the supported-type list, TaggedValue-derived plus a hand-maintained remainder) rather than written per type by hand. The wire itself carries no behavior: an edge handle is a typed erased node plus its `Type`, and every constructor is an ordinary registry row.

## Generated code and overhead

Codegen requirements: the stand-in and error share a box, the status axis is flattened into `GPoll` (together: `GPoll<u32>` is 16 bytes instead of 48), and the status plumbing is `#[inline(always)]` (a plain hint loses to the inliner cost model on the error drop-glue branches). With these, a fused pure region compiles to native arithmetic plus a final tag store; a nine-node add/mul graph evaluates behind `#[inline(never)]` as four adds and two stores, with all `Pending` checks and status meets const-folded away. A fixed-output type-erased edge (`dyn for<'a> Node<Context<'a>, Output = u32>`) is one indirect tail-call with values unboxed; lending edges erase the same way. Measured on the wired prototype: erased edges 1.8ns versus ~9ns for the per-eval box-and-downcast model; batch lend paths reach the memory bandwidth floor; steady-state interpreted frames are allocation-free (nodes in a graph arena, contexts on the stack, values and scratch in the eval arena, memo hits lending).

# Drawbacks

- **Double computation on the provisional path**: the cone downstream of an async node computes once on the placeholder and again on the real value. Inherent to progressive rendering; `#[node(no_partial)]` is the escape hatch.
- **`'static` futures cannot lazily evaluate their inputs**: everything a future needs is evaluated and cloned before spawning.
- **Migration is a cutover**: the macro, registry, and executor flip together (see the integration plan); node bodies migrate mechanically but the landing is one large reviewed unit.
- **Slot and cache memory**: slot maps and LRU caches hold completed values per context hash; eviction policy needs tuning.

# Rationale and alternatives

- **Inline pinned futures** (store each node's future unboxed, poll once per graph evaluation): rejected; `Pin`-through-`Mutex` soundness arguments, a trait bifurcation, conflicts with `BorrowTree` moving nodes, and graph evaluations as polling iterations.
- **Poll-once compatibility shims** for existing `async fn` bodies: rejected in favor of the mechanical `?` edit; genuine IO must move to the source tier anyway (a poll-once body would silently re-issue requests every frame), and the cosmetic sites are few.
- **Bail on `Partial`/`Fallback`** (all statuses through `Err`): rejected as the default; it kills progressive rendering and stand-in propagation, and a value-carrying error type does not typecheck across inputs. It survives as the `no_partial` mapping.
- **`Cow`-shaped wire values** (each edge carries owned-or-borrowed): rejected by measurement, 2.27 to 5.08 ns/edge on the cheapest chain, a graph-wide tax versus coercion adapters that cost only where inserted.
- **Waker as completion signal**: wake does not mean completion, wakers do not compose across host executors, and there is no suspended continuation to resume.
- **Manual downstream cache clearing on completion**: needs runtime topology introspection and cache-tracking logic; generation-in-hash gets the same effect through the compiler pass and works unchanged precompiled.
- **Per-evaluation scope mutation for memo GC**: scope inputs are construction-time and must not be per-frame channels; bounded LRU needs no external signal.

# Unresolved questions

- Partial epochs: futures that publish intermediate data (streams, progressive decode) want a weaker invalidation than completion.
- Arena sizing and growth on exhaustion (grow-and-retry across frames); ownership and the reset policy landed with the two-arena split, transient reset per evaluation and persistent epoch flush after a refused reservation.
- Exact policy for provisional errors.
- `Send`/wasm: spawned futures need `Send` on native only; the arena's concurrency claims need loom before any multi-threaded executor.

# Future possibilities

- **Rank polymorphism via extents**: lists as index-functions with the index carried in the context, domains reported by `extent`, batching as the interpreter's amortization; reserved here by the defaulted `extent` method. One `Add` implementation then covers scalar-scalar, scalar-list, and list-list with no monomorphization, and broadcasting is cheap because index-nullification lets the scalar branch memoize once per domain.
- **Precompiled DAG emission**: node structs in topological order as Rust source; the runtime shrinks to the generation table, a spawner hook, and a dirty flag.
- **JIT of graph regions**: the remaining gap between chunked interpretation and monomorphized code is unfused loop passes, the JIT's job, not the ABI's.
