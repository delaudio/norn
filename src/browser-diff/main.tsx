import React from "react";
import ReactDOM from "react-dom/client";
import "@/index.css";
import { BrowserDiffApp } from "./BrowserDiffApp";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <BrowserDiffApp />
  </React.StrictMode>,
);
