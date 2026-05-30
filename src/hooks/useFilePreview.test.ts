import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";

vi.mock("../api", () => ({
  describeFilePreview: vi.fn(),
  resolveFilePreview: vi.fn(),
}));

import { describeFilePreview, resolveFilePreview } from "../api";
import { useFilePreview } from "./useFilePreview";
import type { PreviewDescriptor, PreviewTarget } from "../types";

function stub(partial?: Partial<PreviewDescriptor>): PreviewDescriptor {
  return {
    path: "src/main.rs", file_name: "main.rs", extension: "rs",
    mime_type: "text/plain", size_bytes: 500, category: "code",
    default_mode: "raw", available_modes: ["raw", "metadata"],
    capabilities: { can_render_inline: true, can_open_focused: true, can_download: true, can_show_text_extract: false, can_show_original_appearance: false },
    content: null, metadata: null, warnings: [], ...partial,
  };
}

const target: PreviewTarget = { path: "src/main.rs", source: "file_tree" };

beforeEach(() => vi.resetAllMocks());

describe("useFilePreview", () => {
  it("starts closed", () => {
    const { result } = renderHook(() => useFilePreview());
    expect(result.current.state.isOpen).toBe(false);
    expect(result.current.state.target).toBeNull();
  });

  it("openPreview calls describe then resolve and updates state", async () => {
    const skeleton = stub();
    const full = stub({ content: { kind: "text", text: "fn main(){}" } });
    vi.mocked(describeFilePreview).mockResolvedValue(skeleton);
    vi.mocked(resolveFilePreview).mockResolvedValue(full);

    const { result } = renderHook(() => useFilePreview());
    await act(async () => { await result.current.openPreview(target, "/ws"); });

    expect(result.current.state.isOpen).toBe(true);
    expect(result.current.state.descriptor?.content).toEqual({ kind: "text", text: "fn main(){}" });
    expect(result.current.state.isLoading).toBe(false);
  });

  it("closePreview resets to initial state", async () => {
    vi.mocked(describeFilePreview).mockResolvedValue(stub());
    vi.mocked(resolveFilePreview).mockResolvedValue(stub());

    const { result } = renderHook(() => useFilePreview());
    await act(async () => { await result.current.openPreview(target, "/ws"); });
    act(() => result.current.closePreview());

    expect(result.current.state.isOpen).toBe(false);
    expect(result.current.state.target).toBeNull();
  });

  it("switchMode calls resolveFilePreview with the new mode", async () => {
    const base = stub();
    const structured = stub({ content: { kind: "table", columns: ["a"], rows: [["1"]] } });
    vi.mocked(describeFilePreview).mockResolvedValue(base);
    vi.mocked(resolveFilePreview)
      .mockResolvedValueOnce(base)
      .mockResolvedValueOnce(structured);

    const { result } = renderHook(() => useFilePreview());
    await act(async () => { await result.current.openPreview(target, "/ws"); });
    await act(async () => { await result.current.switchMode("structured", "/ws"); });

    expect(resolveFilePreview).toHaveBeenCalledWith("src/main.rs", "/ws", "structured");
    expect(result.current.state.descriptor?.content).toEqual({ kind: "table", columns: ["a"], rows: [["1"]] });
  });

  it("sets error when describeFilePreview rejects", async () => {
    vi.mocked(describeFilePreview).mockRejectedValue(new Error("file not found"));

    const { result } = renderHook(() => useFilePreview());
    await act(async () => { await result.current.openPreview(target, "/ws"); });

    expect(result.current.state.error).toBe("file not found");
    expect(result.current.state.isLoading).toBe(false);
  });

  it("openFocused / closeFocused toggle focusedView", () => {
    const { result } = renderHook(() => useFilePreview());
    act(() => result.current.openFocused());
    expect(result.current.state.focusedView).toBe(true);
    act(() => result.current.closeFocused());
    expect(result.current.state.focusedView).toBe(false);
  });
});
