+++
title = "Editor structure"

[extra]
order = 1 # Page number after chapter intro
css = ["/page/developer-guide-editor-structure.css"]
js = ["/js/developer-guide-editor-structure.js"]
+++

The Graphite editor is the application users interact with to create documents. Its code is one Rust crate sandwiched between the frontend and Graphene, the node-based graphics engine. The main business logic of all visual editing is handled by the editor backend. When running in the browser, it is compiled to WebAssembly and passes messages to the frontend.

## Message system

The Graphite editor backend is organized into a hierarchy of subsystems which talk to one another through message passing. Messages are pushed to the front or back of a queue and each one is processed sequentially by the editor's dispatcher.

The dispatcher lives at the root of the editor hierarchy and acts as the owner of all its top-level message handlers. This satisfies Rust's restrictions on mutable borrows because only the dispatcher may mutate its message handlers, one at a time, while each message is processed.

## Editor outline

Click to explore the outline of the editor subsystem hierarchy which forms the structure of the editor's subsystems, state, and interactions. Bookmark this page to reference it later.

<div class="structure-outline">
<!-- replacements::hierarchical_message_system_tree() -->
</div>

### Parts of the hierarchy

<span class="subsystem">Subsystem components</span>

- A <span class="subsystem">*Message</span> enum is the component of an editor subsystem that defines its message interfaces as enum variants. Messages are used for passing a request from anywhere in the application, optionally with some included data, to have a particular block of code be run by its respective message handler.

- A <span class="subsystem">*MessageHandler</span> struct is the component of an editor subsystem that has ownership over its persistent editor state and its child message handlers for the lifetime of the application. It also defines the logic for handling each of its messages that it receives from the dispatcher. Those blocks of logic may further enqueue additional messages to be processed by itself or other message handlers during the same dispatch cycle.

- A <span class="subsystem">*MessageContext</span> struct is the component of an editor subsystem that defines what data is made available from other subsystems when running the logic to handle a dispatched message. It is a struct that is passed to the message handler when processing a message, and it gets filled in with data (owned, borrowed, or mutably borrowed) from its parent message handler. Intermediate subsystem layers may forward data from their parent to their child contexts to make state available from further up the hierarchy.

<span class="submessage">Sub-messages</span>

- A <span class="submessage">#[child] *</span> attribute-decorated message enum variant is a special kind of message that encapsulates a nested subsystem. As with all messages, its handler has a manually written code block. But that code must call its corresponding child message handler's `process_message` method. The child message handler is a field of this parent message handler's state struct.

`Messages`

- A `*` message enum variant is used throughout the editor to request that a certain subsystem performs some action, potentially given some data. In that sense, it resembles a function call, but a key difference is that messages are queued up and processed sequentially in a flat order, always invoked by the dispatcher.

## How messages work

Messages are enum variants that are dispatched to perform some intended activity within their respective message handlers. Here are two <span class="subsystem">DocumentMessage</span> definitions:
```rs
pub enum DocumentMessage {
	...
	// A message that carries one named data field
	DeleteLayer {
		id: NodeId,
	}
	// A message that carries no data
	DeleteSelectedLayers,
	...
}
```

As shown above, additional data fields can be included with each message. But as a special case denoted by the <span class="submessage">#[child]</span> attribute, that data can also be a sub-message enum, which enables hierarchical nesting of message handler subsystems.

By convention, regular data must be written as struct-style named fields (shown above), while a sub-message enum must be written as a tuple/newtype-style field (shown below). The <span class="subsystem">DocumentMessage</span> enum of the previous example is defined as a child of <span class="subsystem">PortfolioMessage</span> which wraps it like this:

```rs
pub enum PortfolioMessage {
	...
	// A message that carries the `DocumentMessage` child enum as data
	#[child]
	Document(DocumentMessage),
	...
}
```

Likewise, the <span class="subsystem">PortfolioMessage</span> enum is wrapped by the top-level <span class="subsystem">Message</span> enum. The dispatcher operates on the queue of these base-level <span class="subsystem">Message</span> types.

So for example, the `DeleteSelectedLayers` message mentioned previously will look like this as a <span class="subsystem">Message</span> data type:

```rs
Message::Portfolio(
	PortfolioMessage::Document(
		DocumentMessage::DeleteSelectedLayers
	)
)
```

Writing out these nested message enum variants would be cumbersome, so that <span class="submessage">#[child]</span> attribute shown earlier invokes a proc macro that automatically implements the `From` trait, letting you write this instead to get a <span class="subsystem">Message</span> data type:

```rs
DocumentMessage::DeleteSelectedLayers.into()
```

Most often, this is simplified even further because the `.into()` is called for you when pushing a message to the queue with `.add()` or `.add_front()`. So this becomes as simple as:

```rs
responses.add(DocumentMessage::DeleteSelectedLayers);
```

The `responses` message queue is composed of <span class="subsystem">Message</span> data types, and thanks to this system, child messages like `DocumentMessage::DeleteSelectedLayers` are automatically wrapped in their ancestor enum variants to become a <span class="subsystem">Message</span>, saving you from writing the verbose nested form.
