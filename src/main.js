import { createApp } from "vue";
import App from "./App.vue";
import {
  Drawer,
  Button,
  Input,
  Pagination,
  Collapse,
  Divider,
  Empty,
} from "ant-design-vue";

const app = createApp(App);
app.use(Drawer);
app.use(Button);
app.use(Input);
app.use(Pagination);
app.use(Collapse);
app.use(Divider);
app.use(Empty);
app.mount("#app");
