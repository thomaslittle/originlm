import type { ReactNode } from "react";
import { highlight } from "../lib/highlight";
import { CopyButton } from "./CopyButton";

type CodeBlockProps = {
  code: string;
  language?: string;
  title?: string;
  copy?: boolean;
  className?: string;
  footer?: ReactNode;
};

export function CodeBlock({ code, language, title, copy = true, className, footer }: CodeBlockProps) {
  const clean = code.replace(/^\n+|\n+$/g, "");
  const caption = title ?? language;
  return (
    <div className={`code-block ${className ?? ""}`}>
      {(caption || copy) && (
        <div className="code-head">
          <span>{caption ?? "code"}</span>
          {copy && <CopyButton value={clean} label="Copy" />}
        </div>
      )}
      <pre tabIndex={0} aria-label={caption ? `${caption} code` : "Code sample"}>
        <code>{highlight(clean)}</code>
      </pre>
      {footer}
    </div>
  );
}
