
# Incremental Compilation
Any time a NodeInput is changed in the network the following changes to the borrow tree are necessary:

1. Insert the new upstream nodes into the borrow tree based on their SNI
2. Remove upstream SNI's which are now orphaned 
3. Update the SNI of the downstream nodes (Not necessary if ghostcell is used and downstream nodes can have their inputs mutated)
4. Reset any downstream caches

We currently clone and recompile the entire network, which is very expensive. As networks grow larger and functionality gets split into separate nodes, a reasonable network may have hundreds of thousands of nodes. I believe with the following data structure and algorithm changes, it will be possible to implement incremental compilation, where a diff of changes is sent to the borrow tree. The overall goal of this method is the editor "damages" the node network by performing a series of set_input requests that invalidate downstream SNI's, and get queued in the compiler. Then, a compilation request iterates over the network, and uses the input requests, global cached compilation metadata,and NODE_REGISTRY to send a diff of updates to to the executor. These updates are `Add(SNI, (Constructor, Vec<SNI>))` and `Remove (SNI)` requests. In the future, GhostCell could be used to make a new `Modify((SNI, usize), SNI)` request, which remaps a nodes input to a new SharedNodeContainer.

# New Editor Workflow
Each document still stores a NodeNetwork, but this is used just for serialization. When a document is opened, the NodeNetwork gets moved into the compiler (within NodeId(0), the render node is NodeId(1), and stays the same). When the document is closed, the network within NodeId(0) is taken from the compiler and moved into the document. All changes such as add node, remove node, and set input get sent to the compiler. Then on a compilation request, the set input requests are applied, the NodeNetwork is modified, and the diff of changes is generated to be applied to the borrow tree. 

# Editor changes:
The editor now performs all compilation, type resolution, and stable node id generation.

```rust
struct SNI(u64);
type ProtonodeInput = (Vec<NodeId>, usize);
```
SNI represents the stable node id of a protonode, which is calculated based on the hash of the inputs SNI + implementation string, or the hash of the value. Protonode SNI's may become stale, in which case the counter in CompiledProtonodeMetadata is incremented

ProtonodeInput is the path to the protonode DocumentNode in the recursive NodeNetwork structure, as well as the input index.

```rust
PortfolioMessageHandler { 
    pub compiler_metadata: HashMap<SNI, CompilerMetadata>
}

// Used by the compiler and editor to send diff based changes to the runtime and cache the types so the constructor can be easily looked up
pub struct NodeNetworkCompiler {
    resolved_type: NodeIOTypes,
    // How many document nodes with this SNI exist. Could possibly be moved to the executor
    usages: usize,
    // The previously compiled NodeNetwork, which represents the current state of the borrow tree.
    network: NodeNetwork
    // A series of SetInput requests which are queued between compilations
    input_requests: HashMap<InputConnector, NodeInput>
}
```

The portfolio message handler stores a mapping of types for each SNI in the borrow tree. This is shared across documents, which also share a runtime. It represents which nodes already exist in the borrow tree, and thus can be replaced with ProtonodeEntry::Existing(SNI). It also stores the usages of each SNI. If it drops to zero after a compilation, then it is removed from the borrow tree.


```rust
DocumentNodeImplementation::ProtoNode(DocumentProtonode)

enum DocumentProtonode {
    stable_node_id: Option<SNI>,
    cache_output: bool,
    identifier: ProtonodeIdentifier,
    // Generated during compile time, and used to unload downstream SNI's which relied on this protonode
    callers: Vec<ProtonodePath>,
}

```

The editor protonode now stores its SNI. This is used to get its type information, thumbnail information, and other metadata during compilation. It is set during compilation. If it already exists, then compilation can skip any upstream nodes. This is similar to the MemoHash for value inputs, which is also changed to follow the same pattern for clarity.

Cache protonodes no longer exist, instead the protonode is annotated to insert a cache on its output. The current cache node can be replaced with an identity node with this field set to true. This field is set to true during compilation whenever the algorithm decides to add a cache node. Currently, this is whenever context nullification is added, but more advanced rules can be implemented. The overall goal is to add a cache node when a protonode has a high likely hood of being called multiple times with the same Context, takes a long time to evaluate, and returns a small amount of data. A similar algorithm could also be used to be determine when an input should be evaluated in another thread.

```rust
pub enum NodeInput {
 Value { ValueInput },
 ...
}

pub struct ValueInput {
    value: TaggedValue,
    stable_node_id: SNI,
    ...metadata
}
```
This is a simple rename to change the current memo hash to SNI, which makes it more clear that it is the same concept as for protonodes.

# Editor -> Runtime changes
These structs are used to transfer data from the editor to the runtime in CompilationRequest. Eventually the goal is move the entire NodeNetwork into the runtime, in which case the editor will send requests such as AddNode and SetInput. Then it will send queries to get data from the network.

```rust
pub struct RuntimeUpdate {
	// Topologically sorted
	nodes: Vec<ProtonodeUpdate>,
}
```
This represents the compiled proto network which is sent to the runtime and used to update borrow tree

```rust
pub enum ProtonodeUpdate {
  // A new SNI that does not exist in the borrow tree or in the ProtoNetwork
  NewProtonode(SNI, ConstructionArgs),
  // A protonode's SNI already exists in the protonetwork update
  Deduplicated,
  // Remove a node from the borrow tree when it has no callers
  Remove(SNI)
}
```

Used to transfer information from the editor to the runtime.

```rust
pub enum ConstructionArgs {
	/// A value of a type that is known, allowing serialization (serde::Deserialize is not object safe)
	Value(TaggedValue),
	Nodes(NodeConstructionArgs),
	/// Used for GPU computation to work around the limitations of rust-gpu.
	Inline(InlineRust),
}
```
Im not sure what Inline is

```rust
pub Struct NodeConstructionArgs {
 //used to get the constructor 
 pub type: NodeIOTypes,
 pub id: ProtonodeIdentifier,
 /// A mapping of each input to the SNI, used to get the upstream SharedNodeContainers for the constructor
 inputs: Vec<SNI>,
}
```
The goal of NodeConstruction args is to create the SharedNodeContainer that can be inserted into the borrow tree. It does this by using the id/types to get the implementation constructor, then using the vec of input SNI's to get references to the upstream inserted nodes.


## Runtime Changes
The runtime only contains the borrow tree, which has to have some way of iterating in topological order from any point.
```rust
pub struct BorrowTree {
	nodes: HashMap<SNI, (SharedNodeContainer,NodeConstructor)>,
}

The SNI is used to perform the correct updates/introspection to the borrow tree by the editor. It doesnt have to be correct, it just has to match the editor state.


# Full Compilation Process

Every time an input changes in the editor, it first performs a downstream traversal using the callers field in the protonode, and sets all Downstream SNI's to None. It also decrements the usages of that SNI by one. If it reaches 0, then it must no longer exist since there has been an upstream change, so its SNI must have changed. Save a vec of SNI to the protonetwork. It then performs an upstream traversal to create a topologically sorted ProtoNetwork. This would ideally be done non recursively, as networks are typically very deep.

The traversal starts at the root export, and gets the SNI of all inputs. gets the SNI of the upstream node. If it is None, then continue traversal. When coming up the callstack, compute the SNI based on the inputs and compute/save the type if it is not saved. Then, increment the usages by one.
If the type already exists, then add a Deduplicated entry to the network. Continue to the root.

Next, iterate over the old input and decrement all usages by one. If it reaches 0, push Remove(SNI) to the protonetwork. Finally, push the saved vec of downstream nodes to remove.

it is now sent to the runtime which inserts or removes the corresponding nodes.


Overall Benefits:
-Compilation is now performed in the editor, which means typing information/SNI generation is all moved into the editor, and nothing has to be returned.

- NodeNetwork doesnt have to be hashed/cloned for every compilation request. It doesnt have to be hashed, since every set compilation request is guaranteed to change the network.

- Much easier to keep runtime in sync and prevent unnecessary compilations since compilation requests don't have to be manually added

- The mirrored metadata structure in network interface can be removed, and all metadata can be stored in `DocumentNode`/`NodeInput`, as they are now editor only.

- Compilation only has to be performed on nodes which do not have an SNI, and can stop early if the SNI already exists in `compiler_metadata`

-Undo history is merged with the compiler functionality. Undoing a network reverts to the previously compiled network. We no longer have to manually add/start transactions, instead just 
