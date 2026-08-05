import React, { type ReactNode } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { cn } from "@/lib/utils";

export interface MarkdownProps {
  children: string;
  className?: string;
  headingIdPrefix?: string;
}

function textFromNode(node: ReactNode): string {
  if (typeof node === "string" || typeof node === "number") return String(node);
  if (Array.isArray(node)) return node.map(textFromNode).join("");
  if (React.isValidElement<{ children?: ReactNode }>(node))
    return textFromNode(node.props.children);
  return "";
}

export function markdownHeadingSlug(value: string): string {
  return (
    value
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "") || "section"
  );
}

export function markdownHeadingId(prefix: string, text: string, occurrence: number): string {
  const suffix = occurrence > 1 ? `-${occurrence}` : "";
  return `${prefix}-${markdownHeadingSlug(text)}${suffix}`;
}

export function Markdown({ children, className, headingIdPrefix }: MarkdownProps) {
  const seenHeadings = new Map<string, number>();
  const heading = (level: 1 | 2 | 3 | 4 | 5 | 6) => {
    const tag = `h${level}`;
    return ({ node: _node, children, ...props }: { node?: unknown; children?: ReactNode }) => {
      const text = textFromNode(children);
      const slug = markdownHeadingSlug(text);
      const occurrence = (seenHeadings.get(slug) ?? 0) + 1;
      seenHeadings.set(slug, occurrence);
      const id = headingIdPrefix ? markdownHeadingId(headingIdPrefix, text, occurrence) : undefined;

      return React.createElement(tag, { ...props, id }, children);
    };
  };

  return (
    <div className={cn("norn-md font-sans", className)}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          a: ({ node: _node, ...props }) => (
            <a {...props} target="_blank" rel="noreferrer noopener" />
          ),
          h1: heading(1),
          h2: heading(2),
          h3: heading(3),
          h4: heading(4),
          h5: heading(5),
          h6: heading(6),
        }}
      >
        {children}
      </ReactMarkdown>
    </div>
  );
}
