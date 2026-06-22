+++
title = "Debugging tips"

[extra]
order = 2 # Page number after chapter intro
css = ["/page/contributor-guide/bisect-tool.css"]
js = ["/js/page/contributor-guide/bisect-tool.js"]
+++

The Wasm-based editor has some unique limitations about how you are able to debug it. This page offers tips and best practices to get the most out of your problem-solving efforts.

## Comparing with deployed builds

When tracking down a bug, first check if the issue you are noticing also exists in `master` or just in your branch. Open up [dev.graphite.art](https://dev.graphite.art) which always deploys the lastest commit, as opposed to [editor.graphite.art](https://editor.graphite.art) which deploys the latest stable release. Build links for any commit may be found by clicking the "comment" icon on the right side of any commit in the GitHub repo [commits list](https://github.com/GraphiteEditor/Graphite/commits/master/).

Use *Help* > *About Graphite…* in the editor to view any build's Git commit hash.

Beware of a potential pitfall: all deploys and build links are built with release optimizations enabled. This means some bugs (like crashes from bounds checks or debug assertions) may exist in `master` and would appear if run locally, but not in the deployed version.

## Build bisect tool

```sh
# Access this quickly in the future:
cargo run explore bisect
```

This interactive tool helps you binary search through recent commits, test the build links of each, and pinpoint which change introduced a regression or added a feature.

<div class="bisect-tool">

<div class="phase active" data-phase="setup">
	<div class="setup-section">
		<div class="section-label">
			<span><strong>What are you looking for?</strong></span>
		</div>
		<label>
			<input type="radio" name="bisect-mode" value="regression" checked />
			<span>Find when a regression or bug started</span>
		</label>
		<label>
			<input type="radio" name="bisect-mode" value="feature" />
			<span>Find when a feature was added or fixed</span>
		</label>
	</div>
	<div class="setup-section">
		<div class="section-label">
			<span><strong>When do you estimate this changed?</strong></span>
		</div>
		<label>
			<input type="radio" name="start-method" value="date" checked />
			<span>Date</span>
		</label>
		<label>
			<input type="radio" name="start-method" value="hash" />
			<span>Commit</span>
		</label>
	</div>
	<div class="commit-inputs">
		<div class="start-input" data-input="date">
			<input type="date" data-commit-date />
		</div>
		<div class="start-input hidden" data-input="hash">
			<input data-commit-hash placeholder="Commit hash" pattern="[0-9a-fA-F]{7,40}" />
		</div>
		<span class="button arrow" data-start-button>Begin bisect</span>
	</div>
</div>

<div class="phase" data-phase="bisect">
	<div class="block feature-box-narrow">
		<div class="step-header">
			<span class="step-label" data-step-label><strong>Bisect step 1</strong></span>
			<span class="go-back hidden" data-go-back-button>(<a>go back</a>)</span>
		</div>
		<div class="progress-info" data-progress-info></div>
		<div class="commit-info" data-commit-info></div>
		<span class="button arrow" data-test-build-button>Test this build</span>
		<span class="findings">After testing, what have you found?</span>
		<div class="bisect-actions">
			<span class="button" data-issue-present-button></span>
			<span class="button" data-issue-absent-button></span>
		</div>
	</div>
</div>

<div class="error-message" data-message-box></div>

</div>

## Printing to the console

Use the browser console (<kbd>F12</kbd>) to check for warnings and errors. In Rust, use `log::debug!("The number is {some_number}");` to print to the browser console. These statements should be for temporary debugging. Remove them before your code is reviewed. Print-based debugging is necessary because breakpoints are not supported in WebAssembly.

Additional print statements are available that *should* be committed:

- `log::error!()` is for descriptive user-facing error messages arising from a bug
- `log::warn!()` is for non-critical problems that likely indicate a bug somewhere
- `log::trace!()` is for verbose logs of ordinary internal activity, hidden by default but viewable by activating *Help* > *Debug: Print Trace Logs*

## Message system logs

To also view logs of the messages dispatched by the message system, activate *Help* > *Debug: Print Messages* > *Only Names*. Or use *Full Contents* for a more verbose view containing the actual data being passed. This is an invaluable window into the activity of the message flow and works well together with `log::debug!()` printouts for tracking down message-related defects.

## Node/layer and document IDs

In debug mode, hover over a layer's name in the Layers panel, or a layer/node in the node graph, to view a tooltip with its ID. Likewise, document IDs may be read from their tab tooltips.

## Performance profiling

Be aware that having your browser's developer tools open will significantly impact performance in both debug and release builds, so it's best to close that when not in use.

The *Performance* tab of the browser developer tools lets you record and analyze performance profiles, and this is a useful way to track down bottlenecks. The Firefox profiler has some additional features missing from the Chromium debugger, so if you are digging deep into a performance issue, it can be worth giving Firefox a try for that purpose. Be sure to use debug builds while profiling, otherwise inlined functions and other optimizations may produce a misleading view of where time is being spent. The live deployed web app (production and dev) and build links hosted by our CI infrastructure are all built with release optimizations.
