import { describe, it } from "vitest";
import { assertType, expectTypeOf } from "vitest";
import type {
  PreviewDescriptor,
  PreviewTarget,
  PreviewState,
  PreviewContent,
  PreviewCategory,
} from "./types";

describe("Preview type definitions", () => {
  it("PreviewDescriptor has required shape", () => {
    const d = {} as PreviewDescriptor;
    assertType<string>(d.path);
    assertType<string>(d.file_name);
    assertType<PreviewCategory>(d.category);
    assertType<string[] | undefined>(d.warnings as string[] | undefined);
    // content is nullable
    assertType<PreviewContent | null>(d.content);
  });

  it("PreviewTarget source is constrained union", () => {
    const t = {} as PreviewTarget;
    expectTypeOf(t.source).toEqualTypeOf<
      "file_tree" | "chat_attachment" | "artifact" | "diff"
    >();
  });

  it("PreviewState initial shape", () => {
    const s = {} as PreviewState;
    assertType<boolean>(s.isOpen);
    assertType<boolean>(s.isLoading);
    assertType<string | null>(s.error);
  });

  it("PreviewContent discriminated union narrows correctly", () => {
    const c = {} as PreviewContent;
    if (c.kind === "table") {
      assertType<string[]>(c.columns);
      assertType<string[][]>(c.rows);
    }
    if (c.kind === "media") {
      assertType<"image" | "audio" | "video" | "pdf">(c.mediaType);
    }
  });
});
