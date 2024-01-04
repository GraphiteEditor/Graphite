import type { Demo, DemoPane, InputOption } from "@/utils/types";

export function renderDemo(demo: Demo) {
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

	demo.inputOptions.forEach((inputOption: InputOption) => {
		const isDropdown = inputOption.inputType === "dropdown";

		const sliderContainer = document.createElement("div");
		sliderContainer.className = isDropdown ? "select-container" : "slider-container";

		const sliderLabel = document.createElement("div");
		const sliderData = demo.sliderData[inputOption.variable];
		const sliderUnit = demo.getSliderUnit(sliderData, inputOption.variable);
		sliderLabel.className = "slider-label";
		sliderLabel.innerText = `${inputOption.variable}: ${isDropdown ? "" : sliderData}${sliderUnit}`;
		sliderContainer.appendChild(sliderLabel);

		if (isDropdown) {
			const selectInput = document.createElement("select");
			selectInput.className = "select-input";
			selectInput.value = String(inputOption.default);
			inputOption.options?.forEach((value, idx) => {
				const id = `${idx}-${value}`;
				const option = document.createElement("option");
				option.value = String(idx);
				option.id = id;
				option.text = value;
				selectInput.append(option);
			});

			if (inputOption.disabled) {
				selectInput.disabled = true;
			}

			selectInput.addEventListener("change", (event: Event) => {
				demo.sliderData[inputOption.variable] = Number((event.target as HTMLInputElement).value);
				demo.drawDemo(figure);
			});
			sliderContainer.appendChild(selectInput);
		} else {
			const sliderInput = document.createElement("input");
			sliderInput.className = "slider-input";
			sliderInput.type = "range";
			sliderInput.max = String(inputOption.max);
			sliderInput.min = String(inputOption.min);
			sliderInput.step = String(inputOption.step);
			sliderInput.value = String(inputOption.default);
			const range = Number(inputOption.max) - Number(inputOption.min);

			const ratio = (Number(inputOption.default) - Number(inputOption.min)) / range;
			sliderInput.style.setProperty("--range-ratio", String(ratio));

			sliderInput.addEventListener("input", (event: Event) => {
				const target = event.target as HTMLInputElement;
				demo.sliderData[inputOption.variable] = Number(target.value);
				const data = demo.sliderData[inputOption.variable];
				const unit = demo.getSliderUnit(demo.sliderData[inputOption.variable], inputOption.variable);
				sliderLabel.innerText = `${inputOption.variable}: ${data}${unit}`;

				const ratio = (Number(target.value) - Number(inputOption.min)) / range;
				sliderInput.style.setProperty("--range-ratio", String(ratio));

				demo.drawDemo(figure);
			});
			sliderContainer.appendChild(sliderInput);
		}

		parentSliderContainer.append(sliderContainer);
	});

	demo.append(parentSliderContainer);
}

export function renderDemoPane(demoPane: DemoPane) {
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

	const demoRow = document.createElement("div");
	demoRow.className = "demo-row";

	demoPane.demos.forEach((demo) => {
		if (demo.disabled) {
			return;
		}
		const demoComponent = demoPane.buildDemo(demo);
		demoRow.append(demoComponent);
	});

	container.append(demoRow);
	demoPane.append(container);
}
