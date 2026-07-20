import { createApp } from "vue";
import App from "./App.vue";
import "./assets/css/main.css";

// @gebinee/components 使用显式导入，需手动导入对应样式
import "element-plus/es/components/dialog/style/css";
import "element-plus/es/components/icon/style/css";
import "element-plus/es/components/button/style/css";
import "element-plus/es/components/progress/style/css";
import "element-plus/es/components/message/style/css";

createApp(App).mount("#app");
