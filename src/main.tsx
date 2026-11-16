import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { dismissBootSplash, prepareInitialTheme } from "./boot";
import "./styles/tokens.css";
import "./styles/app.css";

async function boot() {
  // Window is already visible with the HTML splash; stay there only while
  // real boot work runs (settings → theme vars).
  await prepareInitialTheme();

  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );

  requestAnimationFrame(() => dismissBootSplash());
}

void boot();
