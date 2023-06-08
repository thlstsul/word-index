import { createApp } from "vue";
import App from "./App.vue";
import {
  Drawer,
  Button,
  Input,
  Pagination,
  Collapse,
  Empty,
  BackTop,
  message,
} from "ant-design-vue";

message.config({
  top: "0px",
  duration: 2,
  maxCount: 3,
});

const app = createApp(App);
app.use(Drawer);
app.use(Button);
app.use(Input);
app.use(Pagination);
app.use(Collapse);
app.use(Empty);
app.use(BackTop);
app.mount("#app");
