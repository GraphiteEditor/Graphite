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
			background: #ff5f57;
		}

		&.minimize {
			background: #ffbd2f;
		}

		&.zoom {
			background: #29c93f;
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

export default defineComponent({
	props: {
		maximized: { type: Boolean, default: false },
	},
});
</script>
