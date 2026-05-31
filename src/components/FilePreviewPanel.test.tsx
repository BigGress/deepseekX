import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({ convertFileSrc: (p: string) => `asset://${p}` }));

import FilePreviewPanel from "./FilePreviewPanel";
import type { PreviewDescriptor, PreviewState } from "../types";

afterEach(cleanup);

function makeState(partial?: Partial<PreviewState>): PreviewState {
  return {
    isOpen: true,
    target: { path: "src/main.rs", source: "file_tree" },
    descriptor: {
      path: "src/main.rs", file_name: "main.rs", extension: "rs",
      mime_type: "text/plain", size_bytes: 100, category: "code",
      default_mode: "raw", available_modes: ["raw", "metadata"],
      capabilities: { can_render_inline: true, can_open_focused: true, can_download: true, can_show_text_extract: false, can_show_original_appearance: false },
      content: { kind: "text", text: "fn main() {}", language: "rust" },
      metadata: null, warnings: [],
    } as PreviewDescriptor,
    selectedMode: "raw",
    isLoading: false,
    error: null,
    focusedView: false,
    ...partial,
  };
}

describe("FilePreviewPanel", () => {
  it("shows file name in header", () => {
    render(<FilePreviewPanel state={makeState()} workspaceRoot="/ws" onClose={() => {}} onSwitchMode={() => Promise.resolve()} onOpenFocused={() => {}} />);
    expect(screen.getByText("main.rs")).toBeInTheDocument();
  });

  it("calls onClose when close button is clicked", () => {
    const onClose = vi.fn();
    render(<FilePreviewPanel state={makeState()} workspaceRoot="/ws" onClose={onClose} onSwitchMode={() => Promise.resolve()} onOpenFocused={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: /关闭/i }));
    expect(onClose).toHaveBeenCalled();
  });

  it("shows loading state", () => {
    render(<FilePreviewPanel state={makeState({ isLoading: true, descriptor: null })} workspaceRoot="/ws" onClose={() => {}} onSwitchMode={() => Promise.resolve()} onOpenFocused={() => {}} />);
    expect(screen.getByText("正在加载预览…")).toBeInTheDocument();
  });

  it("shows error state", () => {
    render(<FilePreviewPanel state={makeState({ error: "文件不存在", descriptor: null })} workspaceRoot="/ws" onClose={() => {}} onSwitchMode={() => Promise.resolve()} onOpenFocused={() => {}} />);
    expect(screen.getByText("文件不存在")).toBeInTheDocument();
  });

  it("renders content when descriptor is loaded", () => {
    const { container } = render(<FilePreviewPanel state={makeState()} workspaceRoot="/ws" onClose={() => {}} onSwitchMode={() => Promise.resolve()} onOpenFocused={() => {}} />);
    const codeEl = container.querySelector("code");
    expect(codeEl).not.toBeNull();
    expect(codeEl!.textContent).toContain("fn main() {}");
  });
});
