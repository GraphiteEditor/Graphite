<template>
	<span class="text-label" :class="{ disabled, bold, italic, multiline, 'table-align': tableAlign }" :style="{ 'min-width': minWidth > 0 ? `${minWidth}px` : undefined }" :title="tooltip">
		<slot></slot>
	</span>
</template>

<style lang="scss">
.text-label {
	line-height: 18px;
	white-space: nowrap;
	// Force Safari to not draw a text cursor, even though this element has `user-select: none`
	cursor: default;

	&.disabled {
		color: var(--color-8-uppergray);
	}

	&.bold {
		font-weight: 700;
	}

	&.italic {
		font-style: italic;
	}

	&.multiline {
		white-space: pre-wrap;
		margin: 4px 0;
	}

	&.table-align {
		flex: 0 0 30%;
		text-align: right;
	}
}
</style>

<script lang="ts">
import { defineComponent, type PropType } from "vue";

export default defineComponent({
	props: {
		disabled: { type: Boolean as PropType<boolean>, default: false },
		bold: { type: Boolean as PropType<boolean>, default: false },
		italic: { type: Boolean as PropType<boolean>, default: false },
		tableAlign: { type: Boolean as PropType<boolean>, default: false },
		minWidth: { type: Number as PropType<number>, default: 0 },
		multiline: { type: Boolean as PropType<boolean>, default: false },
		tooltip: { type: String as PropType<string | undefined>, required: false },
	},
});
</script>
