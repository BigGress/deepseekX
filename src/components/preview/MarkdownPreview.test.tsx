import { describe, it, expect, afterEach } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";
import MarkdownPreview from "./MarkdownPreview";

afterEach(cleanup);

describe("MarkdownPreview", () => {
  it("renders markdown as HTML", () => {
    render(<MarkdownPreview content={{ kind: "markdown", markdown: "# Hello\n\nParagraph" }} />);
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("Hello");
    expect(screen.getByText("Paragraph")).toBeInTheDocument();
  });
});
