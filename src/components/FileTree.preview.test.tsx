import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import FileTree from "./FileTree";
import type { FileNode } from "../types";

afterEach(cleanup);

const nodes: FileNode[] = [
  { name: "main.rs", path: "src/main.rs", is_directory: false, children: null, size: 100 },
];

describe("FileTree preview integration", () => {
  it("calls onPreviewFile on double-click", () => {
    const onPreviewFile = vi.fn();
    render(<FileTree nodes={nodes} onPreviewFile={onPreviewFile} />);
    const node = screen.getByText("main.rs");
    fireEvent.dblClick(node.closest("[data-path]")!);
    expect(onPreviewFile).toHaveBeenCalledWith("src/main.rs");
  });

  it("highlights the currently previewed file", () => {
    render(<FileTree nodes={nodes} previewPath="src/main.rs" />);
    const node = screen.getByText("main.rs");
    expect(node.closest("[data-path]")).toHaveClass("text-blue-400");
  });

  it("does not highlight when previewPath does not match", () => {
    render(<FileTree nodes={nodes} previewPath="src/other.rs" />);
    const node = screen.getByText("main.rs");
    expect(node.closest("[data-path]")).not.toHaveClass("text-blue-400");
  });
});
