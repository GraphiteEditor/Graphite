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

	// Called only when `value` is changed from the <textarea> element via user input and committed, either
	// via the `change` event or when the <input> element is unfocused (with the `blur` event binding)
	function onTextChanged(commitValue: string) {
		value = commitValue;

		oncommitText?.(commitValue);
	}

	export function focus() {
		self?.focus();
	}
</script>

<FieldInput class="text-area-input" classes={{ "has-label": Boolean(label) }} {value} oncommitText={onTextChanged} textarea={true} spellcheck={true} {label} {disabled} {tooltip} bind:this={self} />

<style lang="scss" global>
</style>
