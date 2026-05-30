import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({ convertFileSrc: (p: string) => `asset://${p}` }));

import FocusedPreviewOverlay from "./FocusedPreviewOverlay";
import type { PreviewDescriptor } from "../types";

afterEach(cleanup);

const descriptor: PreviewDescriptor = {
  path: "README.md", file_name: "README.md", extension: "md",
  mime_type: "text/markdown", size_bytes: 200, category: "markdown",
  default_mode: "structured", available_modes: ["structured", "raw"],
  capabilities: { can_render_inline: true, can_open_focused: true, can_download: true, can_show_text_extract: false, can_show_original_appearance: false },
  content: { kind: "markdown", markdown: "# Title\n\nBody text" },
  metadata: null, warnings: [],
};

describe("FocusedPreviewOverlay", () => {
  it("renders file name in header", () => {
    render(<FocusedPreviewOverlay descriptor={descriptor} selectedMode="structured" workspaceRoot="/ws" onClose={() => {}} onSwitchMode={async () => {}} />);
    expect(screen.getByText("README.md")).toBeInTheDocument();
  });

  it("calls onClose when close button is clicked", () => {
    const onClose = vi.fn();
    render(<FocusedPreviewOverlay descriptor={descriptor} selectedMode="structured" workspaceRoot="/ws" onClose={onClose} onSwitchMode={async () => {}} />);
    fireEvent.click(screen.getByRole("button", { name: /关闭/i }));
    expect(onClose).toHaveBeenCalled();
  });

  it("renders the content via PreviewRenderer", () => {
    render(<FocusedPreviewOverlay descriptor={descriptor} selectedMode="structured" workspaceRoot="/ws" onClose={() => {}} onSwitchMode={async () => {}} />);
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("Title");
  });
});
