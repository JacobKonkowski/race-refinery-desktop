import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import {
  dismissBootSplash,
  prepareInitialTheme,
  revealMainWindow,
  waitMinSplash,
} from "./boot";
import "./styles/tokens.css";
import "./styles/app.css";

async function boot() {
  // Splash HTML is already painted; show the native window on that frame.
  await revealMainWindow();

  await Promise.all([prepareInitialTheme(), waitMinSplash()]);

  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );

  requestAnimationFrame(() => dismissBootSplash());
}

void boot();
