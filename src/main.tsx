import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import ChatInputPreview from "./debug/ChatInputPreview";
import ProjectSettingsPreview from "./debug/ProjectSettingsPreview";
import TurnItemPreview from "./debug/TurnItemPreview";
import "./index.css";

const searchParams = new URLSearchParams(window.location.search);
const debugView = searchParams.get("debug");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {debugView === "chat-input" ? (
      <ChatInputPreview />
    ) : debugView === "project-settings" ? (
      <ProjectSettingsPreview />
    ) : debugView === "turn-preview" ? (
      <TurnItemPreview />
    ) : (
      <App />
    )}
  </React.StrictMode>,
);
