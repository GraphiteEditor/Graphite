<template>
	<div class="working-colors">
		<SwatchPairInput />
		<div class="swap-and-reset">
			<IconButton :size="16" :iconSize="12">
				<SwapButton />
			</IconButton>
			<IconButton :size="16" :iconSize="12">
				<ResetColorsButton />
			</IconButton>
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
import SwapButton from "../../../assets/12px-solid/swap.svg";
import ResetColorsButton from "../../../assets/12px-solid/reset-colors.svg";

export default defineComponent({
	components: {
		SwatchPairInput,
		IconButton,
		SwapButton,
		ResetColorsButton,
	},
});
</script>
