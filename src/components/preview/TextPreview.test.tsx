import { describe, it, expect, afterEach } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";
import TextPreview from "./TextPreview";

afterEach(cleanup);

describe("TextPreview", () => {
  it("renders the text content", () => {
    const { container } = render(
      <TextPreview content={{ kind: "text", text: "fn main() {}", language: "rust" }} />
    );

    expect(container.querySelector("code")?.textContent).toContain("fn main() {}");
  });

  it("renders syntax highlighting for supported code languages", () => {
    const { container } = render(
      <TextPreview content={{ kind: "text", text: "const answer = 42;", language: "typescript" }} />
    );

    expect(container.querySelector("code")).toBeInTheDocument();
    expect(container.querySelector("[class*=\"token\"]")).toBeInTheDocument();
  });

  it("keeps long code lines horizontally scrollable", () => {
    const { container } = render(
      <TextPreview content={{ kind: "text", text: "const answer = 42;", language: "typescript" }} />
    );

    const scrollContainer = container.firstElementChild as HTMLElement;
    expect(scrollContainer.className).toContain("overflow-auto");
    expect(scrollContainer.className).toContain("overflow-x-auto");
  });

  it("renders text without language", () => {
    render(<TextPreview content={{ kind: "text", text: "hello world" }} />);
    expect(screen.getByText("hello world")).toBeInTheDocument();
  });
});
