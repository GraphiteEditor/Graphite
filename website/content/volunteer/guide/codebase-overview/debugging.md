+++
title = "Debugging"

[extra]
order = 2 # Page number after chapter intro
+++

## Deployed builds

When tracking down a bug, first check if the issue you are noticing also exists in `master` or just your branch. Use [dev.graphite.rs](https://dev.graphite.rs) which should always deploy the lastest commit on `master`. By comparison, [editor.graphite.rs](https://editor.graphite.rs) is manually updated every few days or weeks to ensure stability. Use *Help* > *About Graphite* in the editor to view the build's [commit hash](https://github.com/GraphiteEditor/Graphite/commits/master).

## Printing to the console

Use the browser console (<kbd>F12</kbd>) to check for warnings and errors. Use the Rust macro `debug!("A debug message");` to print to the browser console. These statements should be for temporary debugging. Remove them before committing to `master`. Print-based debugging is necessary because breakpoints are not supported in WebAssembly.

Additional print statements are available that *should* be committed.

- `error!()` is for descriptive user-facing error messages arising from a bug
- `warn!()` is for non-critical problems that likely indicate a bug somewhere
- `trace!()` is for verbose logs of ordinary internal activity, hidden by default

To show `trace!()` logs, activate *Help* > *Debug: Print Trace Logs*.

## Message system logs

To also view logs of the messages dispatched by the message system, activate *Help* > *Debug: Print Messages* > *Only Names*. Or use *Full Contents* for more verbose insight with the actual data being passed. This is an invaluable window into the activity of the message flow and works well together with `debug!()` printouts for tracking down message-related issues.

## Node/layer and document IDs

In debug mode, hover over a layer's name in the Layers panel, or a layer/node in the node graph, to view a tooltip with its ID. Likewise, document IDs may be read by hovering over their tabs.
