import React from "react";
import ReactDOM from "react-dom/client";
import "./styles.css";
import App from "./App";
import StatusBar from "./StatusBar";

const root = ReactDOM.createRoot(document.getElementById("app")!);

if (window.location.hash === "#statusbar") {
  // Floating status-bar window — minimal pill overlay
  document.body.classList.add("statusbar-mode");
  root.render(
    <React.StrictMode>
      <StatusBar />
    </React.StrictMode>,
  );
} else {
  // Main application window
  root.render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}
