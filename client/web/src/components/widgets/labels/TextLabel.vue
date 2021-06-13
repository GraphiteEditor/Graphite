<template>
	<span class="text-label">
		<slot></slot>
	</span>
</template>

<style lang="scss">
.text-label {
	white-space: nowrap;
	font-weight: bold;
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

export default defineComponent({
	components: {},
	props: {},
});
</script>
