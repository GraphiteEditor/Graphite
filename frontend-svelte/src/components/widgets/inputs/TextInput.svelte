<script lang="ts">
	import { createEventDispatcher } from "svelte";

	import FieldInput from "@/components/widgets/inputs/FieldInput.svelte";

	// emits: ["update:value", "commitText"],
	const dispatch = createEventDispatcher<{ value: string; commitText: string }>();

	// Label
	export let label: string | undefined = undefined;
	export let tooltip: string | undefined = undefined;
	export let placeholder: string | undefined = undefined;
	// Disabled
	export let disabled = false;
	// Value
	export let value: string;
	// Styling
	export let centered = false;
	export let minWidth = 0;
	export let sharpRightCorners = false;

	let fieldInput: FieldInput;
	let editing = false;
	let text = value;

	$: dispatch("value", text);

	function onTextFocused() {
		editing = true;

		fieldInput.selectAllText(text);
	}

	// Called only when `value` is changed from the <input> element via user input and committed, either with the
	// enter key (via the `change` event) or when the <input> element is unfocused (with the `blur` event binding)
	function onTextChanged() {
		// The `unFocus()` call in `onCancelTextChange()` causes itself to be run again, so this if statement skips a second run
		if (!editing) return;

		onCancelTextChange();

		// TODO: Find a less hacky way to do this
		dispatch("commitText", fieldInput.getInputElementValue());

		// Required if value is not changed by the parent component upon update:value event
		fieldInput.setInputElementValue(value);
	}

	function onCancelTextChange() {
		editing = false;

		fieldInput.unFocus();
	}
</script>

<FieldInput
	class="text-input"
	classes={{ centered }}
	styles={{ "min-width": minWidth > 0 ? `${minWidth}px` : undefined }}
	value={text}
	on:value={({ detail }) => (text = detail)}
	on:textFocused={onTextFocused}
	on:textChanged={onTextChanged}
	on:cancelTextChange={onCancelTextChange}
	spellcheck={true}
	{label}
	{disabled}
	{tooltip}
	{placeholder}
	{sharpRightCorners}
	bind:this={fieldInput}
/>

<style lang="scss" global>
	.text-input {
		input {
			text-align: left;
		}

		&.centered {
			input:not(:focus) {
				text-align: center;
			}
		}
	}
</style>
