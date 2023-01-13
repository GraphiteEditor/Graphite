import BezierDemo from "@/components/BezierDemo";
import BezierDemoPane from "@/components/BezierDemoPane";
import SubpathDemo from "@/components/SubpathDemo";
import SubpathDemoPane from "@/components/SubpathDemoPane";

import bezierFeatures, { BezierFeatureName } from "@/features/bezierFeatures";
import subpathFeatures, { SubpathFeatureName } from "@/features/subpathFeatures";

import "@/style.css";

window.document.title = "Bezier-rs Interactive Documentation";
window.document.body.innerHTML = `
<h1>Bezier-rs Interactive Documentation</h1>
<p>
	This is the interactive documentation for the <a href="https://crates.io/crates/bezier-rs"><b>Bezier-rs</b></a> library. View the
	<a href="https://docs.rs/bezier-rs/latest/bezier_rs">crate documentation</a>
	for detailed function descriptions and API usage. Click and drag on the endpoints of the demo curves to visualize the various Bezier utilities and functions.
</p>

<h2>Beziers</h2>
<div id="bezier-demos"></div>
<h2>Subpaths</h2>
<div id="subpath-demos"></div>
`.trim();

declare global {
	interface HTMLElementTagNameMap {
		"bezier-demo": BezierDemo;
		"bezier-demo-pane": BezierDemoPane;
		"subpath-demo": SubpathDemo;
		"subpath-demo-pane": SubpathDemoPane;
	}
}

window.customElements.define("bezier-demo", BezierDemo);
window.customElements.define("bezier-demo-pane", BezierDemoPane);
window.customElements.define("subpath-demo", SubpathDemo);
window.customElements.define("subpath-demo-pane", SubpathDemoPane);

const bezierDemos = document.getElementById("bezier-demos");
(Object.keys(bezierFeatures) as BezierFeatureName[]).forEach((featureName) => {
	const feature = bezierFeatures[featureName];
	const demo = document.createElement("bezier-demo-pane");

	demo.setAttribute("name", featureName);
	demo.setAttribute("demoOptions", JSON.stringify(feature.demoOptions || {}));
	demo.setAttribute("triggerOnMouseMove", String(feature.triggerOnMouseMove));
	demo.setAttribute("chooseComputeType", String(feature.chooseComputeType));
	bezierDemos?.append(demo);
});

const subpathDemos = document.getElementById("subpath-demos");
(Object.keys(subpathFeatures) as SubpathFeatureName[]).forEach((featureName) => {
	const feature = subpathFeatures[featureName];
	const demo = document.createElement("subpath-demo-pane");

	demo.setAttribute("name", featureName);
	demo.setAttribute("sliderOptions", JSON.stringify(feature.sliderOptions || []));
	demo.setAttribute("triggerOnMouseMove", String(feature.triggerOnMouseMove));
	demo.setAttribute("chooseComputeType", String(feature.chooseComputeType));
	subpathDemos?.append(demo);
});
