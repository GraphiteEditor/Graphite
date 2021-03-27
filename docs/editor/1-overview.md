# Overview

## Key concepts

TODO

## Glossary of terminology

TODO: Add more to make a comprehensive list, finish writing definitions, separate into categories, alphabetize

- Asset  
  A *GDD* or *GRD* file. Can be shared and *embedded* in another *layer graph*. Useful for providing custom *nodes* that perform some useful functionality. Tangible examples include custom procedural effects, shape generators, and image filters. Many of the Graphite editor's built-in *layers* are also assets that provide useful functionality through a group of nodes rather than being implemented directly in code. The *Asset Manager* panel helps maintain these assets from various sources. The *Asset Store* can be used to share and sell assets for easily inclusion in projects.
- Document  
  A design source file created and edited in the Graphite editor. When saved to disk as *GDD files* (Graphite Design Document), they are one of the two types of *assets*. Documents can be included as *layers* inside other documents, and in doing so they take the form of *groups*. The *layer graph* contents of a *group* actually belong to the *embedded* document's *subgraph*. Because a document is a *group* which is a *layer* in the *layer graph*, documents have *properties* such as the (optional) *pixel bounds* of the *canvas*. Documents are composed of a layer graph, a defined set of properties of set *data types* that are *imported* and *exported*, and the *properties* of the root *layer*.
- Render graph  
  A read-only "compiled" *document* in a format that is immediately ready for rendering. When saved to disk as *GRD files* (Graphite Render Data), they are one of the two types of *assets*. The Graphite editor internally maintains a render graph based on the open document in order to display it live in the *viewport*, but this can also be saved to disk for the purposes of sharing as an *asset*.
- GDD file  
  Graphite Design Document. A binary serialization of a *document* source file. The format includes a chain of *operations* that describe changes to the *layer graph* and the *properties* of *layers* throughout the history of the document since its creation. It also stores certain metadata and *embedded* file data. GDD files, along with *GRD files*, represent *assets* when shared. Because GDD files are editable (unlike *GRD files*), the *layers* of GDD *assets* may be expanded in the layer graph to reveal and modify their contents using a copy-on-write scheme stored to the *asset's* *layer*.
- GRD file  
  Graphite Render Data. A binary serialization of a *render graph* file. The format includes a single directed acyclic graph (DAG) compiled from the *layer graph* of a *document* as well as certain *properties* of set *data types* that are *imported* and *exported*. GRD files, along with *GDD files*, represent *assets* when shared. Because GRD files are read-only and can't be edited (unlike *GDD files*), the *layers* of GRD *assets* do not offer an ability to be expanded in the layer graph. GRD files are useful for sharing *assets* when their authors do not wish to provide the source *documents* used for their authoring. They are also the input format included in games that utilize the *Graphite Renderer Core Library* to render graphical content at runtime, as well as similar applications like headless renderers on web servers and image processing pipelines.
- Window
- Main window
- Popout window
- Title bar
- Status bar
- Workspace  
  The part of the Graphite editor's UI that houses the *panels* in a *window*. The workspace occupies the large space below the *title bar* and above the *status bar* of the *main window*. It occupies the entirety of *popout windows* (window buttons are added in the *tab bar*).
- Workspace layout  
  The specific configuration of panels in the *main window* and any *popout windows*. Workspace layout presets are provided by the Graphite editor and users may customize and save their own.
- Tab bar  
  The bar at the top of a *panel group* which includes a clickable tab for each panel that is docked there. Each tab bar has at least one tab and one active tab.
- Active tab  
  The one tab in a *tab bar* that is currently active. The user can click any inactive tab to make it become the active tab. The active tab shows the *panel content* beneath it unless it is a *folded panel*.
- Folded panel  
  A shrunken *panel* showing only the *tab bar*. A *panel* consists of the *tab bar* and *panel body* except when the latter is folded away. The user may click the *active tab* to fold and restore a panel, however a panel cannot be folded if there are no other unfolded panels in its column.
- Panel
- Panel body
- Panel content
- Editor
- Layer graph  
  A (directed acyclic) graph structure composed of *layers* with *connections* between their input and output *ports*. This is commonly referred to as a "node graph" in other software, but Graphite's layer graph is more suited towards layer-based compositing compared to traditional compositor node graphs.
- Layer  
  Any instance of a *node* that lives in the *layer graph*. Layers (usually) take input data, then they transform it or synthesize new data, then they provide it as output. Layers have *properties* as well as exposed input and output *ports* for sending and receiving data.
- Node  
  A definition of a 
- Group
- Raster
- Vector
- Mask
- Data type
- Subgraph
- Port
- Connection
- Pixel bounds
- Canvas
- Core Libraries
- Editor Core Library
- Document Core Library
- Renderer Core Library
- Trace
- Path
- Shape