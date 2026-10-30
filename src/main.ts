import { mount } from "svelte";
import "./main.css";
import App from "./App.svelte";

const target = document.getElementById("app");
if (!target) {
  throw new Error("Failed to find #app mount target");
}

const app = mount(App, { target });

export default app;
