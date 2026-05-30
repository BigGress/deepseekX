import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import MessageMarkdown from "./MessageMarkdown";

describe("MessageMarkdown", () => {
  it("renders common markdown blocks and inline formatting", () => {
    render(
      <MessageMarkdown
        content={`# 调研结果

## 关键结论

- 第一条结论
- 第二条结论

1. 步骤一
2. 步骤二

这是一段包含 **粗体**、*斜体*、\`行内代码\` 和 [链接](https://example.com) 的文字。`}
      />,
    );

    expect(
      screen.getByRole("heading", { level: 1, name: "调研结果" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("heading", { level: 2, name: "关键结论" }),
    ).toBeInTheDocument();
    expect(screen.getByText("第一条结论")).toBeInTheDocument();
    expect(screen.getByText("第二条结论")).toBeInTheDocument();
    expect(screen.getByText("步骤一")).toBeInTheDocument();
    expect(screen.getByText("步骤二")).toBeInTheDocument();
    expect(screen.getByText("粗体").tagName).toBe("STRONG");
    expect(screen.getByText("斜体").tagName).toBe("EM");
    expect(screen.getByText("行内代码").tagName).toBe("CODE");
    expect(screen.getByRole("link", { name: "链接" })).toHaveAttribute(
      "href",
      "https://example.com",
    );
  });

  it("renders fenced code blocks separately from prose", () => {
    render(
      <MessageMarkdown
        content={`这里是说明文字。

\`\`\`ts
const answer = 42;
\`\`\``}
      />,
    );

    expect(screen.getByText("这里是说明文字。")).toBeInTheDocument();
    expect(screen.getByText("const answer = 42;").tagName).toBe("CODE");
  });
});
