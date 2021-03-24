<template>
	<div class="windows window-button minimize" title="Minimize">
		<WindowButtonWinMinimize width="10" height="10" />
	</div>
	<div class="windows window-button maximize" title="Maximize" v-if="!maximized">
		<WindowButtonWinMaximize width="10" height="10" />
	</div>
	<div class="windows window-button restore-down" title="Restore Down" v-if="maximized">
		<WindowButtonWinRestoreDown width="10" height="10" />
	</div>
	<div class="windows window-button close" title="Close">
		<WindowButtonWinClose width="10" height="10" />
	</div>
</template>

<style lang="scss">
.windows.window-button {
	display: flex;
	align-items: center;
	padding: 0 20px;

	svg {
		fill: #ddd;
	}

	&:hover {
		background: #555;

		svg {
			fill: #fff;
		}
	}

	&.close:hover {
		background: #e81123;
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import WindowButtonWinMinimize from "../../../../assets/svg/10x10-bounds-10x10-icon/window-button-win-minimize.svg";
import WindowButtonWinMaximize from "../../../../assets/svg/10x10-bounds-10x10-icon/window-button-win-maximize.svg";
import WindowButtonWinRestoreDown from "../../../../assets/svg/10x10-bounds-10x10-icon/window-button-win-restore-down.svg";
import WindowButtonWinClose from "../../../../assets/svg/10x10-bounds-10x10-icon/window-button-win-close.svg";

export default defineComponent({
	components: {
		WindowButtonWinMinimize,
		WindowButtonWinMaximize,
		WindowButtonWinRestoreDown,
		WindowButtonWinClose,
	},
	props: {
		maximized: { type: Boolean, default: false },
	},
});
</script>
