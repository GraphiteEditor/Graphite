<script lang="ts">
	import { createEventDispatcher } from "svelte";

	import { type NumberInputMode, type NumberInputIncrementBehavior } from "@graphite/wasm-communication/messages";

	import FieldInput from "@graphite/components/widgets/inputs/FieldInput.svelte";

	// emits: ["update:value"],
	const dispatch = createEventDispatcher<{ value: number | undefined }>();

	// Label
	export let label: string | undefined = undefined;
	export let tooltip: string | undefined = undefined;

	// Disabled
	export let disabled = false;

	// Value
	export let value: number | undefined = undefined; // When not provided, a dash is displayed
	export let min: number | undefined = undefined;
	export let max: number | undefined = undefined;
	export let isInteger = false;

	// Number presentation
	export let displayDecimalPlaces = 3;
	export let unit = "";
	export let unitIsHiddenWhenEditing = true;

	// Mode behavior
	// "Increment" shows arrows and allows dragging left/right to change the value.
	// "Range" shows a range slider between some minimum and maximum value.
	export let mode: NumberInputMode = "Increment";
	// When `mode` is "Increment", `step` is the multiplier or addend used with `incrementBehavior`.
	// When `mode` is "Range", `step` is the range slider's snapping increment if `isInteger` is `true`.
	export let step = 1;
	// `incrementBehavior` is only applicable with a `mode` of "Increment".
	// "Add"/"Multiply": The value is added or multiplied by `step`.
	// "None": the increment arrows are not shown.
	// "Callback": the functions `incrementCallbackIncrease` and `incrementCallbackDecrease` call custom behavior.
	export let incrementBehavior: NumberInputIncrementBehavior = "Add";
	// `rangeMin` and `rangeMax` are only applicable with a `mode` of "Range".
	// They set the lower and upper values of the slider to drag between.
	export let rangeMin = 0;
	export let rangeMax = 1;

	// Styling
	export let minWidth = 0;
	export let sharpRightCorners = false;

	// Callbacks
	export let incrementCallbackIncrease: (() => void) | undefined = undefined;
	export let incrementCallbackDecrease: (() => void) | undefined = undefined;

	let self: FieldInput | undefined;
	let text = displayText(value, displayDecimalPlaces, unit);
	let editing = false;
	// Stays in sync with a binding to the actual input range slider element.
	let rangeSliderValue = value !== undefined ? value : 0;
	// Value used to render the position of the fake slider when applicable, and length of the progress colored region to the slider's left.
	// This is the same as `rangeSliderValue` except in the "mousedown" state, when it has the previous location before the user's mousedown.
	let rangeSliderValueAsRendered = value !== undefined ? value : 0;
	// "default": no interaction is happening.
	// "mousedown": the user has pressed down the mouse and might next decide to either drag left/right or release without dragging.
	// "dragging": the user is dragging the slider left/right.
	let rangeSliderClickDragState: "default" | "mousedown" | "dragging" = "default";

	$: watchValue(value);

	$: sliderStepValue = isInteger ? (step === undefined ? 1 : step) : "any";

	// Called only when `value` is changed from outside this component
	function watchValue(value: number | undefined) {
		// Don't update if the slider is currently being dragged (we don't want the backend fighting with the user's drag)
		if (rangeSliderClickDragState === "dragging") return;

		// Draw a dash if the value is undefined
		if (value === undefined) {
			text = "-";
			return;
		}

		// Update the range slider with the new value
		rangeSliderValue = value;
		rangeSliderValueAsRendered = value;

		// The simple `clamp()` function can't be used here since `undefined` values need to be boundless
		let sanitized = value;
		if (typeof min === "number") sanitized = Math.max(sanitized, min);
		if (typeof max === "number") sanitized = Math.min(sanitized, max);

		text = displayText(sanitized, displayDecimalPlaces, unit);
	}

	function onSliderInput() {
		// Keep only 4 digits after the decimal point
		const ROUNDING_EXPONENT = 4;
		const ROUNDING_MAGNITUDE = 10 ** ROUNDING_EXPONENT;
		const roundedValue = Math.round(rangeSliderValue * ROUNDING_MAGNITUDE) / ROUNDING_MAGNITUDE;

		// Exit if this is an extraneous event invocation that occurred after mouseup, which happens in Firefox
		if (value !== undefined && Math.abs(value - roundedValue) < 1 / ROUNDING_MAGNITUDE) {
			return;
		}

		// The first event upon mousedown means we transition to a "mousedown" state
		if (rangeSliderClickDragState === "default") {
			rangeSliderClickDragState = "mousedown";

			// Exit early because we don't want to use the value set by where on the track the user pressed
			return;
		}

		// The second event upon mousedown that occurs by moving left or right means the user has committed to dragging the slider
		if (rangeSliderClickDragState === "mousedown") {
			rangeSliderClickDragState = "dragging";
		}

		// If we're in a dragging state, we want to use the new slider value
		rangeSliderValueAsRendered = roundedValue;
		updateValue(roundedValue, min, max, displayDecimalPlaces, unit);
	}

	function onSliderPointerDown() {
		// We want to render the fake slider thumb at the old position, which is still the number held by `value`
		rangeSliderValueAsRendered = value || 0;

		// Because an `input` event is fired right before or after this (depending on browser), that first
		// invocation will transition the state machine to `mousedown`. That's why we don't do it here.
	}

	function onSliderPointerUp() {
		// User clicked but didn't drag, so we focus the text input element
		if (rangeSliderClickDragState === "mousedown") {
			const inputElement = self?.element();
			if (!inputElement) return;

			// Set the slider position back to the original position to undo the user moving it
			rangeSliderValue = rangeSliderValueAsRendered;

			// Begin editing the number text field
			inputElement.focus();
		}

		// Releasing the mouse means we can reset the state machine
		rangeSliderClickDragState = "default";
	}

	function onTextFocused() {
		if (value === undefined) text = "";
		else if (unitIsHiddenWhenEditing) text = `${value}`;
		else text = `${value}${unPluralize(unit, value)}`;

		editing = true;

		self?.selectAllText(text);
	}

	// Called only when `value` is changed from the <input> element via user input and committed, either with the
	// enter key (via the `change` event) or when the <input> element is unfocused (with the `blur` event binding)
	function onTextChanged() {
		// The `unFocus()` call at the bottom of this function and in `onCancelTextChange()` causes this function to be run again, so this check skips a second run
		if (!editing) return;

		const parsed = parseFloat(text);
		const newValue = Number.isNaN(parsed) ? undefined : parsed;

		updateValue(newValue, min, max, displayDecimalPlaces, unit);

		editing = false;

		self?.unFocus();
	}

	function onCancelTextChange() {
		updateValue(undefined, min, max, displayDecimalPlaces, unit);

		rangeSliderValue = value;
		rangeSliderValueAsRendered = value;

		editing = false;

		self?.unFocus();
	}

	function onIncrement(direction: "Decrease" | "Increase") {
		if (value === undefined) return;

		const actions: Record<NumberInputIncrementBehavior, () => void> = {
			Add: () => {
				const directionAddend = direction === "Increase" ? step : -step;
				updateValue(value !== undefined ? value + directionAddend : undefined, min, max, displayDecimalPlaces, unit);
			},
			Multiply: () => {
				const directionMultiplier = direction === "Increase" ? step : 1 / step;
				updateValue(value !== undefined ? value * directionMultiplier : undefined, min, max, displayDecimalPlaces, unit);
			},
			Callback: () => {
				if (direction === "Increase") incrementCallbackIncrease?.();
				if (direction === "Decrease") incrementCallbackDecrease?.();
			},
			None: () => {},
		};
		const action = actions[incrementBehavior];
		action();
	}

	function updateValue(newValue: number | undefined, min: number | undefined, max: number | undefined, displayDecimalPlaces: number, unit: string) {
		// Check if the new value is valid, otherwise we use the old value (rounded if it's an integer)
		const oldValue = value !== undefined && isInteger ? Math.round(value) : value;
		let cleaned = newValue !== undefined ? newValue : oldValue;

		if (cleaned !== undefined) {
			if (typeof min === "number" && !Number.isNaN(min)) cleaned = Math.max(cleaned, min);
			if (typeof max === "number" && !Number.isNaN(max)) cleaned = Math.min(cleaned, max);

			rangeSliderValue = cleaned;
			rangeSliderValueAsRendered = cleaned;
		}

		text = displayText(cleaned, displayDecimalPlaces, unit);

		if (newValue !== undefined) dispatch("value", cleaned);
	}

	function displayText(value: number | undefined, displayDecimalPlaces: number, unit: string): string {
		if (value === undefined) return "-";

		// Find the amount of digits on the left side of the decimal
		// 10.25 == 2
		// 1.23 == 1
		// 0.23 == 0 (Reason for the slightly more complicated code)
		const absValueInt = Math.floor(Math.abs(value));
		const leftSideDigits = absValueInt === 0 ? 0 : absValueInt.toString().length;
		const roundingPower = 10 ** Math.max(displayDecimalPlaces - leftSideDigits, 0);

		const displayValue = Math.round(value * roundingPower) / roundingPower;

		return `${displayValue}${unPluralize(unit, value)}`;
	}

	function unPluralize(unit: string, value: number): string {
		if (value === 1 && unit.endsWith("s")) return unit.slice(0, -1);
		return unit;
	}
</script>

<FieldInput
	class={`number-input ${mode.toLocaleLowerCase()}`}
	value={text}
	on:value={({ detail }) => (text = detail)}
	on:textFocused={onTextFocused}
	on:textChanged={onTextChanged}
	on:cancelTextChange={onCancelTextChange}
	{label}
	{disabled}
	{tooltip}
	{sharpRightCorners}
	spellcheck={false}
	styles={{ "min-width": minWidth > 0 ? `${minWidth}px` : undefined, "--progress-factor": (rangeSliderValueAsRendered - rangeMin) / (rangeMax - rangeMin) }}
	bind:this={self}
>
	{#if value !== undefined && mode === "Increment" && incrementBehavior !== "None"}
		<button class="arrow left" on:click={() => onIncrement("Decrease")} tabindex="-1" />
		<button class="arrow right" on:click={() => onIncrement("Increase")} tabindex="-1" />
	{/if}
	{#if mode === "Range" && value !== undefined}
		<input
			type="range"
			class="slider"
			class:hidden={rangeSliderClickDragState === "mousedown"}
			bind:value={rangeSliderValue}
			min={rangeMin}
			max={rangeMax}
			step={sliderStepValue}
			{disabled}
			on:input={onSliderInput}
			on:pointerdown={onSliderPointerDown}
			on:pointerup={onSliderPointerUp}
			tabindex="-1"
		/>
	{/if}
	{#if value !== undefined}
		{#if value !== undefined && rangeSliderClickDragState === "mousedown"}
			<div class="fake-slider-thumb" />
		{/if}
		<div class="slider-progress" />
	{/if}
</FieldInput>

<style lang="scss" global>
	.number-input {
		input {
			text-align: center;
		}

		&.increment {
			// Widen the label and input margins from the edges by an extra 8px to make room for the increment arrows
			label {
				margin-left: 16px;
			}

			input[type="text"]:not(:focus).has-label {
				margin-right: 16px;
			}

			// Hide the increment arrows when entering text, disabled, or not hovered
			input[type="text"]:focus ~ .arrow,
			&.disabled .arrow,
			&:not(:hover) .arrow {
				display: none;
			}

			// Style the increment arrows
			.arrow {
				position: absolute;
				top: 0;
				margin: 0;
				padding: 9px 0;
				border: none;
				background: rgba(var(--color-1-nearblack-rgb), 0.75);

				&:hover {
					background: var(--color-6-lowergray);

					&.right::before {
						border-color: transparent transparent transparent var(--color-f-white);
					}

					&.left::after {
						border-color: transparent var(--color-f-white) transparent transparent;
					}
				}

				&.right {
					right: 0;
					padding-left: 7px;
					padding-right: 6px;

					&::before {
						content: "";
						display: block;
						width: 0;
						height: 0;
						border-style: solid;
						border-width: 3px 0 3px 3px;
						border-color: transparent transparent transparent var(--color-e-nearwhite);
					}
				}

				&.left {
					left: 0;
					padding-left: 6px;
					padding-right: 7px;

					&::after {
						content: "";
						display: block;
						width: 0;
						height: 0;
						border-style: solid;
						border-width: 3px 3px 3px 0;
						border-color: transparent var(--color-e-nearwhite) transparent transparent;
					}
				}
			}
		}

		&.range {
			position: relative;

			input[type="text"],
			label {
				z-index: 1;
			}

			input[type="text"]:focus ~ .slider,
			input[type="text"]:focus ~ .fake-slider-thumb,
			input[type="text"]:focus ~ .slider-progress {
				display: none;
			}

			.slider {
				position: absolute;
				left: 0;
				top: 0;
				width: 100%;
				height: 100%;
				padding: 0;
				margin: 0;
				-webkit-appearance: none; // Required until Safari 15.4 (Graphite supports 15.0+)
				appearance: none;
				background: none;
				cursor: default;
				// Except when disabled, the range slider goes above the label and input so it's interactable.
				// Then we use the blend mode to make it appear behind which works since the text is almost white and background almost black.
				// When disabled, the blend mode trick doesn't work with the grayer colors. But we don't need it to be interactable, so it can actually go behind properly.
				z-index: 2;
				mix-blend-mode: screen;

				&.hidden {
					opacity: 0;
				}

				// Chromium and Safari
				&::-webkit-slider-thumb {
					-webkit-appearance: none; // Required until Safari 15.4 (Graphite supports 15.0+)
					appearance: none;
					border-radius: 2px;
					width: 4px;
					height: 24px;
					background: #494949; // Becomes var(--color-5-dullgray) with screen blend mode over var(--color-1-nearblack) background
				}

				&:hover::-webkit-slider-thumb {
					background: #5b5b5b; // Becomes var(--color-6-lowergray) with screen blend mode over var(--color-1-nearblack) background
				}

				&:disabled {
					mix-blend-mode: normal;
					z-index: 0;

					&::-webkit-slider-thumb {
						background: var(--color-4-dimgray);
					}
				}

				// Firefox
				&::-moz-range-thumb {
					border: none;
					border-radius: 2px;
					width: 4px;
					height: 24px;
					background: #494949; // Becomes var(--color-5-dullgray) with screen blend mode over var(--color-1-nearblack) background
				}

				&:hover::-moz-range-thumb {
					background: #5b5b5b; // Becomes var(--color-6-lowergray) with screen blend mode over var(--color-1-nearblack) background
				}

				&:hover ~ .slider-progress::before {
					background: var(--color-3-darkgray);
				}

				&::-moz-range-track {
					height: 0;
				}
			}

			// This fake slider thumb stays in the location of the real thumb while we have to hide the real slider between mousedown and mouseup or mousemove.
			// That's because the range input element moves to the pressed location immediately upon mousedown, but we don't want to show that yet.
			// Instead, we want to wait until the user does something:
			// Releasing the mouse means we reset the slider to its previous location, thus canceling the slider move. In that case, we focus the text entry.
			// Moving the mouse left/right means we have begun dragging, so then we hide this fake one and continue showing the actual drag of the real slider.
			.fake-slider-thumb {
				position: absolute;
				left: 2px;
				right: 2px;
				top: 0;
				bottom: 0;
				z-index: 2;
				mix-blend-mode: screen;
				pointer-events: none;

				&::before {
					content: "";
					position: absolute;
					border-radius: 2px;
					margin-left: -2px;
					left: calc(var(--progress-factor) * 100%);
					width: 4px;
					height: 24px;
					background: #5b5b5b; // Becomes var(--color-6-lowergray) with screen blend mode over var(--color-1-nearblack) background
				}
			}

			.slider-progress {
				position: absolute;
				top: 2px;
				bottom: 2px;
				left: 2px;
				right: 2px;
				pointer-events: none;

				&::before {
					content: "";
					position: absolute;
					top: 0;
					left: 0;
					width: calc(var(--progress-factor) * 100% - 2px);
					height: 100%;
					background: var(--color-2-mildblack);
					border-radius: 1px 0 0 1px;
				}
			}
		}
	}
</style>
