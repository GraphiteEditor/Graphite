<script lang="ts">
	import { createEventDispatcher, onMount, onDestroy } from "svelte";

	import { type NumberInputMode, type NumberInputIncrementBehavior } from "@graphite/wasm-communication/messages";
	import { evaluateMathExpression } from "@graphite-frontend/wasm/pkg/graphite_wasm.js";

	import FieldInput from "@graphite/components/widgets/inputs/FieldInput.svelte";

	const BUTTONS_LEFT = 0b0000_0001;
	const BUTTONS_RIGHT = 0b0000_0010;
	const BUTTON_LEFT = 0;
	const BUTTON_RIGHT = 2;

	const dispatch = createEventDispatcher<{ value: number | undefined; startHistoryTransaction: undefined }>();

	// Label
	export let label: string | undefined = undefined;
	export let tooltip: string | undefined = undefined;

	// Disabled
	export let disabled = false;

	// Value
	// When `value` is not provided (i.e. it's `undefined`), a dash is displayed.
	export let value: number | undefined = undefined; // NOTE: Do not update this directly, do so by calling `updateValue()` instead.
	export let min: number | undefined = undefined;
	export let max: number | undefined = undefined;
	export let isInteger = false;

	// Number presentation
	export let displayDecimalPlaces = 2;
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

	// Callbacks
	export let incrementCallbackIncrease: (() => void) | undefined = undefined;
	export let incrementCallbackDecrease: (() => void) | undefined = undefined;

	let self: FieldInput | undefined;
	let inputRangeElement: HTMLInputElement | undefined;
	let text = displayText(value);
	let editing = false;
	// Stays in sync with a binding to the actual input range slider element.
	let rangeSliderValue = value !== undefined ? value : 0;
	// Value used to render the position of the fake slider when applicable, and length of the progress colored region to the slider's left.
	// This is the same as `rangeSliderValue` except in the "Deciding" state, when it has the previous location before the user's mousedown.
	let rangeSliderValueAsRendered = value !== undefined ? value : 0;
	// Keeps track of the state of the slider drag as the user transitions through steps of the input process.
	// - "Ready": no interaction is happening.
	// - "Deciding": the user has pressed down the mouse and might next decide to either drag left/right or release without dragging.
	// - "Dragging": the user is dragging the slider left/right.
	// - "Aborted": the user has right clicked or pressed Escape to abort the drag, but hasn't yet released all mouse buttons.
	let rangeSliderClickDragState: "Ready" | "Deciding" | "Dragging" | "Aborted" = "Ready";
	// Stores the initial value upon beginning to drag so it can be restored upon aborting. Set to `undefined` when not dragging.
	let initialValueBeforeDragging: number | undefined = undefined;
	// Stores the total value change during the process of dragging the slider. Set to 0 when not dragging.
	let cumulativeDragDelta = 0;
	// Track whether the Ctrl key is currently held down.
	let ctrlKeyDown = false;

	$: watchValue(value);

	$: sliderStepValue = isInteger ? (step === undefined ? 1 : step) : "any";
	$: styles = {
		...(minWidth > 0 ? { "min-width": `${minWidth}px` } : {}),
		...(mode === "Range" ? { "--progress-factor": Math.min(Math.max((rangeSliderValueAsRendered - rangeMin) / (rangeMax - rangeMin), 0), 1) } : {}),
	};

	// Keep track of the Ctrl key being held down.
	const trackCtrl = (e: KeyboardEvent | MouseEvent) => (ctrlKeyDown = e.ctrlKey);
	onMount(() => {
		addEventListener("keydown", trackCtrl);
		addEventListener("keyup", trackCtrl);
		addEventListener("mousemove", trackCtrl);
	});
	onDestroy(() => {
		removeEventListener("keydown", trackCtrl);
		removeEventListener("keyup", trackCtrl);
		removeEventListener("mousemove", trackCtrl);
	});

	// ===============================
	// TRACKING AND UPDATING THE VALUE
	// ===============================

	// Called only when `value` is changed from outside this component.
	function watchValue(value: number | undefined) {
		// Don't update if the slider is currently being dragged (we don't want the backend fighting with the user's drag)
		if (rangeSliderClickDragState === "Dragging") return;

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

		text = displayText(sanitized);
	}

	// Called internally to update the value indirectly by informing the parent component of the new value,
	// so it can update the prop for this component, finally yielding the value change.
	function updateValue(newValue: number | undefined): number | undefined {
		// Check if the new value is valid, otherwise we use the old value (rounded if it's an integer)
		const oldValue = value !== undefined && isInteger ? Math.round(value) : value;
		let newValueValidated = newValue !== undefined ? newValue : oldValue;

		if (newValueValidated !== undefined) {
			if (typeof min === "number" && !Number.isNaN(min)) newValueValidated = Math.max(newValueValidated, min);
			if (typeof max === "number" && !Number.isNaN(max)) newValueValidated = Math.min(newValueValidated, max);

			if (isInteger) newValueValidated = Math.round(newValueValidated);

			rangeSliderValue = newValueValidated;
			rangeSliderValueAsRendered = newValueValidated;
		}

		text = displayText(newValueValidated);

		if (newValue !== undefined) dispatch("value", newValueValidated);

		// For any caller that needs to know what the value was changed to, we return it here
		return newValueValidated;
	}

	// ================
	// HELPER FUNCTIONS
	// ================

	function displayText(displayValue: number | undefined): string {
		if (displayValue === undefined) return "-";

		const roundingPower = 10 ** Math.max(displayDecimalPlaces, 0);

		const unitlessDisplayValue = Math.round(displayValue * roundingPower) / roundingPower;
		return `${unitlessDisplayValue}${unPluralize(unit, displayValue)}`;
	}

	function unPluralize(unit: string, quantity: number): string {
		if (quantity !== 1 || !unit.endsWith("s")) return unit;
		return unit.slice(0, -1);
	}

	// ===========================
	// ALL MODES: TEXT VALUE ENTRY
	// ===========================

	function onTextFocused() {
		if (value === undefined) text = "";
		else if (unitIsHiddenWhenEditing) text = String(value);
		else text = `${value}${unPluralize(unit, value)}`;

		editing = true;

		self?.selectAllText(text);
	}

	// Called only when `value` is changed from the <input> element via user input and committed, either with the
	// enter key (via the `change` event) or when the <input> element is unfocused (with the `blur` event binding).
	function onTextChanged() {
		// The `unFocus()` call at the bottom of this function and in `onTextChangeCanceled()` causes this function to be run again, so this check skips a second run.
		if (!editing) return;

		// Insert a leading zero before all decimal points lacking a preceding digit, since the library doesn't realize that "point" means "zero point".
		const textWithLeadingZeroes = text.replaceAll(/(?<=^|[^0-9])\./g, "0."); // Match any "." that is preceded by the start of the string (^) or a non-digit character ([^0-9])

		let newValue = evaluateMathExpression(textWithLeadingZeroes);
		if (newValue !== undefined && isNaN(newValue)) newValue = undefined; // Rejects `sqrt(-1)`
		updateValue(newValue);

		editing = false;
		self?.unFocus();
	}

	function onTextChangeCanceled() {
		updateValue(undefined);

		const valueOrZero = value !== undefined ? value : 0;
		rangeSliderValue = valueOrZero;
		rangeSliderValueAsRendered = valueOrZero;

		editing = false;

		self?.unFocus();
	}

	// =============================
	// INCREMENT MODE: ARROW BUTTONS
	// =============================

	function onIncrement(direction: "Decrease" | "Increase") {
		if (value === undefined) return;

		const actions: Record<NumberInputIncrementBehavior, () => void> = {
			Add: () => {
				const directionAddend = direction === "Increase" ? step : -step;
				const newValue = value !== undefined ? value + directionAddend : undefined;
				updateValue(newValue);
			},
			Multiply: () => {
				const directionMultiplier = direction === "Increase" ? step : 1 / step;
				const newValue = value !== undefined ? value * directionMultiplier : undefined;
				updateValue(newValue);
			},
			Callback: () => {
				if (direction === "Increase") incrementCallbackIncrease?.();
				if (direction === "Decrease") incrementCallbackDecrease?.();
			},
			None: () => {},
		};
		actions[incrementBehavior]();
	}

	// =======================================
	// INCREMENT MODE: DRAGGING LEFT AND RIGHT
	// =======================================

	// TODO: Prevent right clicking the input field from focusing it (i.e. entering its text editing state).
	// TODO: `preventDefault()` doesn't work. Relevant StackOverflow question without any working answers:
	// TODO: <https://stackoverflow.com/questions/60746390/react-prevent-right-click-from-focusing-an-otherwise-focusable-element>
	// TODO: Another potential solution is to somehow track if the user right clicked, then use the "focus" event handler to immediately return
	// TODO: focus to the previously focused element with `e.relatedTarget.focus();`. But a "FocusEvent" doesn't include mouse button click info.
	// TODO: Alternatively, we could stick an element in front of the input field that blocks clicks on the underlying input field. Then it could
	// TODO: call `.focus()` on the input field when left clicked and then hide itself so it doesn't block the input field while being edited.

	function onDragPointerDown(e: PointerEvent) {
		// Only drag the number with left click (and when it's valid to do so)
		if (e.button !== BUTTON_LEFT || mode !== "Increment" || value === undefined || disabled) return;

		// Don't drag the text value from is input element
		e.preventDefault();

		// Now we need to wait and see if the user follows this up with a mousemove or mouseup.

		// For some reason, both events can get fired before their event listeners are removed, so we need to guard against both running.
		let alreadyActedGuard = false;

		// If it's a mousemove, we'll enter the dragging state and begin dragging.
		const onMove = () => {
			if (alreadyActedGuard) return;
			alreadyActedGuard = true;

			beginDrag(e);
			removeEventListener("pointermove", onMove);
		};
		// If it's a mouseup, we'll begin editing the text field.
		const onUp = () => {
			if (alreadyActedGuard) return;
			alreadyActedGuard = true;

			self?.focus();
			removeEventListener("pointerup", onUp);
		};
		addEventListener("pointermove", onMove);
		addEventListener("pointerup", onUp);
	}

	function beginDrag(e: PointerEvent) {
		// Get the click target
		const target = e.target || undefined;
		if (!(target instanceof HTMLElement)) return;

		// Enter dragging state
		target.requestPointerLock();
		initialValueBeforeDragging = value;
		cumulativeDragDelta = 0;

		// Tell the backend that we are beginning a transaction for the history system
		startDragging();

		// We ignore the first event invocation's `e.movementX` value because it's unreliable.
		// In both Chrome and Firefox (tested on Windows 10), the first `e.movementX` value is occasionally a very large number
		// (around positive 1000, even if movement was in the negative direction). This seems to happen more often if the movement is rapid.
		let ignoredFirstMovement = false;

		const pointerUp = () => {
			// Confirm on release by setting the reset value to the current value, so once the pointer lock ends,
			// the value is set to itself instead of the initial (abort) value in the "pointerlockchange" event handler function.
			initialValueBeforeDragging = value;
			cumulativeDragDelta = 0;

			document.exitPointerLock();
		};
		const pointerMove = (e: PointerEvent) => {
			// Abort the drag if right click is down. This works here because a "pointermove" event is fired when right clicking even if the cursor didn't move.
			if (e.buttons & BUTTONS_RIGHT) {
				document.exitPointerLock();
				return;
			}

			// If no buttons are down, we are stuck in the drag state after having released the mouse, so we should exit.
			if (e.buttons === 0) {
				document.exitPointerLock();
				return;
			}

			// Calculate and then update the dragged value offset, slowed down by 10x when Shift is held.
			if (ignoredFirstMovement && initialValueBeforeDragging !== undefined) {
				const CHANGE_PER_DRAG_PX = 0.1;
				const CHANGE_PER_DRAG_PX_SLOW = CHANGE_PER_DRAG_PX / 10;

				const dragDelta = e.movementX * (e.shiftKey ? CHANGE_PER_DRAG_PX_SLOW : CHANGE_PER_DRAG_PX);
				cumulativeDragDelta += dragDelta;

				const combined = initialValueBeforeDragging + cumulativeDragDelta;
				const combineSnapped = e.ctrlKey ? Math.round(combined) : combined;

				const newValue = updateValue(combineSnapped);

				// If the value was altered within the `updateValue()` call, we need to rectify the cumulative drag delta to account for the change.
				if (newValue !== undefined) cumulativeDragDelta -= combineSnapped - newValue;
			}
			ignoredFirstMovement = true;
		};
		const pointerLockChange = () => {
			// Do nothing if we just entered, rather than exited, pointer lock.
			if (document.pointerLockElement) return;

			// Reset the value to the initial value if the drag was aborted, or to the current value if it was just confirmed by changing the initial value to the current value.
			updateValue(initialValueBeforeDragging);
			initialValueBeforeDragging = undefined;
			cumulativeDragDelta = 0;

			// Clean up the event listeners.
			removeEventListener("pointerup", pointerUp);
			removeEventListener("pointermove", pointerMove);
			document.removeEventListener("pointerlockchange", pointerLockChange);
		};

		addEventListener("pointerup", pointerUp);
		addEventListener("pointermove", pointerMove);
		document.addEventListener("pointerlockchange", pointerLockChange);
	}

	// ===============================
	// RANGE MODE: DRAGGING THE SLIDER
	// ===============================

	// Called by the range slider's "input" event which fires continuously while the user is dragging the slider.
	// It also fires once when the user clicks the slider, causing its position to jump to the clicked X position.
	// Firefox also likes to fire this event more liberally, and it likes to make this event happen after most others,
	// which is why the logic for this feature has to be pretty complicated to ensure robustness even across unexpected event ordering.
	//
	// Summary:
	// - Do nothing if the user is still dragging with left click after aborting with right click or escape
	// - If this is the first "input" event upon mousedown, manage the state so we end up waiting for the user to decide on a subsequent action:
	//     - Right clicking or pressing Escape means we abort from the "Deciding" state and wait for the user to release all mouse buttons before respecting any further input
	//     - Releasing the click without dragging means we focus the text input element to edit the number field
	//     - Dragging the slider means we commit to dragging the slider, so we begin watching for an abort from that state (and continue onto the next bullet point below)
	// - If the user has committed to dragging the slider, so we update this widget's value.
	function onSliderInput() {
		// Exit early if the slider is disabled by having been aborted by the user.
		if (rangeSliderClickDragState === "Aborted") {
			// If we've just aborted the drag by right clicking, but the user hasn't yet released the left mouse button, Firefox treats
			// some subsequent interactions with the slider (like that right mouse button release, or maybe mouse movement in some cases)
			// as input changes to the slider position. Thus, until we leave the "Aborted" state by releasing all mouse buttons,
			// we have to set the slider position back to currently intended value to fight against Firefox's attempts to let the user move it.
			updateValue(rangeSliderValueAsRendered);

			// Now we exit early because we're ignoring further user input until the user releases all mouse buttons, which gets us back to the "Ready" state.
			return;
		}

		// Keep only 4 digits after the decimal point.
		const ROUNDING_EXPONENT = 4;
		const ROUNDING_MAGNITUDE = 10 ** ROUNDING_EXPONENT;
		const roundedValue = Math.round(rangeSliderValue * ROUNDING_MAGNITUDE) / ROUNDING_MAGNITUDE;

		// Exit if this is an extraneous event invocation that occurred after mouseup, which happens in Firefox.
		if (value !== undefined && Math.abs(value - roundedValue) < 1 / ROUNDING_MAGNITUDE) {
			return;
		}

		// Snap the slider value to the nearest integer if the Ctrl key is held, or the widget is set to integer mode.
		const snappedValue = ctrlKeyDown || isInteger ? Math.round(roundedValue) : roundedValue;

		// The first "input" event upon mousedown means we transition to a "Deciding" state, allowing us to wait for the
		// next event to determine if the user is dragging (to slide the slider) or releasing (to edit the numerical text field).
		if (rangeSliderClickDragState === "Ready") {
			// We're in the "Deciding" state now, which means we're waiting for the user to either drag or release the slider.
			rangeSliderClickDragState = "Deciding";

			// We want to render the fake slider thumb at the old position, which is still the number held by `value`.
			rangeSliderValueAsRendered = value || 0;

			// We also store this initial value so we can restore it if the user aborts the drag.
			initialValueBeforeDragging = value;

			// We want to allow the user to right click to abort from this "Deciding" state so the slider isn't stuck waiting for either a drag or release.
			addEventListener("mousedown", sliderAbortFromMousedown);
			addEventListener("keydown", sliderAbortFromMousedown);

			// Exit early because we don't want to use the value set by where on the track the user pressed.
			return;
		}

		// Now that we've past the point of entering the "Deciding" state in this subsequent invocation, we know that the user has
		// either dragged or released the mouse so we can stop watching for a right click to abort from that short point in the process.
		removeEventListener("mousedown", sliderAbortFromMousedown);
		removeEventListener("keydown", sliderAbortFromMousedown);

		// If the subsequent event upon entering the "Deciding" state is this slider "input" event caused by the user dragging it, that means the user has
		// committed to dragging the slider (instead of alternatively deciding on releasing it to edit the text field, or aborting with Escape/right click).
		if (rangeSliderClickDragState === "Deciding") {
			// We're dragging now, so that's the new state.
			rangeSliderClickDragState = "Dragging";

			// Tell the backend that we are beginning a transaction for the history system
			startDragging();

			// We want to begin watching for an abort while dragging the slider.
			addEventListener("pointermove", sliderAbortFromDragging);
			addEventListener("keydown", sliderAbortFromDragging);

			// Since we've committed to dragging the slider, we want to use the new slider value. Continue to the logic below.
		}

		// If we're in a dragging state, we want to use the new slider value.
		rangeSliderValueAsRendered = snappedValue;
		updateValue(snappedValue);
	}

	// This handles the user releasing all mouse buttons after clicking (and potentially dragging) the slider.
	// If the slider wasn't dragged, we focus the text input element to begin editing the number field.
	// Then, regardless of the above, we clean up the state and event listeners.
	// This is called by the range slider's "pointerup" event bound in the HTML template.
	function onSliderPointerUp() {
		// User clicked but didn't drag, so we focus the text input element.
		if (rangeSliderClickDragState === "Deciding") {
			const inputElement = self?.element();
			if (!inputElement) return;

			// Set the slider position back to the original position to undo the user moving it.
			rangeSliderValue = rangeSliderValueAsRendered;

			// Begin editing the number text field.
			inputElement.focus();

			// In the next step, we'll switch back to the neutral state so that after the user is done editing the text field, the process can begin anew.
		}

		// Since the user decided to release the slider, we reset to the neutral state so the user can begin the process anew.
		// But if the slider was aborted, we don't want to reset the state because we're still waiting for the user to release all mouse buttons.
		if (rangeSliderClickDragState !== "Aborted") {
			rangeSliderClickDragState = "Ready";
		}

		// Clean up the event listeners that were for tracking an abort while dragging the slider, now that we're no longer dragging it.
		removeEventListener("mousedown", sliderAbortFromMousedown);
		removeEventListener("keydown", sliderAbortFromMousedown);
		removeEventListener("pointermove", sliderAbortFromDragging);
		removeEventListener("keydown", sliderAbortFromDragging);
	}

	function startDragging() {
		// This event is sent to the backend so it knows to start a transaction for the history system. See discussion for some explanation:
		// <https://github.com/GraphiteEditor/Graphite/pull/1584#discussion_r1477592483>
		dispatch("startHistoryTransaction");
	}

	// We want to let the user abort while dragging the slider by right clicking or pressing Escape.
	// This function also helps recover and clean up if the window loses focus while dragging the slider.
	// Since we reuse the function for both the "pointermove" and "keydown" events, it is split into parts that only run for a `PointerEvent` or `KeyboardEvent`.
	function sliderAbortFromDragging(e: PointerEvent | KeyboardEvent) {
		// Logic for aborting from pressing Escape.
		if (e instanceof KeyboardEvent) {
			// Detect if the user pressed Escape and abort the slider drag.
			if (e.key === "Escape") {
				// Call the abort helper function.
				sliderAbort();
			}
		}

		// Logic for aborting from a right click.
		// Detect if a right click has occurred and abort the slider drag.
		// This handler's "pointermove" event will be fired upon right click even if the cursor didn't move, which is why it's okay to check in this event handler.
		if (e instanceof PointerEvent && e.buttons & BUTTONS_RIGHT) {
			// Call the abort helper function
			sliderAbort();
		}

		// Recovery from the window losing focus while dragging the slider.
		// If somehow the user moved the pointer while not left click-dragging the slider, we know that we were stuck in the "Deciding" state, so we recover and clean up.
		// This could happen while dragging the slider and using a hotkey to tab away to another window or browser tab, then returning to the stuck state.
		if (e instanceof PointerEvent && !(e.target === inputRangeElement && e.buttons & BUTTONS_LEFT)) {
			// Switch back to the neutral state.
			rangeSliderClickDragState = "Ready";

			// Remove the "pointermove" and "keydown" event listeners that are for tracking an abort while
			// dragging the slider, now that we're no longer dragging it due to the loss of window focus.
			removeEventListener("pointermove", sliderAbortFromDragging);
			removeEventListener("keydown", sliderAbortFromDragging);
		}
	}

	// We want to let the user abort immediately after clicking the slider, but not yet deciding to drag or release.
	// During this momentary step, the slider hasn't moved yet but we want to allow aborting from this limbo state.
	function sliderAbortFromMousedown(e: MouseEvent | KeyboardEvent) {
		// Logic for aborting from a right click or pressing Escape.
		if ((e instanceof KeyboardEvent && e.key === "Escape") || (e instanceof MouseEvent && e.button === BUTTON_RIGHT)) {
			// Call the abort helper function
			sliderAbort();

			// Clean up these event listeners because they were for getting us into this function and now we're done with them.
			removeEventListener("mousedown", sliderAbortFromMousedown);
			removeEventListener("keydown", sliderAbortFromMousedown);
		}
	}

	// Helper function that performs the state management and cleanup for aborting the slider drag.
	function sliderAbort() {
		// End the user's drag by instantaneously disabling and re-enabling the range input element
		if (inputRangeElement) inputRangeElement.disabled = true;
		setTimeout(() => {
			if (inputRangeElement) inputRangeElement.disabled = false;
		}, 0);

		// Set the value back to the original value before the user began dragging.
		if (initialValueBeforeDragging !== undefined) {
			rangeSliderValueAsRendered = initialValueBeforeDragging;
			updateValue(initialValueBeforeDragging);
		}

		// Set the state to "Aborted" so we can ignore further user input until the user releases all mouse buttons.
		rangeSliderClickDragState = "Aborted";

		// Detect when all mouse buttons are released so we can exit the "Aborted" state and return to the "Ready" state.
		// (The "pointerup" event is defined as firing only upon all mouse buttons being released, which is what we need here.)
		const sliderResetAbort = () => {
			// Switch back to the neutral state so the user can begin the process anew.
			// We do this inside setTimeout() to delay this until after Firefox has fired its extraneous "input" event after this "pointerup" event.
			//
			// A delay of 0 seems to be sufficient, but if the bug persists, we can try increasing the delay. The bug is reproduced in Firefox by
			// dragging the slider, hitting Escape, then releasing the mouse button. This results in being transferred by `onSliderInput()` to the
			// "Deciding" state when we should remain in the "Ready" state as set here. (For debugging, this can be visualized in CSS by
			// recoloring the fake slider handle, which is shown in the "Deciding" state.)
			setTimeout(() => {
				rangeSliderClickDragState = "Ready";
			}, 0);

			// Clean up the event listener that was used to call this function.
			removeEventListener("pointerup", sliderResetAbort);
		};
		addEventListener("pointerup", sliderResetAbort);

		// Clean up the event listeners that were for tracking an abort while dragging the slider, now that we're no longer dragging it.
		removeEventListener("pointermove", sliderAbortFromDragging);
		removeEventListener("keydown", sliderAbortFromDragging);
	}
</script>

<FieldInput
	class={"number-input"}
	classes={{
		increment: mode === "Increment",
		range: mode === "Range",
	}}
	value={text}
	on:value={({ detail }) => (text = detail)}
	on:textFocused={onTextFocused}
	on:textChanged={onTextChanged}
	on:textChangeCanceled={onTextChangeCanceled}
	on:pointerdown={onDragPointerDown}
	{label}
	{disabled}
	{tooltip}
	{styles}
	hideContextMenu={true}
	spellcheck={false}
	bind:this={self}
>
	{#if value !== undefined}
		{#if mode === "Increment" && incrementBehavior !== "None"}
			<button class="arrow left" on:click={() => onIncrement("Decrease")} tabindex="-1" />
			<button class="arrow right" on:click={() => onIncrement("Increase")} tabindex="-1" />
		{/if}
		{#if mode === "Range"}
			<input
				type="range"
				tabindex="-1"
				class="slider"
				class:hidden={rangeSliderClickDragState === "Deciding"}
				{disabled}
				min={rangeMin}
				max={rangeMax}
				step={sliderStepValue}
				bind:value={rangeSliderValue}
				on:input={onSliderInput}
				on:pointerup={onSliderPointerUp}
				on:contextmenu|preventDefault
				on:wheel={(e) => /* Stops slider eating the scroll event in Firefox */ e.target instanceof HTMLInputElement && e.target.blur()}
				bind:this={inputRangeElement}
			/>
			{#if rangeSliderClickDragState === "Deciding"}
				<div class="fake-slider-thumb" />
			{/if}
			<div class="slider-progress" />
		{/if}
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

			// Keep the right-aligned input element from overlapping the increment arrow on the right
			input[type="text"]:not(:focus).has-label {
				margin-right: 16px;
			}

			// Hide the increment arrows when entering text, disabled, or not hovered
			input[type="text"]:focus ~ .arrow,
			&.disabled .arrow,
			&:not(:hover) .arrow {
				display: none;
			}

			// Show the left-right arrow cursor when hovered over the draggable area
			&:not(.disabled) input[type="text"]:not(:focus),
			&:not(.disabled) label {
				cursor: ew-resize;
			}

			// Style the decrement/increment arrows
			.arrow {
				position: absolute;
				top: 0;
				margin: 0;
				padding: 9px 0;
				border: none;
				border-radius: 2px;
				background: none;

				&:hover {
					background: var(--color-4-dimgray);
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

				&:disabled {
					mix-blend-mode: normal;
					z-index: 0;
				}

				&:hover ~ .slider-progress::before {
					background: var(--color-3-darkgray);
				}

				// Chromium and Safari
				&::-webkit-slider-thumb {
					-webkit-appearance: none; // Required until Safari 15.4 (Graphite supports 15.0+)
					appearance: none;
					border-radius: 2px;
					width: 4px;
					height: 22px;
					background: #494949; // Becomes var(--color-5-dullgray) with screen blend mode over var(--color-1-nearblack) background
				}

				&:hover::-webkit-slider-thumb {
					background: #5b5b5b; // Becomes var(--color-6-lowergray) with screen blend mode over var(--color-1-nearblack) background
				}

				&:disabled::-webkit-slider-thumb {
					background: var(--color-4-dimgray);
				}

				// Firefox
				&::-moz-range-thumb {
					border: none;
					border-radius: 2px;
					width: 4px;
					height: 22px;
					background: #494949; // Becomes var(--color-5-dullgray) with screen blend mode over var(--color-1-nearblack) background
				}

				&:hover::-moz-range-thumb {
					background: #5b5b5b; // Becomes var(--color-6-lowergray) with screen blend mode over var(--color-1-nearblack) background
				}

				&:disabled::-moz-range-thumb {
					background: var(--color-4-dimgray);
				}

				&::-moz-range-track {
					height: 0;
				}
			}

			// This fake slider thumb stays in the location of the real thumb while we have to hide the real slider between mousedown and mouseup or mousemove.
			// That's because the range input element moves to the pressed location immediately upon mousedown, but we don't want to show that yet.
			// Instead, we want to wait until the user does something:
			// - Releasing the mouse means we reset the slider to its previous location, thus canceling the slider move. In that case, we focus the text entry.
			// - Moving the mouse left/right means we have begun dragging, so then we hide this fake one and continue showing the actual drag of the real slider.
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
					width: 4px;
					height: 22px;
					top: 1px;
					left: calc(var(--progress-factor) * 100%);
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
