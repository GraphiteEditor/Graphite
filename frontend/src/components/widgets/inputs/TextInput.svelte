<script lang="ts">
	import FieldInput from "@graphite/components/widgets/inputs/FieldInput.svelte";

	type Props = {
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
	};

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

	function onTextFocused() {
		self?.selectAllText(value);
	}

	// Called only when `value` is changed from the <input> element via user input and committed, either with the
	// enter key (via the `change` event) or when the <input> element is unfocused (with the `blur` event binding)
	function onTextChanged(commitValue: string) {
		value = commitValue;

		oncommitText?.(commitValue);
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
	oncommitText={oncommitText ? onTextChanged : undefined}
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
