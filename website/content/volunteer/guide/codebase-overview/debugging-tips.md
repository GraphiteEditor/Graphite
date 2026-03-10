+++
title = "Debugging tips"

[extra]
order = 2 # Page number after chapter intro
+++

The Wasm-based editor has some unique limitations about how you are able to debug it. This page offers tips and best practices to get the most out of your problem-solving efforts.

## Comparing with deployed builds

When tracking down a bug, first check if the issue you are noticing also exists in `master` or just in your branch. Open up [dev.graphite.art](https://dev.graphite.art) which always deploys the lastest commit, as opposed to [editor.graphite.art](https://editor.graphite.art) which deploys the latest stable release. Build links for any commit may be found by clicking the "comment" icon on the right side of any commit in the [GitHub repo commits list](https://github.com/GraphiteEditor/Graphite/commits/master/).

Use *Help* > *About Graphite* in the editor to view any build's Git commit hash.

Beware of one potential pitfall: all deploys and build links are built with release optimizations enabled. This means some bugs (like crashes from bounds checks or debug assertions) may exist in `master` and would appear if run locally, but not in the deployed version.

## Printing to the console

Use the browser console (<kbd>F12</kbd>) to check for warnings and errors. Use the Rust macro `debug!("The number is {}", some_number);` to print to the browser console. These statements should be for temporary debugging. Remove them before your code is reviewed. Print-based debugging is necessary because breakpoints are not supported in WebAssembly.

Additional print statements are available that *should* be committed:

- `error!()` is for descriptive user-facing error messages arising from a bug
- `warn!()` is for non-critical problems that likely indicate a bug somewhere
- `trace!()` is for verbose logs of ordinary internal activity, hidden by default but viewable by activating *Help* > *Debug: Print Trace Logs*

## Message system logs

To also view logs of the messages dispatched by the message system, activate *Help* > *Debug: Print Messages* > *Only Names*. Or use *Full Contents* for a more verbose view containing the actual data being passed. This is an invaluable window into the activity of the message flow and works well together with `debug!()` printouts for tracking down message-related defects.

## Node/layer and document IDs

In debug mode, hover over a layer's name in the Layers panel, or a layer/node in the node graph, to view a tooltip with its ID. Likewise, document IDs may be read from their tab tooltips.

## Performance profiling

Be aware that having your browser's developer tools open will significantly impact performance in both debug and release builds, so it's best to close that when not in use.

The *Performance* tab of the browser developer tools lets you record and analyze performance profiles, and this is a useful way to track down bottlenecks. The Firefox profiler has some additional features missing from the Chromium debugger, so if you are digging deep into a performance issue, it can be worth giving Firefox a try for that purpose. Be sure to use debug builds while profiling, otherwise inlined functions and other optimizations may produce a misleading view of where time is being spent. The live deployed web app (production and dev) and build links hosted by our CI infrastructure are all built with release optimizations.
