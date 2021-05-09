import { createApp } from "vue";
import App from "./App.vue";
import { attachResponseHandlerToPage } from "./response-handler";

document.addEventListener("contextmenu", (e) => e.preventDefault());

attachResponseHandlerToPage();

createApp(App).mount("#app");
