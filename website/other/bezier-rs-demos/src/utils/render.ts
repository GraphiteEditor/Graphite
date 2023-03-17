import { TVariant, Demo, DemoPane, SliderOption } from "@/utils/types";

export function renderDemo(demo: Demo): void {
	const header = document.createElement("h4");
	header.className = "demo-header";
	header.innerText = demo.title;

	const figure = document.createElement("figure");
	figure.className = "demo-figure";
	figure.addEventListener("mousedown", demo.onMouseDown.bind(demo));
	figure.addEventListener("mouseup", demo.onMouseUp.bind(demo));
	figure.addEventListener("mousemove", demo.onMouseMove.bind(demo));

	demo.append(header);
	demo.append(figure);

	const parentSliderContainer = document.createElement("div");
	parentSliderContainer.className = "parent-slider-container";

	demo.sliderOptions.forEach((sliderOption: SliderOption) => {
		const sliderContainer = document.createElement("div");
		sliderContainer.className = "slider-container";

		const sliderLabel = document.createElement("div");
		const sliderData = demo.sliderData[sliderOption.variable];
		const sliderUnit = demo.getSliderUnit(sliderData, sliderOption.variable);
		sliderLabel.className = "slider-label";
		sliderLabel.innerText = `${sliderOption.variable}: ${sliderOption.variable !== "strategy" ? sliderData : ""}${sliderUnit}`;
		sliderContainer.appendChild(sliderLabel);

		const sliderInput = document.createElement("input");
		sliderInput.className = "slider-input";
		sliderInput.type = "range";
		sliderInput.max = String(sliderOption.max);
		sliderInput.min = String(sliderOption.min);
		sliderInput.step = String(sliderOption.step);
		sliderInput.value = String(sliderOption.default);
		sliderInput.addEventListener("input", (event: Event): void => {
			demo.sliderData[sliderOption.variable] = Number((event.target as HTMLInputElement).value);
			const data = sliderOption.variable !== "strategy" ? demo.sliderData[sliderOption.variable] : "";
			const unit = demo.getSliderUnit(demo.sliderData[sliderOption.variable], sliderOption.variable);
			sliderLabel.innerText = `${sliderOption.variable}: ${data}${unit}`;
			demo.drawDemo(figure);
		});
		sliderContainer.appendChild(sliderInput);

		parentSliderContainer.append(sliderContainer);
	});

	demo.append(parentSliderContainer);
}

export function renderDemoPane(demoPane: DemoPane): void {
	const container = document.createElement("div");
	container.className = "demo-pane-container";

	const headerAnchorLink = document.createElement("a");
	headerAnchorLink.innerText = "#";
	const currentHash = window.location.hash.split("/");
	// Add header and href anchor if not on a solo example page
	if (currentHash.length !== 3 && currentHash[2] !== "solo") {
		headerAnchorLink.href = `#${demoPane.id}`;
		const header = document.createElement("h3");
		header.innerText = demoPane.name;
		header.className = "demo-pane-header";
		header.append(headerAnchorLink);
		container.append(header);
	}

	const tVariantContainer = document.createElement("div");
	tVariantContainer.className = "t-variant-choice";

	const tVariantLabel = document.createElement("strong");
	tVariantLabel.innerText = "TValue Variant:";
	tVariantContainer.append(tVariantLabel);

	const variantSelect = document.createElement("select");
	["Parametric", "Euclidean"].forEach((tVariant) => {
		const id = `${demoPane.id}-${tVariant}`;
		const option = document.createElement("option");
		option.value = tVariant;
		option.id = id;
		option.text = tVariant;
		variantSelect.append(option);
	});

	tVariantContainer.appendChild(variantSelect);

	const demoRow = document.createElement("div");
	demoRow.className = "demo-row";

	demoPane.demos.forEach((demo) => {
		if (demo.disabled) {
			return;
		}
		const demoComponent = demoPane.buildDemo(demo);

		variantSelect.addEventListener("change", (event: Event): void => {
			demoPane.tVariant = (event.target as HTMLInputElement).value as TVariant;
			demoComponent.setAttribute("tvariant", demoPane.tVariant);
		});

		demoRow.append(demoComponent);
	});

	container.append(demoRow);

	if (demoPane.chooseTVariant) {
		container.append(tVariantContainer);
	}

	demoPane.append(container);
}
