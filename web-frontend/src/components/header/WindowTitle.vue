<template>
	<div class="window-title">
		<span>X-35B.gdd* - Graphite</span>
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
	name: "WindowTitle",
});
</script>
