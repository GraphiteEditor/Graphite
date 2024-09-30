import { default as init } from "@/../wasm/pkg";
import { demoBezier } from "@/demo-bezier";
import { demoSubpath } from "@/demo-subpath";
import bezierFeatures from "@/features-bezier";
import type { BezierFeatureKey, BezierFeatureOptions } from "@/features-bezier";
import subpathFeatures from "@/features-subpath";
import type { SubpathFeatureKey, SubpathFeatureOptions } from "@/features-subpath";
import { BEZIER_CURVE_TYPE, getBezierDemoPointDefaults, getSubpathDemoArgs } from "@/types";
import type { DemoArgs, BezierCurveType, BezierDemoArgs, SubpathDemoArgs } from "@/types";

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

export function renderDemo(demo: ReturnType<typeof demoBezier> | ReturnType<typeof demoSubpath>) {
	const id = String(Math.random()).slice(2);
	demo.element.insertAdjacentHTML(
		"beforeend",
		`
		<h4 class="demo-header">${demo.title}</h4>
		<div class="demo-figure" data-demo-figure="${id}"></div>
		<div class="parent-input-container" data-parent-input-container="${id}">
			${(() =>
				demo.inputOptions
					.map((inputOption) =>
						`
						<div
							class="${(() => {
								if (inputOption.inputType === "dropdown") return "select-container";
								if (inputOption.inputType === "slider") return "slider-container";
								return "";
							})()}"
							data-input-container
						>
							<div class="input-label" data-input-label>
								${inputOption.variable}: ${inputOption.inputType === "dropdown" ? "" : demo.sliderData[inputOption.variable]}${demo.getSliderUnit(inputOption.variable)}
							</div>
							${(() => {
								if (inputOption.inputType !== "dropdown") return "";
								return `
									<select class="select-input" value="${inputOption.default}" ${inputOption.disabled ? "disabled" : ""} data-select>
										${inputOption.options?.map((value, idx) => `<option value="${idx}" id="${idx}-${value}">${value}</option>`).join("\n")}
									</select>
									`.trim();
							})()}
							${(() => {
								if (inputOption.inputType !== "slider") return "";
								const ratio = (Number(inputOption.default) - (inputOption.min || 0)) / ((inputOption.max || 100) - (inputOption.min || 0));
								return `
									<input
										class="slider-input"
										type="range"
										max="${inputOption.max}"
										min="${inputOption.min}"
										step="${inputOption.step}"
										value="${inputOption.default}"
										style="--range-ratio: ${ratio}"
										data-slider-input
									/>
									`.trim();
							})()}
						</div>
						`.trim(),
					)
					.join("\n"))()}
		</div>
		`.trim(),
	);

	const figure = demo.element.querySelector(`[data-demo-figure="${id}"]`);
	if (!(figure instanceof HTMLElement)) return;
	figure.addEventListener("mousedown", demo.onMouseDown);
	figure.addEventListener("mouseup", demo.onMouseUp);
	figure.addEventListener("mousemove", demo.onMouseMove);

	demo.inputOptions.forEach((inputOption, index) => {
		const inputContainer = demo.element.querySelectorAll(`[data-parent-input-container="${id}"] [data-input-container]`)[index];
		if (!(inputContainer instanceof HTMLDivElement)) return;

		if (inputOption.inputType === "dropdown") {
			const selectElement = inputContainer.querySelector("[data-select]");
			if (!(selectElement instanceof HTMLSelectElement)) return;

			selectElement.addEventListener("change", (e: Event) => {
				if (!(e.target instanceof HTMLSelectElement)) return;

				demo.sliderData[inputOption.variable] = Number(e.target.value);
				demo.drawDemo(figure);
			});
		}

		if (inputOption.inputType === "slider") {
			const sliderInput = inputContainer.querySelector("[data-slider-input]");
			if (!(sliderInput instanceof HTMLInputElement)) return;

			sliderInput.addEventListener("input", (e: Event) => {
				const target = e.target;
				if (!(target instanceof HTMLInputElement)) return;

				// Set the slider label text
				const variable = inputOption.variable;
				const data = demo.sliderData[variable];
				const unit = demo.getSliderUnit(variable);
				const label = inputContainer.querySelector("[data-input-label]");
				if (!(label instanceof HTMLDivElement)) return;
				label.innerText = `${variable}: ${data}${unit}`;

				// Set the slider input range percentage
				sliderInput.style.setProperty("--range-ratio", String((Number(target.value) - (inputOption.min || 0)) / ((inputOption.max || 100) - (inputOption.min || 0))));

				// Update the slider data and redraw the demo
				demo.sliderData[variable] = Number(target.value);
				demo.drawDemo(figure);
			});
		}
	});
}

function renderDemoGroup<T extends DemoArgs>(id: string, name: string, demos: T[], buildDemo: (demo: T) => HTMLElement): HTMLDivElement {
	const demoGroup = document.createElement("div");
	demoGroup.className = "demo-group-container";

	demoGroup.insertAdjacentHTML(
		"beforeend",
		`
		${(() => {
			// Add header and href anchor if not on a solo example page
			const currentHash = window.location.hash.split("/");
			if (currentHash.length === 3 || currentHash[2] === "solo") return "";
			return `
				<h3 class="demo-group-header">
					<a href="#${id}">#</a>
					${name}
				</h3>
				`.trim();
		})()}
		<div class="demo-row" data-demo-row></div>
		`.trim(),
	);

	const demoRow = demoGroup.querySelector("[data-demo-row]");
	if (demoRow) {
		demos.forEach((demo) => {
			if (demo.disabled) return;
			demoRow.append(buildDemo(demo));
		});
	}

	return demoGroup;
}

function bezierDemoGroup(key: BezierFeatureKey, options: BezierFeatureOptions): HTMLDivElement {
	const demoOptions = options.demoOptions || {};
	const triggerOnMouseMove = options.triggerOnMouseMove || false;
	const name = bezierFeatures[key].name;
	const id = `bezier/${key}`;

	const demos: BezierDemoArgs[] = BEZIER_CURVE_TYPE.map((curveType: BezierCurveType) => ({
		title: curveType,
		disabled: demoOptions[curveType]?.disabled || false,
		points: demoOptions[curveType]?.customPoints || getBezierDemoPointDefaults()[curveType],
		inputOptions: demoOptions[curveType]?.inputOptions || demoOptions.Quadratic?.inputOptions || [],
	}));

	return renderDemoGroup(id, name, demos, (demo: BezierDemoArgs): HTMLElement => demoBezier(demo.title, demo.points, key, demo.inputOptions, triggerOnMouseMove).element);
}

function subpathDemoGroup(key: SubpathFeatureKey, options: SubpathFeatureOptions): HTMLDivElement {
	const inputOptions = options.inputOptions || [];
	const triggerOnMouseMove = options.triggerOnMouseMove || false;
	const name = subpathFeatures[key].name;
	const id = `subpath/${key}`;

	const demos = getSubpathDemoArgs();

	const buildDemo = (demo: SubpathDemoArgs): HTMLElement => {
		const newInputOptions = inputOptions.map((option) => ({
			...option,
			disabled: option.isDisabledForClosed && demo.closed,
		}));
		return demoSubpath(demo.title, demo.triples, key, demo.closed, newInputOptions, triggerOnMouseMove).element;
	};

	return renderDemoGroup(id, name, demos, buildDemo);
}
