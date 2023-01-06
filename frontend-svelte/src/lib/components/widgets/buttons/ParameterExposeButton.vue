<template>
	<LayoutRow class="parameter-expose-button">
		<button :class="{ exposed }" :style="{ '--data-type-color': `var(--color-data-${dataType})` }" @click="(e: MouseEvent) => action(e)" :title="tooltip" :tabindex="0"></button>
	</LayoutRow>
</template>

<style lang="scss" global>
.parameter-expose-button {
	display: flex;
	align-items: center;
	flex: 0 0 auto;
	max-height: 24px;

	button {
		flex: 0 0 auto;
		width: 8px;
		height: 8px;
		margin: 0;
		padding: 0;
		border: none;
		border-radius: 50%;

		&:not(.exposed) {
			background: none;
			border: 1px solid var(--data-type-color);

			&:hover {
				background: var(--color-6-lowergray);
			}
		}

		&.exposed {
			background: var(--data-type-color);

			&:hover {
				border: 1px solid var(--color-f-white);
			}
		}
	}
}
</style>

<script lang="ts">


import LayoutRow from "$lib/components/layout/LayoutRow.svelte";

export default defineComponent({
	props: {
		exposed: { type: Boolean as PropType<boolean>, required: true },
		dataType: { type: String as PropType<string>, required: true },
		tooltip: { type: String as PropType<string | undefined>, required: false },

		// Callbacks
		action: { type: Function as PropType<(e?: MouseEvent) => void>, required: true },
	},
	components: { LayoutRow },
});
</script>
