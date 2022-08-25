<template>
	<span class="text-label" :class="{ bold, italic, multiline, 'table-align': tableAlign }">
		<slot></slot>
	</span>
</template>

<style lang="scss">
.text-label {
	line-height: 18px;
	white-space: nowrap;

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
		bold: { type: Boolean as PropType<boolean>, default: false },
		italic: { type: Boolean as PropType<boolean>, default: false },
		tableAlign: { type: Boolean as PropType<boolean>, default: false },
		multiline: { type: Boolean as PropType<boolean>, default: false },
	},
});
</script>
