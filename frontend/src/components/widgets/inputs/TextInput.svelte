<script lang="ts">
	import FieldInput from "@graphite/components/widgets/inputs/FieldInput.svelte";

	interface Props {
		// Label
		label?: string | undefined;
		tooltip?: string | undefined;
		placeholder?: string | undefined;
		// Disabled
		disabled?: boolean;
		// Value
		value: string;
		// Styling
		centered?: boolean;
		minWidth?: number;
		class?: string;
		classes?: Record<string, boolean>;
		oncommitText?: (arg1: string) => void;
	}

	let {
		label = undefined,
		tooltip = undefined,
		placeholder = undefined,
		disabled = false,
		value = $bindable(),
		centered = false,
		minWidth = 0,
		class: className = "",
		classes = {},
		oncommitText,
	}: Props = $props();

	let self: FieldInput | undefined = $state();
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

	export function element(): HTMLInputElement | HTMLTextAreaElement | undefined {
		return self?.element();
	}
</script>

<FieldInput
	class={`text-input ${className}`.trim()}
	classes={{ centered, ...classes }}
	styles={{ ...(minWidth > 0 ? { "min-width": `${minWidth}px` } : {}) }}
	bind:value
	onfocus={onTextFocused}
	onchange={onTextChanged}
	ontextChangeCanceled={onTextChangeCanceled}
	spellcheck={true}
	{label}
	{disabled}
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
