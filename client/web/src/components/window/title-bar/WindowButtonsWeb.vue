<template>
	<div class="window-buttons-web" @click="handleClick" :title="fullscreen.windowFullscreen ? 'Exit Fullscreen (F11)' : 'Enter Fullscreen (F11)'">
		<TextLabel v-if="requestFullscreenHotkeys">Click to access all hotkeys</TextLabel>
		<Icon :icon="fullscreen.windowFullscreen ? 'FullscreenExit' : 'FullscreenEnter'" />
	</div>
</template>

<style lang="scss">
.window-buttons-web {
	display: flex;
	align-items: center;
	padding: 0 8px;
	fill: var(--color-e-nearwhite);

	.text-label {
		font-weight: normal;
		font-style: italic;
		margin-right: 8px;
	}

	&:hover {
		background: var(--color-6-lowergray);
		color: var(--color-f-white);
		fill: var(--color-f-white);
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import fullscreen, { keyboardLockApiSupported, enterFullscreen, exitFullscreen } from "@/utilities/fullscreen";
import Icon from "@/components/widgets/labels/Icon.vue";
import TextLabel from "@/components/widgets/labels/TextLabel.vue";

const canUseKeyboardLock = keyboardLockApiSupported();

export default defineComponent({
	inject: ["fullscreen"],
	methods: {
		async handleClick() {
			if (fullscreen.windowFullscreen) exitFullscreen();
			else enterFullscreen();
		},
	},
	computed: {
		requestFullscreenHotkeys() {
			return canUseKeyboardLock && !fullscreen.keyboardLocked;
		},
	},
	components: {
		Icon,
		TextLabel,
	},
});
</script>
