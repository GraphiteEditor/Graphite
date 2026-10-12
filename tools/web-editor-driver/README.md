# Web editor driver

Drives the Graphite web editor the way a person does. Every action is real mouse or keyboard input delivered to the page, so nothing bypasses the interface, and what comes back is a screenshot of the whole editor. It serves three audiences:

- **Scripts** that reproduce a sequence of interactions.
- **QA** that exercises the web editor end to end and records what it looked like.
- **Agents** that operate the editor by looking at screenshots and choosing the next input.

The driver runs its own copy of Chromium, downloaded into this directory, with a fresh profile every session. It never touches a browser, profile, or history already on the machine.

It is written in Rust, apart from the small Node.js process in `playwright/` that delivers input and takes screenshots through Playwright. The commands, their results, and scenario files are defined by the editor control protocol in `editor/control-protocol`.

The TypeScript in `playwright/` follows the frontend's lint rules, using the tools the frontend installs. Check it with `npm run check` in that folder, and fix what can be fixed automatically with `npm run fix`.

## Setup

Requires Node.js 22.18 or newer.

```sh
cargo run drive install
```

On Nix use the shell `nix develop .#full`

## Usage

```sh
cargo run drive serve --port 8090    # Build the Wasm and serve the editor (omit if `cargo run` is already serving it)
cargo run drive start                # Open the editor in a new session
cargo run drive screenshot editor.png
cargo run drive key press KeyM       # Switch to the Rectangle tool
cargo run drive drag 200 150 500 350 --in viewport --shot rectangle.png
cargo run drive shutdown             # Close the session and the dev server
```

Run `cargo run drive help` for every command. Those that act on the editor print their results as JSON, and every command exits with a nonzero status if it failed. Every invocation goes through Cargo, so a batch with `act` or `run` is much faster than many separate commands.

Points are in CSS pixels from the page's top left corner, or from the document viewport's top left corner with `--in viewport`, and land on whole device pixels as a real pointer's do. Relative file paths are written to the `output` directory.

The `--keys` of a `click` or `drag` are held from before the button goes down, and `--hold` keeps them held along with the button. To press a key partway through a drag, use `drag --hold`, then `key down`, then `move`, then `up`. Combinations such as `Control+KeyZ` work with `key press`.

The session runs in the background and listens on a local socket, only accepting connections that open with the secret token in `.session/session.json`.

## Watching motion

A screenshot cannot show how an interaction felt partway through, and it leaves out the pointer. Two options cover that:

- `--cursor` on `screenshot` marks where the pointer is with a crosshair.
- `--filmstrip <file>` on `drag` captures frames along the drag and lays them out in one image, read left to right then top to bottom, each with its crosshair.

To look closely at a small area, limit either one with `--region viewport` or `--clip <x>,<y>,<width>,<height>`, and start the session with `--scale 2` for twice the pixel density.

## Scenarios

A scenario file holds the same commands the command line performs, one per line written as JSON, for replaying with `run`. Blank lines and lines starting with `//` are skipped, so a scenario can describe itself in its first line and group its steps:

```
// Draw a rectangle with the Rectangle tool

{ "type": "resize", "size": [1600, 1000] }
{ "type": "press", "key": "KeyM" }

// Drag out the rectangle, then capture the result
{ "type": "drag", "from": [200, 150], "to": [500, 350], "space": "viewport" }
{ "type": "waitIdle" }
{ "type": "screenshot", "path": "rectangle.png" }
```

Scenario files are named for what they do, ending in `.jsonl`, such as `draw-rectangle.jsonl`.

The commands and their fields are defined by `Command` in `editor/control-protocol/src/lib.rs`.

## QA

Scenarios check themselves with `expect` steps, which fail unless the page shows the expected elements. An `expect` finds elements as `locate` does, by a `data-*` attribute name, text they contain (ignoring case), or both, then narrows them to those with a field holding a `value`, such as a number field showing `"500.26"`. With only a `value`, it looks among every field on the page. It expects exactly `count` elements, or at least one without a count, so `"count": 0` checks that something is gone:

```
{ "type": "expect", "data": "layer", "count": 2 }
{ "type": "expect", "text": "Draw Rectangle" }
{ "type": "expect", "value": "257.30", "count": 2 }
```

Every result lists the errors the page logged since the previous command. When `run` performs a scenario, a step fails if any were logged, unless `run` is given `--allow-console-errors`.

Given a folder, `run` performs each `.jsonl` scenario file in it in a new session of its own, then prints how each went and exits with a nonzero status if any failed:

```sh
cargo run drive serve --port 8090
cargo run drive run tools/web-editor-driver/scenarios
```

The scenarios in `scenarios/` assume a page of 1600 by 1000 pixels, which their first step sets, since the values they expect depend on the zoom that size gives a new document.

The images a scenario saves to relative paths land in `output/`, or in another folder given to `run` with `--output`, such as one holding documentation screenshots. A folder's sessions open at a scale of 1, or another given with `--scale`, such as 2 for HiDPI screenshots.
