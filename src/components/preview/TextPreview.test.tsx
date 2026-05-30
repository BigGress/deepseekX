import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { cleanup, afterEach } from "@testing-library/react";
import TextPreview from "./TextPreview";

afterEach(cleanup);

describe("TextPreview", () => {
  it("renders the text content", () => {
    render(<TextPreview content={{ kind: "text", text: "fn main() {}", language: "rust" }} />);
    expect(screen.getByText("fn main() {}")).toBeInTheDocument();
  });

  it("renders text without language", () => {
    render(<TextPreview content={{ kind: "text", text: "hello world" }} />);
    expect(screen.getByText("hello world")).toBeInTheDocument();
  });
});
