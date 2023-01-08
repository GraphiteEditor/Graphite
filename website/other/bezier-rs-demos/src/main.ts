import BezierExample from "@/components/BezierExample";
import BezierExamplePane from "@/components/BezierExamplePane";
import SubpathExample from "@/components/SubpathExample";
import SubpathExamplePane from "@/components/SubpathExamplePane";

import bezierFeatures, { BezierFeatureName } from "@/features/bezierFeatures";
import subpathFeatures, { SubpathFeatureName } from "@/features/subpathFeatures";

import "@/style.css";

document.title = "Bezier-rs Interactive Documentation";

window.customElements.define("bezier-example", BezierExample);
window.customElements.define("bezier-example-pane", BezierExamplePane);
window.customElements.define("subpath-example", SubpathExample);
window.customElements.define("subpath-example-pane", SubpathExamplePane);

const App = document.getElementById("app");
if (App) {
	App.innerHTML = `<h1>Bezier-rs Interactive Documentation</h1>
  <p>
    This is the interactive documentation for the <a href="https://crates.io/crates/bezier-rs"><b>Bezier-rs</b></a> library. View the
    <a href="https://docs.rs/bezier-rs/latest/bezier_rs">crate documentation</a>
    for detailed function descriptions and API usage. Click and drag on the endpoints of the example curves to visualize the various Bezier utilities and functions.
  </p>

  <h2>Beziers</h2>
  <div id="bezier-examples"></div>
  <h2>Subpaths</h2>
  <div id="subpath-examples"></div>`;
}
const bezierExamples = document.getElementById("bezier-examples");
(Object.keys(bezierFeatures) as BezierFeatureName[]).forEach((featureName) => {
	const feature = bezierFeatures[featureName];
	const example = document.createElement("bezier-example-pane");

	example.setAttribute("name", featureName);
	example.setAttribute("exampleOptions", JSON.stringify(feature.exampleOptions || {}));
	example.setAttribute("triggerOnMouseMove", String(feature.triggerOnMouseMove));
	example.setAttribute("chooseComputeType", String(feature.chooseComputeType));
	bezierExamples?.append(example);
});

const subpathExamples = document.getElementById("subpath-examples");
(Object.keys(subpathFeatures) as SubpathFeatureName[]).forEach((featureName) => {
	const feature = subpathFeatures[featureName];
	const example = document.createElement("subpath-example-pane");

	example.setAttribute("name", featureName);
	example.setAttribute("sliderOptions", JSON.stringify(feature.sliderOptions || []));
	example.setAttribute("triggerOnMouseMove", String(feature.triggerOnMouseMove));
	example.setAttribute("chooseComputeType", String(feature.chooseComputeType));
	subpathExamples?.append(example);
});

declare global {
	interface HTMLElementTagNameMap {
		"bezier-example": BezierExample;
		"bezier-example-pane": BezierExamplePane;
		"subpath-example": SubpathExample;
		"subpath-example-pane": SubpathExamplePane;
	}
}
