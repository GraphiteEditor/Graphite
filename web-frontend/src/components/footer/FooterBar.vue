<template>
	<div class="status-area footer-third">
		<div>
			<span>File: 1.8 MB | Memory: 137 MB | Scratch: 0.7/12.3 GB</span>
		</div>
	</div>
	<div class="hint-area footer-third">
		<InputHint :inputMouse="'LMBDrag'">Box Select</InputHint>
		<InputHint :inputKeys="['G']">Grab Selection</InputHint>
		<InputHint :inputKeys="['R']">Rotate Selection</InputHint>
		<InputHint :inputKeys="['S']">Scale Selection</InputHint>
	</div>
	<div class="version-area footer-third">
		<div>
			<span>Graphite 0.1.0</span>
		</div>
	</div>
</template>

<style lang="scss">
.footer-third {
	display: flex;
	flex: 1 1 100%;

	&:nth-child(1) {
		justify-content: flex-start;
	}

	&:nth-child(2) {
		justify-content: center;
	}

	&:nth-child(3) {
		justify-content: flex-end;
	}

	&.status-area div, &.version-area {
		margin: 0 8px;
		display: flex;
		align-items: center;
		white-space: nowrap;
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import InputHint from "./InputHint.vue";

export default defineComponent({
	name: "FooterBar",
	components: {
		InputHint,
	},
});

</script>
