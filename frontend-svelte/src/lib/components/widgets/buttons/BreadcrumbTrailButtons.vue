<template>
	<LayoutRow class="breadcrumb-trail-buttons" :title="tooltip">
		<TextButton
			v-for="(label, index) in labels"
			:key="index"
			:label="label"
			:emphasized="index === labels.length - 1"
			:disabled="disabled"
			:action="() => !disabled && index !== labels.length - 1 && action(index)"
		/>
	</LayoutRow>
</template>

<style lang="scss" global>
.breadcrumb-trail-buttons {
	.text-button {
		position: relative;

		&:not(:first-of-type) {
			border-top-left-radius: 0;
			border-bottom-left-radius: 0;

			&::before {
				content: "";
				position: absolute;
				top: 0;
				left: -4px;
				width: 0;
				height: 0;
				border-style: solid;
				border-width: 12px 0 12px 4px;
				border-color: var(--button-background-color) var(--button-background-color) var(--button-background-color) transparent;
			}
		}

		&:not(:last-of-type) {
			border-top-right-radius: 0;
			border-bottom-right-radius: 0;

			&::after {
				content: "";
				position: absolute;
				top: 0;
				right: -4px;
				width: 0;
				height: 0;
				border-style: solid;
				border-width: 12px 0 12px 4px;
				border-color: transparent transparent transparent var(--button-background-color);
			}
		}

		&:last-of-type {
			// Make this non-functional button not change color on hover
			pointer-events: none;
		}
	}
}
</style>

<script lang="ts">


import LayoutRow from "$lib/components/layout/LayoutRow.svelte";
import TextButton from "$lib/components/widgets/buttons/TextButton.svelte";

export default defineComponent({
	props: {
		labels: { type: Array as PropType<string[]>, required: true },
		disabled: { type: Boolean as PropType<boolean>, default: false },
		tooltip: { type: String as PropType<string | undefined>, required: false },

		// Callbacks
		action: { type: Function as PropType<(index: number) => void>, required: true },
	},
	components: {
		LayoutRow,
		TextButton,
	},
});
</script>
