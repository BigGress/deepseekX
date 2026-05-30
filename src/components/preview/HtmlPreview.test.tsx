import { describe, it, expect, afterEach } from "vitest";
import { render, cleanup } from "@testing-library/react";
import HtmlPreview from "./HtmlPreview";

afterEach(cleanup);

describe("HtmlPreview", () => {
  it("renders an iframe for sandboxed HTML", () => {
    const { container } = render(<HtmlPreview content={{ kind: "html", html: "<h1>Hi</h1>", sandboxed: true }} />);
    expect(container.querySelector("iframe")).toBeTruthy();
  });
});
