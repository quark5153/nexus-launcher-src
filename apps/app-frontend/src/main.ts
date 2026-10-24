import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import { spotlight } from "./directives/spotlight";
import "./assets/fonts/fonts.css";
import "./style.css";

createApp(App)
  .use(createPinia())
  .directive("spotlight", spotlight)
  .mount("#app");
