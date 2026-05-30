import type { PreviewContent } from "../../types";

interface TablePreviewProps {
  content: Extract<PreviewContent, { kind: "table" }>;
}

export default function TablePreview({ content }: TablePreviewProps) {
  const { columns, rows } = content;

  if (columns.length === 0) {
    return <div className="p-4 text-neutral-500 text-sm">表格为空</div>;
  }

  return (
    <div className="h-full overflow-auto">
      <table className="w-full text-xs border-collapse">
        <thead className="sticky top-0 bg-neutral-900">
          <tr>
            {columns.map((col, i) => (
              <th
                key={i}
                className="text-left px-3 py-2 text-neutral-400 font-medium border-b border-neutral-800 whitespace-nowrap"
              >
                {col}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row, ri) => (
            <tr key={ri} className="hover:bg-neutral-850 border-b border-neutral-800/40">
              {row.map((cell, ci) => (
                <td key={ci} className="px-3 py-1.5 text-neutral-300 max-w-xs truncate">
                  {cell}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
