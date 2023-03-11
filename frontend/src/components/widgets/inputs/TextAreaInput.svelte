<script lang="ts">
	import { createEventDispatcher } from "svelte";

	import FieldInput from "~/src/components/widgets/inputs/FieldInput.svelte";

	// emits: ["update:value", "commitText"],
	const dispatch = createEventDispatcher<{ commitText: string }>();

	export let value: string;
	export let label: string | undefined = undefined;
	export let tooltip: string | undefined = undefined;
	export let disabled = false;

	let self: FieldInput | undefined;
	let editing = false;

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
		if (self) dispatch("commitText", self.getValue());

		// Required if value is not changed by the parent component upon update:value event
		self?.setInputElementValue(self.getValue());
	}

	function onCancelTextChange() {
		editing = false;

		self?.unFocus();
	}

	export function focus() {
		self?.focus();
	}
</script>

<FieldInput
	class="text-area-input"
	classes={{ "has-label": Boolean(label) }}
	{value}
	on:value
	on:textFocused={onTextFocused}
	on:textChanged={onTextChanged}
	on:cancelTextChange={onCancelTextChange}
	textarea={true}
	spellcheck={true}
	{label}
	{disabled}
	{tooltip}
	bind:this={self}
/>

<style lang="scss" global>
</style>
