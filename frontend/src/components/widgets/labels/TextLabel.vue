<template>
	<span class="text-label" :class="{ bold, italic, preserveWhitespace }">
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
	},
});
</script>
