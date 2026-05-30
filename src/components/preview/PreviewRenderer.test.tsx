import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { afterEach } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ convertFileSrc: (p: string) => `asset://${p}` }));

import PreviewRenderer from "./PreviewRenderer";
import type { PreviewContent } from "../../types";

afterEach(() => {
  // Cleanup after each test
});

describe("PreviewRenderer", () => {
  it("renders TextPreview for text kind", () => {
    const content: PreviewContent = { kind: "text", text: "hello", language: "rs" };
    render(<PreviewRenderer content={content} />);
    expect(screen.getByText("hello")).toBeInTheDocument();
  });

  it("renders TablePreview for table kind", () => {
    const content: PreviewContent = { kind: "table", columns: ["col"], rows: [["val"]] };
    render(<PreviewRenderer content={content} />);
    expect(screen.getByText("col")).toBeInTheDocument();
    expect(screen.getByText("val")).toBeInTheDocument();
  });

  it("renders FallbackPreview for fallback kind", () => {
    const content: PreviewContent = { kind: "fallback", message: "不支持此类型" };
    render(<PreviewRenderer content={content} />);
    expect(screen.getByText("不支持此类型")).toBeInTheDocument();
  });
});
