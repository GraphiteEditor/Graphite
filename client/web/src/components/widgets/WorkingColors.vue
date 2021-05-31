<template>
	<div class="working-colors">
		<SwatchPairInput />
		<div class="swap-and-reset">
			<IconButton @click="swapColors" :icon="'SwapButton'" :size="16" />
			<IconButton @click="resetColors" :icon="'ResetColorsButton'" :size="16" />
		</div>
	</div>
</template>

<style lang="scss">
.working-colors {
	.swap-and-reset {
		font-size: 0;
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import SwatchPairInput from "./inputs/SwatchPairInput.vue";
import IconButton from "./buttons/IconButton.vue";

const wasm = import("../../../wasm/pkg");

export default defineComponent({
	methods: {
		async swapColors() {
			const { swap_colors } = await wasm;
			swap_colors();
		},
		async resetColors() {
			const { reset_colors } = await wasm;
			reset_colors();
		},
	},
	components: {
		SwatchPairInput,
		IconButton,
	},
});
</script>
