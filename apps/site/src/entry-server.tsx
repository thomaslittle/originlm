import { StrictMode } from "react";
import { renderToString } from "react-dom/server";
import App from "./App";
export { VERSION } from "./data/facts";

export function render() {
  return renderToString(<StrictMode><App /></StrictMode>);
}
