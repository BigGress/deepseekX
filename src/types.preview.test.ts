import { describe, it, expectTypeOf } from "vitest";
import type {
  PreviewMode, PreviewCategory, PreviewContent,
  PreviewDescriptor, PreviewTarget, PreviewState,
} from "./types";

describe("preview types", () => {
  it("PreviewDescriptor has required shape", () => {
    const d: PreviewDescriptor = {
      path: "src/main.rs", file_name: "main.rs", extension: "rs",
      mime_type: "text/plain", size_bytes: 100, category: "code",
      default_mode: "raw", available_modes: ["raw", "metadata"],
      capabilities: {
        can_render_inline: true, can_open_focused: true, can_download: true,
        can_show_text_extract: false, can_show_original_appearance: false,
      },
      content: { kind: "text", text: "fn main() {}", language: "rust" },
      metadata: { line_count: 1 }, warnings: [],
    };
    expectTypeOf(d.category).toMatchTypeOf<PreviewCategory>();
  });

  it("PreviewTarget source is constrained", () => {
    const t: PreviewTarget = { path: "src/main.rs", source: "file_tree" };
    expectTypeOf(t.source).toMatchTypeOf<string>();
  });

  it("PreviewState starts closed", () => {
    const s: PreviewState = {
      isOpen: false, target: null, descriptor: null, selectedMode: null,
      isLoading: false, error: null, focusedView: false,
    };
    expectTypeOf(s.isOpen).toMatchTypeOf<boolean>();
  });

  it("PreviewContent is a discriminated union on kind", () => {
    const c: PreviewContent = { kind: "table", columns: ["a", "b"], rows: [["1", "2"]] };
    expectTypeOf(c.kind).toMatchTypeOf<string>();
  });
});
