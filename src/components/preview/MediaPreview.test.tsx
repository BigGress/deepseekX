import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({ convertFileSrc: (p: string) => `asset://${p}` }));

import MediaPreview from "./MediaPreview";

afterEach(cleanup);

describe("MediaPreview", () => {
  it("renders an img for image type", () => {
    render(<MediaPreview content={{ kind: "media", url: "/img.png", media_type: "image" }} />);
    expect(screen.getByRole("img")).toHaveAttribute("src", "asset:///img.png");
  });

  it("renders a video element for video type", () => {
    const { container } = render(<MediaPreview content={{ kind: "media", url: "/v.mp4", media_type: "video" }} />);
    expect(container.querySelector("video")).toBeTruthy();
  });

  it("renders an audio element for audio type", () => {
    const { container } = render(<MediaPreview content={{ kind: "media", url: "/s.mp3", media_type: "audio" }} />);
    expect(container.querySelector("audio")).toBeTruthy();
  });
});
