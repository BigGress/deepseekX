import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { describeFilePreview, resolveFilePreview } from "./api";
import type { PreviewDescriptor } from "./types";

function stub(): PreviewDescriptor {
  return {
    path: "src/main.rs", file_name: "main.rs", extension: "rs",
    mime_type: "text/plain", size_bytes: 500, category: "code",
    default_mode: "raw", available_modes: ["raw", "metadata"],
    capabilities: { can_render_inline: true, can_open_focused: true, can_download: true, can_show_text_extract: false, can_show_original_appearance: false },
    content: null, metadata: null, warnings: [],
  };
}

beforeEach(() => vi.resetAllMocks());

describe("describeFilePreview", () => {
  it("invokes describe_file_preview with path and workspaceRoot", async () => {
    vi.mocked(invoke).mockResolvedValue(stub());
    const result = await describeFilePreview("src/main.rs", "/ws");
    expect(invoke).toHaveBeenCalledWith("describe_file_preview", { path: "src/main.rs", workspaceRoot: "/ws" });
    expect(result.category).toBe("code");
  });
});

describe("resolveFilePreview", () => {
  it("invokes resolve_file_preview with path, workspaceRoot, mode", async () => {
    const d = { ...stub(), content: { kind: "text" as const, text: "fn main(){}" } };
    vi.mocked(invoke).mockResolvedValue(d);
    const result = await resolveFilePreview("src/main.rs", "/ws", "raw");
    expect(invoke).toHaveBeenCalledWith("resolve_file_preview", { path: "src/main.rs", workspaceRoot: "/ws", mode: "raw" });
    expect(result.content).toEqual({ kind: "text", text: "fn main(){}" });
  });
});
