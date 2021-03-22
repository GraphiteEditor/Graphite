<template>
	<div class="window-title">
		<span>{{title}}</span>
	</div>
</template>

<style lang="scss">
.window-title {
	display: flex;
	align-items: center;
	white-space: nowrap;
	padding: 0 8px;
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

export default defineComponent({
	props: {
		title: { type: String, required: true },
	},
});
</script>
