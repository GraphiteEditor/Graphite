<template>
	<div class="window-buttons-web" @click="handleClick" :title="fullscreen.state.windowFullscreen ? 'Exit Fullscreen (F11)' : 'Enter Fullscreen (F11)'">
		<TextLabel v-if="requestFullscreenHotkeys" :italic="true">Go fullscreen to access all hotkeys</TextLabel>
		<IconLabel :icon="fullscreen.state.windowFullscreen ? 'FullscreenExit' : 'FullscreenEnter'" />
	</div>
</template>

<style lang="scss">
.window-buttons-web {
	display: flex;
	align-items: center;
	padding: 0 8px;

	svg {
		fill: var(--color-e-nearwhite);
	}

	.text-label {
		margin-right: 8px;
	}

	&:hover {
		background: var(--color-6-lowergray);
		color: var(--color-f-white);

		svg {
			fill: var(--color-f-white);
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

import IconLabel from "@/components/widgets/labels/IconLabel.vue";
import TextLabel from "@/components/widgets/labels/TextLabel.vue";

export default defineComponent({
	inject: ["fullscreen"],
	methods: {
		async handleClick() {
			if (this.fullscreen.state.windowFullscreen) this.fullscreen.exitFullscreen();
			else this.fullscreen.enterFullscreen();
		},
	},
	computed: {
		requestFullscreenHotkeys() {
			return this.fullscreen.keyboardLockApiSupported && !this.fullscreen.state.keyboardLocked;
		},
	},
	components: {
		IconLabel,
		TextLabel,
	},
});
</script>
