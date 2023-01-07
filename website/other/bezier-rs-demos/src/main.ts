import { createApp } from "vue";


import BezierExample from "@/components/BezierExample";
import BezierExamplePane from "@/components/BezierExamplePane";
import SubpathExample from "@/components/SubpathExample";

import App from "@/App.vue";

document.title = "Bezier-rs Interactive Documentation";

window.customElements.define("bezier-example", BezierExample);
window.customElements.define("bezier-example-pane", BezierExamplePane);
window.customElements.define("subpath-example", SubpathExample);
createApp(App).mount("#app");

declare global {
	interface HTMLElementTagNameMap {
		"bezier-example": BezierExample;
		"bezier-example-pane": BezierExamplePane;
		"subpath-example": SubpathExample;
	}
}
