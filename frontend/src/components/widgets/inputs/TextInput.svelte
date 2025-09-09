<script lang="ts">
	import { createEventDispatcher } from "svelte";

	import FieldInput from "@graphite/components/widgets/inputs/FieldInput.svelte";

	const dispatch = createEventDispatcher<{ commitText: string }>();

	// Label
	export let label: string | undefined = undefined;
	export let tooltip: string | undefined = undefined;
	export let placeholder: string | undefined = undefined;
	// Disabled
	export let disabled = false;
	// Narrow
	export let narrow = false;
	// Value
	export let value: string;
	// Styling
	export let centered = false;
	export let minWidth = 0;
	export let maxWidth = 0;

	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};

	let self: FieldInput | undefined;
	let editing = false;

	function onTextFocused() {
		editing = true;

		self?.selectAllText(value);
	}

	// Called only when `value` is changed from the <input> element via user input and committed, either with the
	// enter key (via the `change` event) or when the <input> element is unfocused (with the `blur` event binding)
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

	export function element(): HTMLInputElement | HTMLTextAreaElement | undefined {
		return self?.element();
	}
</script>

<FieldInput
	class={`text-input ${className}`.trim()}
	classes={{ centered, ...classes }}
	styles={{
		...(minWidth > 0 ? { "min-width": `${minWidth}px` } : {}),
		...(maxWidth > 0 ? { "max-width": `${maxWidth}px` } : {}),
	}}
	{value}
	on:value
	on:textFocused={onTextFocused}
	on:textChanged={onTextChanged}
	on:textChangeCanceled={onTextChangeCanceled}
	spellcheck={true}
	{label}
	{disabled}
	{narrow}
	{tooltip}
	{placeholder}
	bind:this={self}
/>

<style lang="scss" global>
	.text-input {
		flex-shrink: 0;

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
