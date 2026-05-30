import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { cleanup, afterEach } from "@testing-library/react";
import MarkdownPreview from "./MarkdownPreview";

afterEach(cleanup);

describe("MarkdownPreview", () => {
  it("renders markdown as HTML", () => {
    render(<MarkdownPreview content={{ kind: "markdown", markdown: "# Hello\n\nParagraph" }} />);
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("Hello");
    expect(screen.getByText("Paragraph")).toBeInTheDocument();
  });
});
