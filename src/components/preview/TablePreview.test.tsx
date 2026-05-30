import { describe, it, expect, afterEach } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";
import TablePreview from "./TablePreview";

afterEach(cleanup);

describe("TablePreview", () => {
  it("renders column headers", () => {
    render(<TablePreview content={{ kind: "table", columns: ["name", "age"], rows: [["Alice", "30"]] }} />);
    expect(screen.getByText("name")).toBeInTheDocument();
    expect(screen.getByText("age")).toBeInTheDocument();
  });

  it("renders data rows", () => {
    render(<TablePreview content={{ kind: "table", columns: ["x"], rows: [["foo"], ["bar"]] }} />);
    expect(screen.getByText("foo")).toBeInTheDocument();
    expect(screen.getByText("bar")).toBeInTheDocument();
  });
});
