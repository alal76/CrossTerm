import { useMemo } from "react";

// ── Simple Markdown Renderer ──
// Renders a subset of markdown: headings, paragraphs, code blocks,
// lists, bold, italic, inline code, links, horizontal rules, and tables.

interface MarkdownNode {
  type: string;
  content?: string;
  children?: MarkdownNode[];
  level?: number;
  ordered?: boolean;
  rows?: string[][];
  lang?: string;
}

function parseMarkdown(source: string): MarkdownNode[] {
  const lines = source.split("\n");
  const nodes: MarkdownNode[] = [];
  let i = 0;

  while (i < lines.length) {
    const line = lines[i];

    // Blank line
    if (line.trim() === "") {
      i++;
      continue;
    }

    // Code block
    if (line.startsWith("```")) {
      const lang = line.slice(3).trim();
      const codeLines: string[] = [];
      i++;
      while (i < lines.length && !lines[i].startsWith("```")) {
        codeLines.push(lines[i]);
        i++;
      }
      i++; // skip closing ```
      nodes.push({ type: "code", content: codeLines.join("\n"), lang });
      continue;
    }

    // Heading
    const headingMatch = line.match(/^(#{1,6})\s+(.+)$/);
    if (headingMatch) {
      nodes.push({ type: "heading", level: headingMatch[1].length, content: headingMatch[2] });
      i++;
      continue;
    }

    // Horizontal rule
    if (/^(-{3,}|\*{3,}|_{3,})$/.test(line.trim())) {
      nodes.push({ type: "hr" });
      i++;
      continue;
    }

    // Table
    if (line.includes("|") && i + 1 < lines.length && /^\|?\s*[-:]+/.test(lines[i + 1])) {
      const tableRows: string[][] = [];
      // Header
      tableRows.push(
        line.split("|").map((c) => c.trim()).filter(Boolean),
      );
      i += 2; // skip header + separator
      while (i < lines.length && lines[i].includes("|") && lines[i].trim() !== "") {
        tableRows.push(
          lines[i].split("|").map((c) => c.trim()).filter(Boolean),
        );
        i++;
      }
      nodes.push({ type: "table", rows: tableRows });
      continue;
    }

    // Unordered list
    if (/^[-*+]\s+/.test(line)) {
      const items: MarkdownNode[] = [];
      while (i < lines.length && /^[-*+]\s+/.test(lines[i])) {
        items.push({ type: "list-item", content: lines[i].replace(/^[-*+]\s+/, "") });
        i++;
      }
      nodes.push({ type: "list", ordered: false, children: items });
      continue;
    }

    // Ordered list
    if (/^\d+\.\s+/.test(line)) {
      const items: MarkdownNode[] = [];
      while (i < lines.length && /^\d+\.\s+/.test(lines[i])) {
        items.push({ type: "list-item", content: lines[i].replace(/^\d+\.\s+/, "") });
        i++;
      }
      nodes.push({ type: "list", ordered: true, children: items });
      continue;
    }

    // Paragraph (collect consecutive non-empty, non-special lines)
    const paraLines: string[] = [];
    while (
      i < lines.length &&
      lines[i].trim() !== "" &&
      !lines[i].startsWith("#") &&
      !lines[i].startsWith("```") &&
      !/^[-*+]\s+/.test(lines[i]) &&
      !/^\d+\.\s+/.test(lines[i]) &&
      !/^(-{3,}|\*{3,}|_{3,})$/.test(lines[i].trim())
    ) {
      paraLines.push(lines[i]);
      i++;
    }
    if (paraLines.length > 0) {
      nodes.push({ type: "paragraph", content: paraLines.join(" ") });
    }
  }

  return nodes;
}

function renderInline(text: string): React.ReactNode[] {
  const result: React.ReactNode[] = [];
  // Process inline markdown: bold, italic, inline code, links
  const regex = /(\*\*(.+?)\*\*)|(\*(.+?)\*)|(`(.+?)`)|(\[(.+?)\]\((.+?)\))/g;
  let lastIndex = 0;
  let match: RegExpExecArray | null;
  let key = 0;

  while ((match = regex.exec(text)) !== null) {
    if (match.index > lastIndex) {
      result.push(text.slice(lastIndex, match.index));
    }

    if (match[1]) {
      // Bold
      result.push(<strong key={key++} className="font-semibold text-text-primary">{match[2]}</strong>);
    } else if (match[3]) {
      // Italic
      result.push(<em key={key++} className="italic">{match[4]}</em>);
    } else if (match[5]) {
      // Inline code
      result.push(
        <code key={key++} className="px-1 py-0.5 rounded bg-surface-sunken text-accent-primary text-[0.85em] font-mono">
          {match[6]}
        </code>,
      );
    } else if (match[7]) {
      // Link
      result.push(
        <span key={key++} className="text-text-link underline cursor-pointer">
          {match[8]}
        </span>,
      );
    }

    lastIndex = match.index + match[0].length;
  }

  if (lastIndex < text.length) {
    result.push(text.slice(lastIndex));
  }

  return result;
}

function MarkdownBlock({ node }: { readonly node: MarkdownNode }) {
  switch (node.type) {
    case "heading": {
      const Tag = `h${node.level}` as keyof React.JSX.IntrinsicElements;
      const sizes: Record<number, string> = {
        1: "text-xl font-bold mt-6 mb-3 text-text-primary",
        2: "text-lg font-semibold mt-5 mb-2 text-text-primary",
        3: "text-base font-semibold mt-4 mb-2 text-text-primary",
        4: "text-sm font-semibold mt-3 mb-1 text-text-primary",
        5: "text-xs font-semibold mt-2 mb-1 text-text-primary",
        6: "text-xs font-medium mt-2 mb-1 text-text-secondary",
      };
      return <Tag className={sizes[node.level ?? 1]}>{renderInline(node.content ?? "")}</Tag>;
    }

    case "paragraph":
      return <p className="text-sm text-text-secondary leading-relaxed mb-3">{renderInline(node.content ?? "")}</p>;

    case "code":
      return (
        <pre className="bg-surface-sunken border border-border-subtle rounded-lg p-3 mb-3 overflow-x-auto">
          <code className="text-xs font-mono text-text-primary whitespace-pre">{node.content}</code>
        </pre>
      );

    case "list":
      if (node.ordered) {
        return (
          <ol className="list-decimal list-inside mb-3 space-y-1">
            {node.children?.map((child, idx) => (
              <li key={idx} className="text-sm text-text-secondary leading-relaxed">
                {renderInline(child.content ?? "")}
              </li>
            ))}
          </ol>
        );
      }
      return (
        <ul className="list-disc list-inside mb-3 space-y-1">
          {node.children?.map((child, idx) => (
            <li key={idx} className="text-sm text-text-secondary leading-relaxed">
              {renderInline(child.content ?? "")}
            </li>
          ))}
        </ul>
      );

    case "table":
      return (
        <div className="overflow-x-auto mb-3">
          <table className="w-full text-xs border-collapse">
            {node.rows && node.rows.length > 0 && (
              <>
                <thead>
                  <tr className="border-b border-border-default">
                    {node.rows[0].map((cell, ci) => (
                      <th key={ci} className="text-left px-2 py-1.5 font-semibold text-text-primary">
                        {renderInline(cell)}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {node.rows.slice(1).map((row, ri) => (
                    <tr key={ri} className="border-b border-border-subtle">
                      {row.map((cell, ci) => (
                        <td key={ci} className="px-2 py-1.5 text-text-secondary">
                          {renderInline(cell)}
                        </td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </>
            )}
          </table>
        </div>
      );

    case "hr":
      return <hr className="border-border-subtle my-4" />;

    default:
      return null;
  }
}

export default function MarkdownRenderer({ content }: { readonly content: string }) {
  const nodes = useMemo(() => parseMarkdown(content), [content]);

  return (
    <div className="markdown-content">
      {nodes.map((node, idx) => (
        <MarkdownBlock key={idx} node={node} />
      ))}
    </div>
  );
}
