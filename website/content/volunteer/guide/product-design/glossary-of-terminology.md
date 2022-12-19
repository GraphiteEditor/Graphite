+++
title = "Glossary of terminology"

[extra]
order = 3 # Page number after chapter intro
+++

**NOTE: This is old. Some parts may not match current usage.**

### Document
A design source file created and edited in the Graphite editor. Saved to disk as a Graphite Design Document in a _GDD file_. Documents can be included as _layers_ inside other documents, and in doing so they take the form of _groups_. The _layer graph_ contents of a _group_ actually belong to the _embedded_ document's _subgraph_. Because a document is a _group_ which is a _layer_ in the _layer graph_, documents have _properties_ such as the _frames_ in the _canvas_. Documents are composed of a layer graph, a defined set of properties of set _data types_ that are _imported_ and _exported_, and the _properties_ of the _root layer_.
### Asset
A portable mechanism for distributing a "compiled" Graphite _document_ in a format that is immediately ready for rendering. Saved to disk as a Graphite Digital Asset in a _GDA file_. Assets are created by "flattening" a _document's_ complex, nested _layer graph_ structure into a single, simple directed acyclic graph (DAG). The Graphite editor internally maintains an asset version of any open _document_ in order to draw the _canvas_ live in the _viewport_. An asset also includes certain exposed _properties_ of specified _data types_ that are _imported_ and _exported_, as defined by the asset's author in the source _document's_ _layer graph_. They can be shared and _embedded_ in another _layer graph_ as a black box (meaning it can't be expanded to reveal or edit its interior graph), as compared to _embedded_ _documents_ from _GDD files_ which are white boxes (they can be expanded to reveal their _subgraph_ which can be edited). Assets are helpful for defining custom _nodes_ that perform some useful functionality. Tangible examples include custom procedural effects, shape generators, and image filters. Many of the Graphite editor's own built-in _nodes_ are actually assets rather than being implemented directly in code. The _Asset Manager_ panel helps maintain these assets from various sources. The _Asset Store_ can be used to share and sell assets for easy inclusion in other projects.
### GDD file
Graphite Design Document. A binary serialization of a _document_ source file. The format includes a chain of _operations_ that describe changes to the _layer graph_ and the _properties_ of _layers_ throughout the history of the document since its creation. It also stores certain metadata and the raw data of _embedded_ files. Because GDD files are editable (unlike _GDA files_), the _layers_ of GDD files imported into another _document_ may be expanded in its _layer graph_ to reveal and modify their contents using a copy-on-write scheme stored to the _asset's_ _layer_.
### GDA file
Graphite Digital Asset. A binary serialization of an _asset_ file. Because GDA files are read-only and can't be edited (unlike _GDD files_), the _layers_ created from _assets_ do not offer an ability to be expanded in the _layer graph_ of a _document_ that _embeds_ them. GDA files are useful for sharing _assets_ when their authors do not wish to provide the source _documents_ to author them. _DGA files_ are also the input format included in games that utilize the _Graphite Renderer Core Library_ to render graphical content at runtime, as well as similar applications like headless renderers on web servers and image processing pipelines.
### Window
### Main window
### Popout window
### Title bar
### Status bar
### Workspace
The part of the Graphite editor's UI that houses the _panels_ in a _window_. The workspace occupies the large space below the _title bar_ and above the _status bar_ of the _main window_. It occupies the entirety of _popout windows_ (window buttons are added in the _tab bar_).
### Workspace layout
The specific configuration of panels in the _main window_ and any _popout windows_. Workspace layout presets are provided by the Graphite editor and users may customize and save their own.
### Tab bar
The bar at the top of a _panel group_ which includes a clickable tab for each panel that is docked there. Each tab bar has at least one tab and one active tab.
### Active tab
The one tab in a _tab bar_ that is currently active. The user can click any inactive tab to make it become the active tab. The active tab shows the _panel content_ beneath it unless it is a _folded panel_.
### Folded panel
A shrunken _panel_ showing only the _tab bar_. A _panel_ consists of the _tab bar_ and _panel body_ except when the latter is folded away. The user may click the _active tab_ to fold and restore a panel, however a panel cannot be folded if there are no other unfolded panels in its column.
### Panel
### Panel body
### Options bar
The bar that spans horizontally across the top of a _panel_ (located under the _tab bar_) which displays options related to the _panel_.
### Viewport
The area that takes up the main space in a _panel_ (located beneath the _options bar_) which displays the primary content of the _panel_.
### Shelf
The bar that spans vertically along the left side of some _panels_ (located left of the _viewport_) which displays a catalog of available items, such as document editing _tools_ or common _nodes_.
### Tool
An instrument for interactively editing _documents_ through a collection of related behavior. Each tool puts the editor into a mode that provides the ability to perform certain _operations_ on the document interactively. Each _operation_ is run based on the current context of mouse and modifier buttons, key presses, tool options, selected layers, editor state, and document state. The _operations_ that get run are appended to the document history and update the underlying _layer graph_ in real time.
### Canvas
The infinite coordinate system that shows the visual output of an open _document_ at the current zoom level and pan position. It is drawn in the document panel's _viewport_ within the area inside the scroll bars on the bottom/right edges and the _rulers_ on the top/left edges. The canvas can be panned and zoomed in order to display all or part of the artwork in any _frames_. A canvas has a coordinate system spanning infinitely in all directions with an origin always located at the top left of the primary _artboard_. The purpose of an infinite canvas is to offer a convenient editing experience when there is no logical edge to the artwork, for example a loosely-arranged board of logo design concepts, a mood board, or whiteboard-style notes.
### Artboard
An area inside a _canvas_ that provides rectangular bounds to the artwork contained within, as well as default bounds for an exported image. The _Artboard tool_ adjusts the bounds and placement of frames in the _document_ and each artboard is stored in a "artboard list" property of the _root layer_. When there is at least one artboard, the infinite _canvas_ area outside any artboard displays a configurable background color. Artwork can be placed outside of a artboard but it will appear mostly transparent. The purpose of using one artboard is to provide convenient cropping to the edges of the artwork, such as a single digital painting or photograph. The purpose of using multiple frames is to work on related artwork with separate bounds, such as the layout for a book.
### Layer graph
A (directed acyclic) graph structure composed of _layers_ with _connections_ between their input and output _ports_. This is commonly referred to as a "node graph" in other software, but Graphite's layer graph is more suited towards layer-based compositing compared to traditional compositor node graphs.
### Node
A definition of a _layer_. A node is a graph "operation" or "function" that receives input and generates deterministic output.
### Layer
Any instance of a _node_ that lives in the _layer graph_. Layers (usually) take input data, then they transform it or synthesize new data, then they provide it as output. Layers have _properties_ as well as exposed input and output _ports_ for sending and receiving data.
### Root layer
### Group
### Raster
### Vector
### Mask
### Data type
### Subgraph
### Port
### Connection
### Core Libraries
### Graphite Editor (Frontend)
### Graphite Editor (Backend)
### Graphene (Node Graph Engine)
### Trace
### Path
### Shape
