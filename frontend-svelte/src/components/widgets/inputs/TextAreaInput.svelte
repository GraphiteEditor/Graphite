<script lang="ts">
	import { createEventDispatcher } from "svelte";

	import FieldInput from "@/components/widgets/inputs/FieldInput.svelte";

	// emits: ["update:value", "commitText"],

	export let value: string;
	export let label: string | undefined = undefined;
	export let tooltip: string | undefined = undefined;
	export let disabled = false;

	let fieldInput: FieldInput;
	let editing = false;
	let inputValue = value;

	$: createEventDispatcher("update:value", inputValue);

	function onTextFocused() {
		editing = true;
	}

	// Called only when `value` is changed from the <textarea> element via user input and committed, either
	// via the `change` event or when the <input> element is unfocused (with the `blur` event binding)
	function onTextChanged() {
		// The `unFocus()` call in `onCancelTextChange()` causes itself to be run again, so this if statement skips a second run
		if (!editing) return;

		onCancelTextChange();

		// TODO: Find a less hacky way to do this
		createEventDispatcher("commitText", fieldInput.getInputElementValue());

		// Required if value is not changed by the parent component upon update:value event
		fieldInput.setInputElementValue(value);
	}

	function onCancelTextChange() {
		editing = false;

		fieldInput.unFocus();
	}
</script>

<FieldInput
	textarea={true}
	class="text-area-input"
	classes={{
		// TODO: Svelte: check if this should be based on `Boolean(label)` or `label !== ""`
		"has-label": Boolean(label),
	}}
	{label}
	{disabled}
	{tooltip}
	spellcheck={true}
	bind:value={inputValue}
	on:textFocused={onTextFocused}
	on:textChanged={onTextChanged}
	on:cancelTextChange={onCancelTextChange}
	bind:this={fieldInput}
/>

<style lang="scss" global></style>
