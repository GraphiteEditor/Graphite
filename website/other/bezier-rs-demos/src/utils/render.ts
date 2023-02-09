import { ComputeType, Demo, DemoPane, SliderOption } from "@/utils/types";

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

	demo.sliderOptions.forEach((sliderOption: SliderOption) => {
		const sliderLabel = document.createElement("div");
		const sliderData = demo.sliderData[sliderOption.variable];
		const sliderUnit = demo.getSliderUnit(sliderData, sliderOption.variable);
		sliderLabel.className = "slider-label";
		sliderLabel.innerText = `${sliderOption.variable} = ${sliderData}${sliderUnit}`;
		demo.append(sliderLabel);

		const sliderInput = document.createElement("input");
		sliderInput.className = "slider-input";
		sliderInput.type = "range";
		sliderInput.max = String(sliderOption.max);
		sliderInput.min = String(sliderOption.min);
		sliderInput.step = String(sliderOption.step);
		sliderInput.value = String(sliderOption.default);
		sliderInput.addEventListener("input", (event: Event): void => {
			demo.sliderData[sliderOption.variable] = Number((event.target as HTMLInputElement).value);
			sliderLabel.innerText = `${sliderOption.variable} = ${demo.sliderData[sliderOption.variable]}${sliderUnit}`;
			demo.drawDemo(figure);
		});
		demo.append(sliderInput);
	});
}

export function renderDemoPane(demoPane: DemoPane): void {
	const container = document.createElement("div");
	container.className = "demo-pane-container";

	const headerAnchorLink = document.createElement("a");
	headerAnchorLink.innerText = "#";
	const currentHash = window.location.hash.split("/");
	// Add href anchor if not on a solo example page
	if (currentHash.length !== 3 && currentHash[2] !== "solo") headerAnchorLink.href = `#${demoPane.id}`;

	const header = document.createElement("h3");
	header.innerText = demoPane.name;
	header.className = "demo-pane-header";
	header.append(headerAnchorLink);

	const computeTypeContainer = document.createElement("div");
	computeTypeContainer.className = "compute-type-choice";

	const computeTypeLabel = document.createElement("strong");
	computeTypeLabel.innerText = "ComputeType:";
	computeTypeContainer.append(computeTypeLabel);

	const radioInputs = ["Parametric", "Euclidean"].map((computeType) => {
		const id = `${demoPane.id}-${computeType}`;
		const radioInput = document.createElement("input");
		radioInput.type = "radio";
		radioInput.id = id;
		radioInput.value = computeType;
		radioInput.name = "ComputeType";
		radioInput.checked = computeType === "Parametric";
		computeTypeContainer.append(radioInput);

		const label = document.createElement("label");
		label.htmlFor = id;
		label.innerText = computeType;
		computeTypeContainer.append(label);
		return radioInput;
	});

	const demoRow = document.createElement("div");
	demoRow.className = "demo-row";

	demoPane.demos.forEach((demo) => {
		if (demo.disabled) {
			return;
		}
		const demoComponent = demoPane.buildDemo(demo);

		radioInputs.forEach((radioInput: HTMLElement) => {
			radioInput.addEventListener("input", (event: Event): void => {
				demoPane.computeType = (event.target as HTMLInputElement).value as ComputeType;
				demoComponent.setAttribute("computetype", demoPane.computeType);
			});
		});
		demoRow.append(demoComponent);
	});

	container.append(header);
	if (demoPane.chooseComputeType) {
		container.append(computeTypeContainer);
	}
	container.append(demoRow);

	demoPane.append(container);
}
