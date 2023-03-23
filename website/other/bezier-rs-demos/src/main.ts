import BezierDemo from "@graphite/components/BezierDemo";
import BezierDemoPane from "@graphite/components/BezierDemoPane";
import SubpathDemo from "@graphite/components/SubpathDemo";
import SubpathDemoPane from "@graphite/components/SubpathDemoPane";

import bezierFeatures, { BezierFeatureKey } from "@graphite/features/bezier-features";
import subpathFeatures, { SubpathFeatureKey } from "@graphite/features/subpath-features";

import "@graphite/style.css";

declare global {
	interface HTMLElementTagNameMap {
		"bezier-demo": BezierDemo;
		"bezier-demo-pane": BezierDemoPane;
		"subpath-demo": SubpathDemo;
		"subpath-demo-pane": SubpathDemoPane;
	}
}

window.document.title = "Bezier-rs Interactive Documentation";
window.document.head.innerHTML += `
<link rel="stylesheet" href="https://rsms.me/inter/inter.css">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=Bona+Nova:wght@700&family=EB+Garamond:ital,wght@0,500;1,500&display=swap" rel="stylesheet">
`.trim();

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
	container?.append(demo);
}

function renderSubpathPane(featureName: SubpathFeatureKey, container: HTMLElement | null): void {
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
