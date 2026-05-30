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

  it("renders gfm tables", () => {
    render(
      <MessageMarkdown
        content={`| 平台 | 优势 |
| --- | --- |
| Snowflake | 易用性 |
| Databricks | ML 生态 |`}
      />,
    );

    expect(screen.getByRole("table")).toBeInTheDocument();
    expect(
      screen.getByRole("columnheader", { name: "平台" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "Snowflake" })).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "ML 生态" })).toBeInTheDocument();
  });

  it("renders gfm task lists as read-only checkboxes", () => {
    render(
      <MessageMarkdown
        content={`- [x] 已完成验证
- [ ] 待补充数据`}
      />,
    );

    const checkboxes = screen.getAllByRole("checkbox");
    expect(checkboxes).toHaveLength(2);
    expect(checkboxes[0]).toBeChecked();
    expect(checkboxes[0]).toBeDisabled();
    expect(checkboxes[1]).not.toBeChecked();
    expect(checkboxes[1]).toBeDisabled();
  });

  it("renders strikethrough text", () => {
    render(<MessageMarkdown content={"~~过时结论~~"} />);

    expect(screen.getByText("过时结论").tagName).toBe("DEL");
  });
});
