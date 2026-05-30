import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import PreviewModeTabs from "./PreviewModeTabs";
import type { PreviewMode } from "../../types";

afterEach(cleanup);

const modes: PreviewMode[] = ["structured", "raw", "metadata"];

describe("PreviewModeTabs", () => {
  it("renders available modes", () => {
    render(<PreviewModeTabs modes={modes} selected="structured" onSelect={() => {}} />);
    expect(screen.getByText("结构化")).toBeInTheDocument();
    expect(screen.getByText("源码")).toBeInTheDocument();
    expect(screen.getByText("元数据")).toBeInTheDocument();
  });

  it("marks selected mode as active", () => {
    render(<PreviewModeTabs modes={modes} selected="raw" onSelect={() => {}} />);
    const rawBtn = screen.getByText("源码");
    expect(rawBtn.closest("button")).toHaveClass("text-white");
  });

  it("calls onSelect when a tab is clicked", () => {
    const onSelect = vi.fn();
    render(<PreviewModeTabs modes={modes} selected="structured" onSelect={onSelect} />);
    fireEvent.click(screen.getByText("源码"));
    expect(onSelect).toHaveBeenCalledWith("raw");
  });
});
