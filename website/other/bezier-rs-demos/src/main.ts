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

function isUrlSolo(url: string): boolean {
	const hash = url.split("#")?.[1];
	const splitHash = hash?.split("/");
	return splitHash?.length === 3 && splitHash?.[2] === "solo";
}

window.addEventListener("hashchange", (e: Event): void => {
	const hashChangeEvent = e as HashChangeEvent;
	const isOldHashSolo = isUrlSolo(hashChangeEvent.oldURL);
	const isNewHashSolo = isUrlSolo(hashChangeEvent.newURL);
	const target = document.getElementById(window.location.hash.substring(1));
	// Determine whether the page needs to recompute which examples to show
	if (!target || isOldHashSolo !== isNewHashSolo) {
		renderExamples();
	}
});

function renderExamples(): void {
	const hash = window.location.hash;
	const splitHash = hash.split("/");

	// Determine which examples to render based on hash
	if (splitHash[0] === "#bezier" && splitHash[1] in bezierFeatures && splitHash[2] === "solo") {
		window.document.body.innerHTML = `<div id="bezier-demos"></div>`;
		renderBezierPane(splitHash[1] as BezierFeatureKey, document.getElementById("bezier-demos"));
	} else if (splitHash[0] === "#subpath" && splitHash[1] in subpathFeatures && splitHash[2] === "solo") {
		window.document.body.innerHTML = `<div id="subpath-demos"></div>`;
		renderSubpathPane(splitHash[1] as SubpathFeatureKey, document.getElementById("subpath-demos"));
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

	// Scroll to specified hash if it exists
	if (hash) {
		const target = document.getElementById(hash.substring(1));
		if (target) {
			target.scrollIntoView();
		}
	}
}

renderExamples();
