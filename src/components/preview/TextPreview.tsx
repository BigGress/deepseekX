import type { PreviewContent } from "../../types";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { oneDark } from "react-syntax-highlighter/dist/esm/styles/prism";

interface TextPreviewProps {
  content: Extract<PreviewContent, { kind: "text" }>;
}

const SUPPORTED_LANGUAGES: Record<string, string> = {
  typescript: "typescript",
  javascript: "javascript",
  rust: "rust",
  python: "python",
  go: "go",
  bash: "bash",
  shell: "bash",
  json: "json",
  yaml: "yaml",
  yml: "yaml",
  toml: "toml",
  sql: "sql",
  css: "css",
  html: "html",
  markdown: "markdown",
};

export default function TextPreview({ content }: TextPreviewProps) {
  const normalizedLanguage = content.language?.toLowerCase();
  const syntaxLanguage = normalizedLanguage
    ? SUPPORTED_LANGUAGES[normalizedLanguage]
    : undefined;

  return (
    <div className="h-full overflow-auto overflow-x-auto bg-neutral-950">
      {content.language && (
        <div className="px-4 py-1 text-xs text-neutral-600 border-b border-neutral-800 font-mono">
          {content.language}
        </div>
      )}
      {syntaxLanguage ? (
        <SyntaxHighlighter
          language={syntaxLanguage}
          style={oneDark}
          wrapLongLines={false}
          customStyle={{
            margin: 0,
            minHeight: "100%",
            background: "transparent",
            padding: "1rem",
            fontSize: "0.75rem",
            lineHeight: 1.625,
          }}
          codeTagProps={{
            style: {
              fontFamily:
                'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace',
            },
          }}
        >
          {content.text}
        </SyntaxHighlighter>
      ) : (
        <pre className="p-4 text-xs font-mono text-neutral-300 leading-relaxed whitespace-pre min-w-max">
          {content.text}
        </pre>
      )}
    </div>
  );
}
