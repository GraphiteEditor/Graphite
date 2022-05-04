<template>
	<span class="text-label" :class="{ bold, italic, preserveWhitespace, tableAlign }">
		<slot></slot>
	</span>
</template>

<style lang="scss">
.text-label {
	white-space: nowrap;
	line-height: 18px;

	&.bold {
		font-weight: 700;
	}

	&.italic {
		font-style: italic;
	}
	&.preserveWhitespace {
		white-space: pre-wrap;
		line-height: normal;
		margin: 0 !important;
	}
	&.tableAlign {
		flex: 0 0 30%;
		text-align: right;
	}
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

export default defineComponent({
	props: {
		bold: { type: Boolean as PropType<boolean>, default: false },
		italic: { type: Boolean as PropType<boolean>, default: false },
		preserveWhitespace: { type: Boolean as PropType<boolean>, default: false },
		tableAlign: { type: Boolean as PropType<boolean>, default: false },
	},
});
</script>
