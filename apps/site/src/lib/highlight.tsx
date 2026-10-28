import type { ReactNode } from "react";

const KEYWORDS = new Set([
  "import", "from", "const", "let", "var", "await", "async", "function", "return", "if", "else",
  "new", "export", "default", "type", "interface", "true", "false", "null", "undefined", "try",
  "catch", "finally", "throw", "for", "of", "in", "as", "use", "fn", "pub", "struct", "impl",
  "match", "mut", "use", "crate", "self", "def", "with", "yield", "raise", "class", "pass",
  "None", "True", "False", "and", "or", "not", "is",
]);

/** Small deterministic highlighter for the short, authored snippets on this page. */
export function highlight(source: string): ReactNode[] {
  const out: ReactNode[] = [];
  let i = 0;
  let key = 0;
  const push = (text: string, cls?: string) => {
    if (!text) return;
    out.push(cls ? <span key={key++} className={cls}>{text}</span> : <span key={key++}>{text}</span>);
  };

  while (i < source.length) {
    const ch = source[i];
    const rest = source.slice(i);

    // Comments: //, #, /* */
    if (rest.startsWith("//") || (ch === "#" && !rest.startsWith("#!"))) {
      const end = source.indexOf("\n", i);
      const stop = end === -1 ? source.length : end;
      push(source.slice(i, stop), "tok-com");
      i = stop;
      continue;
    }
    if (rest.startsWith("/*")) {
      const end = source.indexOf("*/", i + 2);
      const stop = end === -1 ? source.length : end + 2;
      push(source.slice(i, stop), "tok-com");
      i = stop;
      continue;
    }

    // Strings, including template literals.
    if (ch === '"' || ch === "'" || ch === "`") {
      let j = i + 1;
      while (j < source.length) {
        if (source[j] === "\\") { j += 2; continue; }
        if (source[j] === ch) { j++; break; }
        j++;
      }
      push(source.slice(i, j), "tok-str");
      i = j;
      continue;
    }

    // Numbers.
    if (/[0-9]/.test(ch)) {
      let j = i + 1;
      while (j < source.length && /[0-9a-fx._]/i.test(source[j])) j++;
      push(source.slice(i, j), "tok-num");
      i = j;
      continue;
    }

    // Identifiers.
    if (/[A-Za-z_$]/.test(ch)) {
      let j = i + 1;
      while (j < source.length && /[A-Za-z0-9_$]/.test(source[j])) j++;
      const word = source.slice(i, j);
      const cls = KEYWORDS.has(word) ? "tok-key" : source[j] === "(" ? "tok-fn" : undefined;
      push(word, cls);
      i = j;
      continue;
    }

    push(ch);
    i++;
  }

  return out;
}
