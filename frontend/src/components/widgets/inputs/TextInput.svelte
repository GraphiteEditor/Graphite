<script lang="ts">
	import { createEventDispatcher } from "svelte";

	import FieldInput from "~/src/components/widgets/inputs/FieldInput.svelte";

	// emits: ["update:value", "commitText"],
	const dispatch = createEventDispatcher<{ commitText: string }>();

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

	let self: FieldInput | undefined;
	let editing = false;

	function onTextFocused() {
		editing = true;

		self?.selectAllText(value);
	}

	// Called only when `value` is changed from the <input> element via user input and committed, either with the
	// enter key (via the `change` event) or when the <input> element is unfocused (with the `blur` event binding)
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
	class="text-input"
	classes={{ centered }}
	styles={{ "min-width": minWidth > 0 ? `${minWidth}px` : undefined }}
	{value}
	on:value
	on:textFocused={onTextFocused}
	on:textChanged={onTextChanged}
	on:cancelTextChange={onCancelTextChange}
	spellcheck={true}
	{label}
	{disabled}
	{tooltip}
	{placeholder}
	{sharpRightCorners}
	bind:this={self}
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
