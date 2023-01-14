import BezierDemo from "@/components/BezierDemo";
import BezierDemoPane from "@/components/BezierDemoPane";
import SubpathDemo from "@/components/SubpathDemo";
import SubpathDemoPane from "@/components/SubpathDemoPane";

import bezierFeatures, { BezierFeatureKey } from "@/features/bezier-features";
import subpathFeatures, { SubpathFeatureKey } from "@/features/subpath-features";

import "@/style.css";

declare global {
	interface HTMLElementTagNameMap {
		"bezier-demo": BezierDemo;
		"bezier-demo-pane": BezierDemoPane;
		"subpath-demo": SubpathDemo;
		"subpath-demo-pane": SubpathDemoPane;
	}
}

window.document.title = "Bezier-rs Interactive Documentation";

window.customElements.define("bezier-demo", BezierDemo);
window.customElements.define("bezier-demo-pane", BezierDemoPane);
window.customElements.define("subpath-demo", SubpathDemo);
window.customElements.define("subpath-demo-pane", SubpathDemoPane);

function renderBezierPane(featureName: BezierFeatureKey, container: HTMLElement | null): void {
	const feature = bezierFeatures[featureName];
	const demo = document.createElement("bezier-demo-pane");

	demo.setAttribute("name", featureName);
	demo.setAttribute("demoOptions", JSON.stringify(feature.demoOptions || {}));
	demo.setAttribute("triggerOnMouseMove", String(feature.triggerOnMouseMove));
	demo.setAttribute("chooseComputeType", String(feature.chooseComputeType));
	container?.append(demo);
}

function renderSubpathPane(featureName: SubpathFeatureKey, container: HTMLElement | null): void {
	const feature = subpathFeatures[featureName];
	const demo = document.createElement("subpath-demo-pane");

	demo.setAttribute("name", featureName);
	demo.setAttribute("sliderOptions", JSON.stringify(feature.sliderOptions || []));
	demo.setAttribute("triggerOnMouseMove", String(feature.triggerOnMouseMove));
	demo.setAttribute("chooseComputeType", String(feature.chooseComputeType));
	container?.append(demo);
}

const pathname = window.location.pathname;
const splitPathName = pathname.split("/");

// Render based on pathname
if (splitPathName[1] === "bezier" && splitPathName[2] in bezierFeatures) {
	window.document.body.innerHTML = `
  <div id="bezier-demos"></div>
  `.trim();
	renderBezierPane(splitPathName[2] as BezierFeatureKey, document.getElementById("bezier-demos"));
} else if (splitPathName[1] === "subpath" && splitPathName[2] in subpathFeatures) {
	window.document.body.innerHTML = `
  <div id="subpath-demos"></div>
  `.trim();
	renderSubpathPane(splitPathName[2] as SubpathFeatureKey, document.getElementById("subpath-demos"));
} else if (pathname !== "/") {
	window.location.pathname = "/";
} else {
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

	const bezierDemos = document.getElementById("bezier-demos");
	const subpathDemos = document.getElementById("subpath-demos");

	(Object.keys(bezierFeatures) as BezierFeatureKey[]).forEach((feature) => renderBezierPane(feature, bezierDemos));
	(Object.keys(subpathFeatures) as SubpathFeatureKey[]).forEach((feature) => renderSubpathPane(feature, subpathDemos));
}
