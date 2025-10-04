# How the Svelte UI is Connected to the Rust Code in Graphite

The connection between Svelte and Rust in Graphite is achieved through **WebAssembly (WASM)** using **wasm-bindgen** as the bridge. This document explains the architecture, implementation, and communication flow between the frontend and backend.

## Architecture Overview

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Svelte UI     │◄──►│  WASM Bridge     │◄──►│  Rust Editor    │
│   (Frontend)    │    │  (wasm-bindgen)  │    │   (Backend)     │
└─────────────────┘    └──────────────────┘    └─────────────────┘
```

## WASM Bridge Layer

The bridge is implemented in `frontend/wasm/src/`:

- **`lib.rs`**: Main WASM entry point that initializes the editor backend
- **`editor_api.rs`**: Contains the `EditorHandle` struct with functions callable from JavaScript
- The Rust code is compiled to WASM using `wasm-pack`

## Build Process

The build process is defined in `frontend/package.json`:

```javascript
"wasm:build-dev": "wasm-pack build ./wasm --dev --target=web",
"start": "npm run wasm:build-dev && concurrently \"vite\" \"npm run wasm:watch-dev\""
```

The build flow:
1. **Rust → WASM**: `wasm-pack` compiles the Rust code in `frontend/wasm/` to WebAssembly
2. **WASM → JS bindings**: `wasm-bindgen` generates JavaScript bindings
3. **Svelte app**: Vite builds the Svelte frontend and imports the WASM module

## Connection Flow

### Initialization (`main.ts` → `App.svelte` → `editor.ts`)

```typescript
// frontend/src/editor.ts
export async function initWasm() {
    // Skip if the WASM module is already initialized
    if (wasmImport !== undefined) return;

    // Import the WASM module JS bindings
    const wasm = await init();
    wasmImport = await wasmMemory();
    
    // Set random seed for the Rust backend
    const randomSeed = BigInt(Math.floor(Math.random() * Number.MAX_SAFE_INTEGER));
    setRandomSeed(randomSeed);
}
```

### Editor Creation

```typescript
// frontend/src/editor.ts
export function createEditor(): Editor {
    const raw: WebAssembly.Memory = wasmImport;
    
    // Create the EditorHandle - this is the main bridge to Rust
    const handle: EditorHandle = new EditorHandle((messageType, messageData) => {
        // This callback handles messages FROM Rust TO JavaScript
        subscriptions.handleJsMessage(messageType, messageData, raw, handle);
    });
    
    return { raw, handle, subscriptions };
}
```

## Message-Based Communication

The communication uses a **bidirectional message system**:

### JavaScript → Rust (Function Calls)

JavaScript calls functions on the `EditorHandle` (defined in `editor_api.rs`):

```rust
// frontend/wasm/src/editor_api.rs
#[wasm_bindgen(js_name = onMouseMove)]
pub fn on_mouse_move(&self, x: f64, y: f64, mouse_keys: u8, modifiers: u8) {
    let editor_mouse_state = EditorMouseState::from_keys_and_editor_position(mouse_keys, (x, y).into());
    let modifier_keys = ModifierKeys::from_bits(modifiers).expect("Invalid modifier keys");
    let message = InputPreprocessorMessage::PointerMove { editor_mouse_state, modifier_keys };
    self.dispatch(message);  // Send to Rust backend
}
```

### Rust → JavaScript (Message System)

Rust sends `FrontendMessage`s back to JavaScript via the callback:

```rust
// Rust sends a message
self.send_frontend_message_to_js(FrontendMessage::UpdateDocumentArtwork { svg });
```

Messages are handled by the subscription router:

```typescript
// frontend/src/subscription-router.ts
const handleJsMessage = (messageType: JsMessageType, messageData: Record<string, unknown>) => {
    const messageMaker = messageMakers[messageType];
    const message = plainToInstance(messageMaker, messageData);
    const callback = subscriptions[message.constructor.name];
    callback(message);  // Call the registered Svelte handler
};
```

## State Management

Svelte components use **state providers** that subscribe to Rust messages:

```typescript
// frontend/src/components/Editor.svelte
// State provider systems
let dialog = createDialogState(editor);
let document = createDocumentState(editor);
let fonts = createFontsState(editor);
let fullscreen = createFullscreenState(editor);
let nodeGraph = createNodeGraphState(editor);
let portfolio = createPortfolioState(editor);
let appWindow = createAppWindowState(editor);
```

Each state provider:
- Subscribes to specific `FrontendMessage` types from Rust
- Updates Svelte stores when messages are received
- Provides reactive state to Svelte components

## Message Types and Transformation

Messages are defined in `frontend/src/messages.ts` using class-transformer:

```typescript
export class UpdateDocumentArtwork extends JsMessage {
    readonly svg!: string;
}

export class UpdateActiveDocument extends JsMessage {
    readonly documentId!: bigint;
}

export class Color {
    readonly red!: number;
    readonly green!: number;
    readonly blue!: number;
    readonly alpha!: number;
    readonly none!: boolean;
    
    // Methods for color conversion and manipulation
}
```

## Practical Example: Layer Selection

When a user clicks on a layer in the UI:

1. **Svelte**: User clicks layer → calls `editor.handle.selectLayer(id)`

2. **WASM Bridge**: JavaScript function maps to Rust `select_layer()`:
   ```rust
   #[wasm_bindgen(js_name = selectLayer)]
   pub fn select_layer(&self, id: u64, ctrl: bool, shift: bool) {
       let id = NodeId(id);
       let message = DocumentMessage::SelectLayer { id, ctrl, shift };
       self.dispatch(message);
   }
   ```

3. **Rust**: Processes selection → updates document state → sends `FrontendMessage::UpdateDocumentLayerDetails`

4. **WASM Bridge**: Message serialized and sent to JavaScript callback

5. **Svelte**: State provider receives message → updates reactive store → UI re-renders

## Key Files

### Frontend (TypeScript/Svelte)
- `frontend/src/main.ts` - Entry point
- `frontend/src/App.svelte` - Root Svelte component
- `frontend/src/editor.ts` - WASM initialization and editor creation
- `frontend/src/messages.ts` - Message type definitions
- `frontend/src/subscription-router.ts` - Message routing system
- `frontend/src/components/Editor.svelte` - Main editor component
- `frontend/src/state-providers/` - Reactive state management

### WASM Bridge (Rust)
- `frontend/wasm/src/lib.rs` - WASM module entry point
- `frontend/wasm/src/editor_api.rs` - Main API bridge with JavaScript-callable functions
- `frontend/wasm/Cargo.toml` - WASM module configuration

### Backend (Rust)
- `editor/` - Main editor backend implementation
- `editor/src/messages/` - Message handling system
- `node-graph/` - Node graph processing
- `libraries/` - Shared libraries

## Configuration Files

### Build Configuration
- `frontend/vite.config.ts` - Vite build configuration
- `frontend/package.json` - NPM dependencies and scripts
- `frontend/wasm/Cargo.toml` - WASM compilation settings
- `Cargo.toml` - Root workspace configuration

### Development
- `frontend/.gitignore` - Frontend-specific ignores
- `frontend/tsconfig.json` - TypeScript configuration
- `rustfmt.toml` - Rust formatting rules

## Key Benefits

- **Performance**: Core editor logic runs in compiled Rust (fast)
- **Memory Safety**: Rust prevents crashes and memory leaks  
- **Reactivity**: Svelte provides modern reactive UI
- **Type Safety**: Both ends are strongly typed with message contracts
- **Modularity**: Clear separation between UI and business logic
- **Hot Reload**: Development server supports hot reload for both Rust and Svelte changes

## Development Workflow

1. **Setup**: Run `npm run start` in `frontend/` directory
2. **WASM Build**: `wasm-pack` compiles Rust to WASM automatically
3. **Hot Reload**: Changes to Rust or Svelte code trigger automatic rebuilds
4. **Debugging**: Use browser dev tools for frontend, `log::debug!()` for Rust backend
5. **Testing**: Build with `cargo build` and test with browser

## Message Flow Diagram

```
User Interaction (Svelte)
         ↓
JavaScript Function Call
         ↓
EditorHandle Method (WASM Bridge)
         ↓
Rust Message Dispatch
         ↓
Editor Backend Processing
         ↓
FrontendMessage Generation
         ↓
WASM Serialization
         ↓
JavaScript Callback
         ↓
Subscription Router
         ↓
State Provider Update
         ↓
Svelte Store Update
         ↓
UI Re-render
```

This architecture enables Graphite to deliver a native-like performance experience in the browser while maintaining the benefits of modern web development practices with Svelte's reactive framework.