import { useEffect, useRef, useState } from "react";
import { Check, Copy } from "./Icon";

type CopyButtonProps = {
  value: string;
  label?: string;
  className?: string;
  onCopied?: () => void;
};

export function CopyButton({ value, label = "Copy", className, onCopied }: CopyButtonProps) {
  const [copied, setCopied] = useState(false);
  const timer = useRef<number | null>(null);

  useEffect(() => () => {
    if (timer.current) window.clearTimeout(timer.current);
  }, []);

  async function copy() {
    try {
      await navigator.clipboard.writeText(value);
    } catch {
      const area = document.createElement("textarea");
      area.value = value;
      area.setAttribute("readonly", "");
      area.style.position = "fixed";
      area.style.opacity = "0";
      document.body.appendChild(area);
      area.select();
      document.execCommand("copy");
      area.remove();
    }
    setCopied(true);
    onCopied?.();
    if (timer.current) window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setCopied(false), 1600);
  }

  return (
    <button
      type="button"
      className={`copy-btn ${className ?? ""}`}
      data-copied={copied}
      onClick={copy}
      aria-label={copied ? "Copied to clipboard" : `${label}: ${value}`}
    >
      {copied ? <Check size={13} /> : <Copy size={13} />}
      <span aria-live="polite">{copied ? "Copied" : label}</span>
    </button>
  );
}
