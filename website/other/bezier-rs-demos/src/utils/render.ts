import type { newBezierDemo } from "@/components/BezierDemo";
import type { newSubpathDemo } from "@/components/SubpathDemo";
import type { DemoArgs } from "@/utils/types";

export function renderDemo(demo: ReturnType<typeof newBezierDemo> | ReturnType<typeof newSubpathDemo>) {
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

export function renderDemoGroup<T extends DemoArgs>(id: string, name: string, demos: T[], buildDemo: (demo: T) => HTMLElement): HTMLDivElement {
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
