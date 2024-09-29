import { default as init } from "@/../wasm/pkg";
import type { BezierFeatureKey, BezierFeatureOptions } from "@/features/bezier-features";
import bezierFeatures from "@/features/bezier-features";
import type { SubpathFeatureKey, SubpathFeatureOptions } from "@/features/subpath-features";
import subpathFeatures from "@/features/subpath-features";
import { bezierDemoGroup, subpathDemoGroup } from "@/utils/groups";

(async () => {
	await init();

	// Determine whether the page needs to recompute which examples to show
	window.addEventListener("hashchange", (e: HashChangeEvent) => {
		const isUrlSolo = (url: string) => {
			const splitHash = url.split("#")?.[1]?.split("/");
			return splitHash?.length === 3 && splitHash?.[2] === "solo";
		};

		const isOldHashSolo = isUrlSolo(e.oldURL);
		const isNewHashSolo = isUrlSolo(e.newURL);
		const target = document.getElementById(window.location.hash.substring(1));
		if (!target || isOldHashSolo !== isNewHashSolo) renderExamples();
	});

	renderExamples();
})();

function renderExamples() {
	const hash = window.location.hash;
	const splitHash = hash.split("/");

	// Scroll to specified hash if it exists
	if (hash) document.getElementById(hash.substring(1))?.scrollIntoView();

	// Determine which examples to render based on hash
	if (splitHash[0] === "#bezier" && splitHash[1] in bezierFeatures && splitHash[2] === "solo") {
		window.document.body.innerHTML = `<div id="bezier-demos"></div>`;
		const container = document.getElementById("bezier-demos");
		if (!container) return;

		const key = splitHash[1];
		const value = (bezierFeatures as Record<string, BezierFeatureOptions>)[key];
		if (value) container.append(bezierDemoGroup(key as BezierFeatureKey, value));

		return;
	}

	if (splitHash[0] === "#subpath" && splitHash[1] in subpathFeatures && splitHash[2] === "solo") {
		window.document.body.innerHTML = `<div id="subpath-demos"></div>`;
		const container = document.getElementById("subpath-demos");
		if (!container) return;

		const key = splitHash[1];
		const value = (subpathFeatures as Record<string, SubpathFeatureOptions>)[key];
		if (value) container.append(subpathDemoGroup(key as SubpathFeatureKey, value));

		return;
	}

	window.document.body.innerHTML = `
		<h1 class="website-header">Bezier-rs Interactive Documentation</h1>
		<p class="website-description">
			This is the interactive documentation for the <a href="https://crates.io/crates/bezier-rs">Bezier-rs</a> library. View the
			<a href="https://docs.rs/bezier-rs/latest/bezier_rs">crate documentation</a>
			for detailed function descriptions and API usage. Click and drag on the endpoints of the demo curves to visualize the various Bezier utilities and functions.
		</p>
		
		<h2 class="category-header">Beziers</h2>
		<div id="bezier-demos"></div>
		
		<h2 class="category-header">Subpaths</h2>
		<div id="subpath-demos"></div>
		`.trim();

	const bezierDemos = document.getElementById("bezier-demos") || undefined;
	if (bezierDemos) Object.entries(bezierFeatures).forEach(([key, options]) => bezierDemos.appendChild(bezierDemoGroup(key as BezierFeatureKey, options)));

	const subpathDemos = document.getElementById("subpath-demos") || undefined;
	if (subpathDemos) Object.entries(subpathFeatures).forEach(([key, options]) => subpathDemos.appendChild(subpathDemoGroup(key as SubpathFeatureKey, options)));
}
