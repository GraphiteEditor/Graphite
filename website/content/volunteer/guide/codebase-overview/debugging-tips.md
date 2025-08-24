+++
title = "Debugging tips"

[extra]
order = 2 # Page number after chapter intro
+++

The Wasm-based editor has some unique limitations about how you are able to debug it. This page offers tips and best practices to get the most out of your problem-solving efforts.

## Comparing with deployed builds

When tracking down a bug, first check if the issue you are noticing also exists in `master` or just your branch. Open up [dev.graphite.rs](https://dev.graphite.rs) which always deploys the lastest commit, compared to [editor.graphite.rs](https://editor.graphite.rs) which is manually deployed from time to time for the sake of stability.

Use *Help* > *About Graphite* in the editor to view any build's Git commit hash.

Beware of one potential pitfall: all deploys and build links are built with release optimizations enabled. This means some bugs (like crashes from bounds checks or debug assertions) may exist in `master` and would appear if run locally, but not in the deployed version.

## Printing to the console

Use the browser console (<kbd>F12</kbd>) to check for warnings and errors. Use the Rust macro `debug!("The number is {}", some_number);` to print to the browser console. These statements should be for temporary debugging. Remove them before your code is reviewed. Print-based debugging is necessary because breakpoints are not supported in WebAssembly.

Additional print statements are available that *should* be committed.

- `error!()` is for descriptive user-facing error messages arising from a bug
- `warn!()` is for non-critical problems that likely indicate a bug somewhere
- `trace!()` is for verbose logs of ordinary internal activity, hidden by default

To show `trace!()` logs, activate *Help* > *Debug: Print Trace Logs*.

## Message system logs

To also view logs of the messages dispatched by the message system, activate *Help* > *Debug: Print Messages* > *Only Names*. Or use *Full Contents* for a more verbose view containing the actual data being passed. This is an invaluable window into the activity of the message flow and works well together with `debug!()` printouts for tracking down message-related defects.

## Node/layer and document IDs

In debug mode, hover over a layer's name in the Layers panel, or a layer/node in the node graph, to view a tooltip with its ID. Likewise, document IDs may be read from their tab tooltips.
