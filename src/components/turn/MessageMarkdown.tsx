import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

interface MessageMarkdownProps {
  content: string;
}

export default function MessageMarkdown({ content }: MessageMarkdownProps) {
  return (
    <div className="space-y-3 text-sm leading-7 text-neutral-200">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        skipHtml
        components={{
          h1: ({ children }) => (
            <h1 className="text-xl font-semibold text-white">{children}</h1>
          ),
          h2: ({ children }) => (
            <h2 className="mt-4 text-lg font-semibold text-white">{children}</h2>
          ),
          h3: ({ children }) => (
            <h3 className="mt-4 text-base font-semibold text-white">
              {children}
            </h3>
          ),
          h4: ({ children }) => (
            <h4 className="mt-3 text-sm font-semibold uppercase tracking-wide text-neutral-100">
              {children}
            </h4>
          ),
          h5: ({ children }) => (
            <h5 className="mt-3 text-sm font-semibold text-neutral-100">
              {children}
            </h5>
          ),
          h6: ({ children }) => (
            <h6 className="mt-3 text-xs font-semibold uppercase tracking-wide text-neutral-300">
              {children}
            </h6>
          ),
          p: ({ children }) => <p className="leading-7 text-neutral-200">{children}</p>,
          ul: ({ children }) => (
            <ul className="list-disc space-y-1 pl-5 text-neutral-200">{children}</ul>
          ),
          ol: ({ children }) => (
            <ol className="list-decimal space-y-1 pl-5 text-neutral-200">{children}</ol>
          ),
          li: ({ children }) => <li className="leading-7 marker:text-neutral-400">{children}</li>,
          blockquote: ({ children }) => (
            <blockquote className="border-l-2 border-emerald-500/40 pl-4 text-neutral-300">
              {children}
            </blockquote>
          ),
          a: ({ href, children }) => (
            <a
              href={href}
              target="_blank"
              rel="noreferrer"
              className="text-sky-300 underline decoration-sky-500/60 underline-offset-2 hover:text-sky-200"
            >
              {children}
            </a>
          ),
          code: ({ children, className, ...props }) => {
            const inline = !className;
            if (inline) {
              return (
                <code
                  className="rounded bg-neutral-950/80 px-1 py-0.5 text-[0.9em] text-neutral-100"
                  {...props}
                >
                  {children}
                </code>
              );
            }
            return (
              <code className={className} {...props}>
                {children}
              </code>
            );
          },
          pre: ({ children }) => (
            <pre className="my-3 overflow-x-auto rounded-md bg-neutral-950/80 p-3 text-xs text-neutral-200">
              {children}
            </pre>
          ),
          table: ({ children }) => (
            <div className="my-4 overflow-x-auto">
              <table className="min-w-full border-collapse text-sm text-neutral-200">
                {children}
              </table>
            </div>
          ),
          thead: ({ children }) => (
            <thead className="bg-neutral-900/70 text-neutral-100">{children}</thead>
          ),
          tbody: ({ children }) => <tbody>{children}</tbody>,
          tr: ({ children }) => (
            <tr className="border-b border-neutral-800 last:border-b-0">{children}</tr>
          ),
          th: ({ children }) => (
            <th className="border-b border-neutral-700 px-3 py-2 text-left font-medium text-white">
              {children}
            </th>
          ),
          td: ({ children }) => (
            <td className="px-3 py-2 align-top text-neutral-200">{children}</td>
          ),
          del: ({ children }) => (
            <del className="text-neutral-400 line-through">{children}</del>
          ),
          input: ({ type, checked, disabled, ...props }) => {
            if (type !== "checkbox") {
              return <input type={type} disabled={disabled} readOnly {...props} />;
            }
            return (
              <input
                type="checkbox"
                checked={checked}
                disabled
                readOnly
                className="mr-2 accent-emerald-400"
                {...props}
              />
            );
          },
        }}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
}
