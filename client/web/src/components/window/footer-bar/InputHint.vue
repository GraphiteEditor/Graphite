<template>
	<div class="input-hint">
		<span class="input-key" v-for="inputKey in inputKeys" :key="inputKey" :class="keyCapWidth(inputKey)">
			{{ inputKey }}
		</span>
		<span class="input-mouse" v-if="inputMouse">
			<MouseHintNone v-if="inputMouse === MouseInputInteraction.None" width="16" height="16" />
			<MouseHintLMB v-if="inputMouse === MouseInputInteraction.LMB" width="16" height="16" />
			<MouseHintRMB v-if="inputMouse === MouseInputInteraction.RMB" width="16" height="16" />
			<MouseHintMMB v-if="inputMouse === MouseInputInteraction.MMB" width="16" height="16" />
			<MouseHintScrollUp v-if="inputMouse === MouseInputInteraction.ScrollUp" width="16" height="16" />
			<MouseHintScrollDown v-if="inputMouse === MouseInputInteraction.ScrollDown" width="16" height="16" />
			<MouseHintDrag v-if="inputMouse === MouseInputInteraction.Drag" width="16" height="16" />
			<MouseHintLMBDrag v-if="inputMouse === MouseInputInteraction.LMBDrag" width="16" height="16" />
			<MouseHintRMBDrag v-if="inputMouse === MouseInputInteraction.RMBDrag" width="16" height="16" />
			<MouseHintMMBDrag v-if="inputMouse === MouseInputInteraction.MMBDrag" width="16" height="16" />
		</span>
		<span class="hint-text">
			<slot></slot>
		</span>
	</div>
</template>

<style lang="scss">
.input-hint {
	height: 100%;
	margin: 0 8px;
	display: flex;
	align-items: center;
	white-space: nowrap;

	.input-key,
	.input-mouse {
		margin-right: 4px;
	}

	.input-key {
		font-family: "Inconsolata", monospace;
		font-weight: bold;
		text-align: center;
		color: #111;
		background: #ddd;
		border: 1px;
		box-sizing: border-box;
		border-style: solid;
		border-color: #888;
		border-radius: 4px;
		height: 16px;
		line-height: 16px;
	}

	.input-key.width-16 {
		width: 16px;
	}

	.input-key.width-24 {
		width: 24px;
	}

	.input-key.width-32 {
		width: 32px;
	}

	.input-key.width-40 {
		width: 40px;
	}

	.input-key.width-48 {
		width: 48px;
	}

	.input-mouse {
		font-size: 0;

		.bright {
			fill: #ddd;
		}

		.dim {
			fill: #888;
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import MouseHintNone from "../../../../assets/svg/16x16-bounds-16x16-icon/mouse-hint-none.svg";
import MouseHintLMB from "../../../../assets/svg/16x16-bounds-16x16-icon/mouse-hint-lmb.svg";
import MouseHintRMB from "../../../../assets/svg/16x16-bounds-16x16-icon/mouse-hint-rmb.svg";
import MouseHintMMB from "../../../../assets/svg/16x16-bounds-16x16-icon/mouse-hint-mmb.svg";
import MouseHintScrollUp from "../../../../assets/svg/16x16-bounds-16x16-icon/mouse-hint-scroll-up.svg";
import MouseHintScrollDown from "../../../../assets/svg/16x16-bounds-16x16-icon/mouse-hint-scroll-down.svg";
import MouseHintDrag from "../../../../assets/svg/16x16-bounds-16x16-icon/mouse-hint-drag.svg";
import MouseHintLMBDrag from "../../../../assets/svg/16x16-bounds-16x16-icon/mouse-hint-lmb-drag.svg";
import MouseHintRMBDrag from "../../../../assets/svg/16x16-bounds-16x16-icon/mouse-hint-rmb-drag.svg";
import MouseHintMMBDrag from "../../../../assets/svg/16x16-bounds-16x16-icon/mouse-hint-mmb-drag.svg";

export enum MouseInputInteraction {
	"None" = "None",
	"LMB" = "LMB",
	"RMB" = "RMB",
	"MMB" = "MMB",
	"ScrollUp" = "ScrollUp",
	"ScrollDown" = "ScrollDown",
	"Drag" = "Drag",
	"LMBDrag" = "LMBDrag",
	"RMBDrag" = "RMBDrag",
	"MMBDrag" = "MMBDrag",
}

export default defineComponent({
	components: {
		MouseHintNone,
		MouseHintLMB,
		MouseHintRMB,
		MouseHintMMB,
		MouseHintScrollUp,
		MouseHintScrollDown,
		MouseHintDrag,
		MouseHintLMBDrag,
		MouseHintRMBDrag,
		MouseHintMMBDrag,
	},
	props: {
		inputKeys: { type: Array, default: () => [] },
		inputMouse: { type: String },
	},
	methods: {
		keyCapWidth(keyText: string) {
			return `width-${keyText.length * 8 + 8}`;
		},
	},
	data() {
		return {
			MouseInputInteraction,
		};
	},
});
</script>
