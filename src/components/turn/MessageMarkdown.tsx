import { Fragment, type ReactNode } from "react";

interface InlinePart {
  kind: "text" | "strong" | "em" | "code" | "link";
  text: string;
  href?: string;
}

export default function MessageMarkdown({ content }: { content: string }) {
  const blocks = splitMarkdownBlocks(content);

  return (
    <>
      {blocks.map((block, index) => {
        if (block.kind === "code") {
          return (
            <pre
              key={`code-${index}`}
              className="my-3 overflow-x-auto rounded-md bg-neutral-950/80 p-3 text-xs text-neutral-200"
            >
              <code>{block.text}</code>
            </pre>
          );
        }

        return (
          <Fragment key={`markdown-${index}`}>
            {renderMarkdownChunk(block.text, index)}
          </Fragment>
        );
      })}
    </>
  );
}

function splitMarkdownBlocks(content: string) {
  const parts = content.split(/(```[\s\S]*?```)/g);

  return parts
    .filter(Boolean)
    .map((part) => {
      if (part.startsWith("```")) {
        return {
          kind: "code" as const,
          text: part
            .replace(/```\w*\s*file:\S+\n?/, "")
            .replace(/```\w*\n?/, "")
            .replace(/```$/, ""),
        };
      }

      return {
        kind: "markdown" as const,
        text: part,
      };
    });
}

function renderMarkdownChunk(text: string, keyBase: number) {
  const lines = text.split("\n");
  const nodes: ReactNode[] = [];
  let index = 0;

  while (index < lines.length) {
    const line = lines[index];
    const trimmed = line.trim();

    if (!trimmed) {
      index += 1;
      continue;
    }

    const headingMatch = trimmed.match(/^(#{1,6})\s+(.*)$/);
    if (headingMatch) {
      const level = headingMatch[1].length as 1 | 2 | 3 | 4 | 5 | 6;
      const Tag = `h${level}` as keyof JSX.IntrinsicElements;
      nodes.push(
        <Tag key={`heading-${keyBase}-${index}`}>
          {renderInline(headingMatch[2], `heading-${keyBase}-${index}`)}
        </Tag>,
      );
      index += 1;
      continue;
    }

    if (trimmed.startsWith("> ")) {
      const quoteLines: string[] = [];
      while (index < lines.length && lines[index].trim().startsWith("> ")) {
        quoteLines.push(lines[index].trim().replace(/^>\s?/, ""));
        index += 1;
      }
      nodes.push(
        <blockquote key={`quote-${keyBase}-${index}`}>
          {quoteLines.map((quoteLine, quoteIndex) => (
            <p key={`quote-line-${keyBase}-${index}-${quoteIndex}`}>
              {renderInline(
                quoteLine,
                `quote-inline-${keyBase}-${index}-${quoteIndex}`,
              )}
            </p>
          ))}
        </blockquote>,
      );
      continue;
    }

    if (isUnorderedListItem(trimmed)) {
      const items: string[] = [];
      while (index < lines.length && isUnorderedListItem(lines[index].trim())) {
        items.push(lines[index].trim().replace(/^[-*+]\s+/, ""));
        index += 1;
      }
      nodes.push(
        <ul key={`ul-${keyBase}-${index}`}>
          {items.map((item, itemIndex) => (
            <li key={`ul-item-${keyBase}-${index}-${itemIndex}`}>
              {renderInline(item, `ul-inline-${keyBase}-${index}-${itemIndex}`)}
            </li>
          ))}
        </ul>,
      );
      continue;
    }

    if (isOrderedListItem(trimmed)) {
      const items: string[] = [];
      while (index < lines.length && isOrderedListItem(lines[index].trim())) {
        items.push(lines[index].trim().replace(/^\d+\.\s+/, ""));
        index += 1;
      }
      nodes.push(
        <ol key={`ol-${keyBase}-${index}`}>
          {items.map((item, itemIndex) => (
            <li key={`ol-item-${keyBase}-${index}-${itemIndex}`}>
              {renderInline(item, `ol-inline-${keyBase}-${index}-${itemIndex}`)}
            </li>
          ))}
        </ol>,
      );
      continue;
    }

    const paragraphLines: string[] = [];
    while (index < lines.length) {
      const candidate = lines[index].trim();
      if (
        !candidate ||
        candidate.match(/^(#{1,6})\s+/) ||
        candidate.startsWith("> ") ||
        isUnorderedListItem(candidate) ||
        isOrderedListItem(candidate)
      ) {
        break;
      }
      paragraphLines.push(lines[index]);
      index += 1;
    }

    nodes.push(
      <p key={`p-${keyBase}-${index}`}>
        {paragraphLines.map((paragraphLine, paragraphIndex) => (
          <Fragment key={`p-line-${keyBase}-${index}-${paragraphIndex}`}>
            {paragraphIndex > 0 ? <br /> : null}
            {renderInline(
              paragraphLine,
              `p-inline-${keyBase}-${index}-${paragraphIndex}`,
            )}
          </Fragment>
        ))}
      </p>,
    );
  }

  return nodes;
}

function renderInline(text: string, keyBase: string) {
  return parseInline(text).map((part, index) => {
    const key = `${keyBase}-${index}`;

    switch (part.kind) {
      case "strong":
        return <strong key={key}>{part.text}</strong>;
      case "em":
        return <em key={key}>{part.text}</em>;
      case "code":
        return (
          <code
            key={key}
            className="rounded bg-neutral-950/80 px-1 py-0.5 text-[0.9em] text-neutral-100"
          >
            {part.text}
          </code>
        );
      case "link":
        return (
          <a
            key={key}
            href={part.href}
            target="_blank"
            rel="noreferrer"
            className="text-sky-300 underline decoration-sky-500/60 underline-offset-2 hover:text-sky-200"
          >
            {part.text}
          </a>
        );
      default:
        return <Fragment key={key}>{part.text}</Fragment>;
    }
  });
}

function parseInline(text: string): InlinePart[] {
  const parts: InlinePart[] = [];
  let buffer = "";
  let index = 0;

  const flushBuffer = () => {
    if (buffer) {
      parts.push({ kind: "text", text: buffer });
      buffer = "";
    }
  };

  while (index < text.length) {
    if (text.startsWith("**", index)) {
      const end = text.indexOf("**", index + 2);
      if (end !== -1) {
        flushBuffer();
        parts.push({ kind: "strong", text: text.slice(index + 2, end) });
        index = end + 2;
        continue;
      }
    }

    if (text[index] === "*" && !text.startsWith("**", index)) {
      const end = text.indexOf("*", index + 1);
      if (end !== -1) {
        flushBuffer();
        parts.push({ kind: "em", text: text.slice(index + 1, end) });
        index = end + 1;
        continue;
      }
    }

    if (text[index] === "`") {
      const end = text.indexOf("`", index + 1);
      if (end !== -1) {
        flushBuffer();
        parts.push({ kind: "code", text: text.slice(index + 1, end) });
        index = end + 1;
        continue;
      }
    }

    if (text[index] === "[") {
      const labelEnd = text.indexOf("]", index + 1);
      const hrefStart = labelEnd !== -1 ? text.indexOf("(", labelEnd) : -1;
      const hrefEnd = hrefStart !== -1 ? text.indexOf(")", hrefStart) : -1;
      if (labelEnd !== -1 && hrefStart === labelEnd + 1 && hrefEnd !== -1) {
        flushBuffer();
        parts.push({
          kind: "link",
          text: text.slice(index + 1, labelEnd),
          href: text.slice(hrefStart + 1, hrefEnd),
        });
        index = hrefEnd + 1;
        continue;
      }
    }

    buffer += text[index];
    index += 1;
  }

  flushBuffer();
  return parts;
}

function isUnorderedListItem(line: string) {
  return /^[-*+]\s+/.test(line);
}

function isOrderedListItem(line: string) {
  return /^\d+\.\s+/.test(line);
}
