import { describe, it, expect, vi, afterEach } from "vitest";
import { render, cleanup } from "@testing-library/react";

// Mock Tauri APIs
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn().mockResolvedValue([]) }));
vi.mock("@tauri-apps/plugin-shell", () => ({ Command: { create: vi.fn() } }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

// Mock the api module
vi.mock("./api", () => ({
  listProjects: vi.fn().mockResolvedValue([]),
  createProject: vi.fn(),
  updateProject: vi.fn(),
  deleteProject: vi.fn(),
  listFiles: vi.fn().mockResolvedValue([]),
  getSetting: vi.fn().mockResolvedValue(""),
  setSetting: vi.fn(),
  listTuiSessions: vi.fn().mockResolvedValue([]),
  readTuiSession: vi.fn(),
  deleteTuiSession: vi.fn(),
  readTuiSessionTurns: vi.fn(),
  readLlmLogs: vi.fn().mockResolvedValue([]),
  runAgentTask: vi.fn(),
  sendMessageViaApi: vi.fn(),
}));

import App from "./App";

afterEach(cleanup);

describe("App", () => {
  it("renders without crashing", () => {
    const { container } = render(<App />);
    expect(container.firstChild).not.toBeNull();
  });
});
