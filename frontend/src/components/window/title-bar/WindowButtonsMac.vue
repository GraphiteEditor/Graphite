<template>
	<div class="mac window-buttons">
		<div class="close" title="Close"></div>
		<div class="minimize" title="Minimize"></div>
		<div class="zoom" title="Zoom"></div>
	</div>
</template>

<style lang="scss">
.mac.window-buttons {
	display: flex;
	align-items: center;

	div {
		display: flex;
		align-items: center;
		margin-left: 8px;
		width: 11px;
		height: 11px;
		border-radius: 50%;

		&.close {
			background: #ff5a52;
		}

		&.minimize {
			background: #e6c029;
		}

		&.zoom {
			background: #54c22b;
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

export default defineComponent({
	props: {
		maximized: { type: Boolean as PropType<boolean>, default: false },
	},
});
</script>
