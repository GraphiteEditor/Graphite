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
import { defineComponent } from "vue";

export default defineComponent({
	components: {},
	props: {
		bold: { type: Boolean, default: false },
		italic: { type: Boolean, default: false },
	},
});
</script>
