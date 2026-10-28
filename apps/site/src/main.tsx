import { StrictMode } from "react";
import { createRoot, hydrateRoot } from "react-dom/client";
import "./styles/site.css";
import App from "./App";

document.documentElement.classList.add("js");
const root = document.getElementById("root");
if (!root) throw new Error("Root element missing");

const page = <StrictMode><App /></StrictMode>;
if (root.hasChildNodes()) hydrateRoot(root, page);
else createRoot(root).render(page);
