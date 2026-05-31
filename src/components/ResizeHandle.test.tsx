import { describe, it, expect, afterEach } from "vitest";
import { render, cleanup } from "@testing-library/react";
import ResizeHandle from "./ResizeHandle";

afterEach(cleanup);

describe("ResizeHandle", () => {
  it("renders a div without crashing", () => {
    const { container } = render(<ResizeHandle />);
    expect(container.firstChild).not.toBeNull();
  });

  it("has cursor-col-resize class", () => {
    const { container } = render(<ResizeHandle />);
    expect(container.firstChild).toHaveClass("cursor-col-resize");
  });

  it("renders the inner visual bar", () => {
    const { container } = render(<ResizeHandle />);
    const inner = container.querySelector(".bg-neutral-700");
    expect(inner).not.toBeNull();
  });
});
