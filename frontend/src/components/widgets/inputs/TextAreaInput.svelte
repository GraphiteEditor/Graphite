<script lang="ts">
	import { createEventDispatcher } from "svelte";

	import FieldInput from "@graphite/components/widgets/inputs/FieldInput.svelte";

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
		// The `unFocus()` call in `onTextChangeCanceled()` causes itself to be run again, so this if statement skips a second run
		if (!editing) return;

		onTextChangeCanceled();

		// TODO: Find a less hacky way to do this
		if (self) dispatch("commitText", self.getValue());

		// Required if value is not changed by the parent component upon update:value event
		self?.setInputElementValue(self.getValue());
	}

	function onTextChangeCanceled() {
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
	on:textChangeCanceled={onTextChangeCanceled}
	textarea={true}
	spellcheck={true}
	{label}
	{disabled}
	{tooltip}
	bind:this={self}
/>

<style lang="scss" global>
</style>
