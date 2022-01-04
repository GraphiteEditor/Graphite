<template>
	<span class="text-label" :class="{ bold, italic }">
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
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

export default defineComponent({
	props: {
		bold: { type: Boolean as PropType<boolean>, default: false },
		italic: { type: Boolean as PropType<boolean>, default: false },
	},
});
</script>
