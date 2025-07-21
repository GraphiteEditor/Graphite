The editor has two types of state, persistent and UI state.

Persistent state is everything that will be synced between clients, saved in the file, and saved to the document file. This data is all stored in the NodeNetwork, which is loaded into the runtime when a file is opened.

UI state will be passed through the context in an `Arc<EditorMetadata>`. This allows the node graph to perform different operations based on the current state of the UI for different clients. For example, this can include returning data to the properties panel, or using the selection state to render a bbox overlay over a shape.

The runtime's responsibility is to take request from the editor, and return a list of Frontend UI updates, editor state updates, and a list of updates to the NodeNetwork itself. For example, consider rendering a properties panel. First, the node has to be selected. This is done within the layer node, which stores the click target in the local layer space, transforms the mouse position, and if it intersects it sends an EditorMessage to select that layer. It also uses the &NodeNetwork in the Context to iterate upstream from the node and get NodeIds for the upstream nodes. It uses these upstream IDs to send EditorMessages which add the upstream nodes to the render_properties `HashSet<NodeId>`. Like selection state, this is stored in the editor, cloned when making an evaluation request, and passed as a reference. Then, an editor message is added which requests that the network is evaluated, but from the properties panel generator node. This node uses the upstream inputs to send the properties panel widgets to the editor.

Since all persistent state is stored in the NodeNetwork, the undo/redo history can be implemented in the dynamic executor. The executor stores a hashmap representing the previous state for each node input that gets set between history steps, and a mapping of node ID to added/removed nodes. Then, when adding a history step, it pushes this list of changes into a vec. When undoing, a request is made which sets the network to this previous state, and pushes the previous inputs into the redo queue.

For example, the requests:
AddNode(1)
SetInput(1, Value::Int(1))

Then on a undo, this stored input/node is reapplied. This relies on the fact that setting an input is a reversable operation. It also means a CRDT is not necessary, since the client stores no stateful information regarding the node graph. Multiple clients make requests to the runtime, which is the single source of truth, and it returns data back to client based on the parameters passed in the request, which is used to evaluate the network.

