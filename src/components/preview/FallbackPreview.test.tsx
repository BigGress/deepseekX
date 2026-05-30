import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";
import FallbackPreview from "./FallbackPreview";

afterEach(cleanup);

describe("FallbackPreview", () => {
  it("renders the message", () => {
    render(<FallbackPreview message="不支持预览此文件类型" />);
    expect(screen.getByText("不支持预览此文件类型")).toBeInTheDocument();
  });
});
