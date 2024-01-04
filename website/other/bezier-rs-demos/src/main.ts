import { default as init } from "@/../wasm/pkg";
import BezierDemo from "@/components/BezierDemo";
import BezierDemoPane from "@/components/BezierDemoPane";
import SubpathDemo from "@/components/SubpathDemo";
import SubpathDemoPane from "@/components/SubpathDemoPane";
import type { BezierFeatureKey } from "@/features/bezier-features";
import bezierFeatures from "@/features/bezier-features";
import type { SubpathFeatureKey } from "@/features/subpath-features";
import subpathFeatures from "@/features/subpath-features";

(async () => {
	await init();

	window.customElements.define("bezier-demo", BezierDemo);
	window.customElements.define("bezier-demo-pane", BezierDemoPane);
	window.customElements.define("subpath-demo", SubpathDemo);
	window.customElements.define("subpath-demo-pane", SubpathDemoPane);

	window.addEventListener("hashchange", (e: Event) => {
		const hashChangeEvent = e as HashChangeEvent;
		const isOldHashSolo = isUrlSolo(hashChangeEvent.oldURL);
		const isNewHashSolo = isUrlSolo(hashChangeEvent.newURL);
		const target = document.getElementById(window.location.hash.substring(1));
		// Determine whether the page needs to recompute which examples to show
		if (!target || isOldHashSolo !== isNewHashSolo) {
			renderExamples();
		}
	});

	renderExamples();
})();

function renderBezierPane(featureName: BezierFeatureKey, container?: HTMLElement) {
	const feature = bezierFeatures[featureName];
	const demo = document.createElement("bezier-demo-pane");

	demo.setAttribute("name", featureName);
	demo.setAttribute("demoOptions", JSON.stringify(feature.demoOptions || {}));
	demo.setAttribute("triggerOnMouseMove", String(feature.triggerOnMouseMove));
	container?.append(demo);
}

function renderSubpathPane(featureName: SubpathFeatureKey, container?: HTMLElement) {
	const feature = subpathFeatures[featureName];
	const demo = document.createElement("subpath-demo-pane");

	demo.setAttribute("name", featureName);
	demo.setAttribute("inputOptions", JSON.stringify(feature.inputOptions || []));
	demo.setAttribute("triggerOnMouseMove", String(feature.triggerOnMouseMove));
	container?.append(demo);
}

function isUrlSolo(url: string): boolean {
	const hash = url.split("#")?.[1];
	const splitHash = hash?.split("/");
	return splitHash?.length === 3 && splitHash?.[2] === "solo";
}

function renderExamples() {
	const hash = window.location.hash;
	const splitHash = hash.split("/");

	// Determine which examples to render based on hash
	if (splitHash[0] === "#bezier" && splitHash[1] in bezierFeatures && splitHash[2] === "solo") {
		window.document.body.innerHTML = `<div id="bezier-demos"></div>`;
		renderBezierPane(splitHash[1] as BezierFeatureKey, document.getElementById("bezier-demos") || undefined);
	} else if (splitHash[0] === "#subpath" && splitHash[1] in subpathFeatures && splitHash[2] === "solo") {
		window.document.body.innerHTML = `<div id="subpath-demos"></div>`;
		renderSubpathPane(splitHash[1] as SubpathFeatureKey, document.getElementById("subpath-demos") || undefined);
	} else {
		window.document.body.innerHTML = `
		<h1 class="website-header">Bezier-rs Interactive Documentation</h1>
		<p class="website-description">
			This is the interactive documentation for the <a href="https://crates.io/crates/bezier-rs"><b>Bezier-rs</b></a> library. View the
			<a href="https://docs.rs/bezier-rs/latest/bezier_rs">crate documentation</a>
			for detailed function descriptions and API usage. Click and drag on the endpoints of the demo curves to visualize the various Bezier utilities and functions.
		</p>
		
		<h2 class="class-header">Beziers</h2>
		<div id="bezier-demos"></div>
		<h2 class="class-header">Subpaths</h2>
		<div id="subpath-demos"></div>
		`.trim();

		const bezierDemos = document.getElementById("bezier-demos") || undefined;
		const subpathDemos = document.getElementById("subpath-demos") || undefined;

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
