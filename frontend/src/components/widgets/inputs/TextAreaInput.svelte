<script lang="ts">
	import FieldInput from "@graphite/components/widgets/inputs/FieldInput.svelte";

	type Props = {
		value: string;
		label?: string | undefined;
		tooltip?: string | undefined;
		disabled?: boolean;
		oncommitText?: (arg1: string) => void;
	};

	let { value = $bindable(), label = undefined, tooltip = undefined, disabled = false, oncommitText }: Props = $props();

	let self: FieldInput | undefined = $state();
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
		if (self) oncommitText?.(self.getValue());

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
	bind:value
	onfocus={onTextFocused}
	onchange={onTextChanged}
	ontextChangeCanceled={onTextChangeCanceled}
	textarea={true}
	spellcheck={true}
	{label}
	{disabled}
	{tooltip}
	bind:this={self}
/>

<style lang="scss" global>
</style>
