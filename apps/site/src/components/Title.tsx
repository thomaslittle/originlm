import { Fragment, useEffect, useRef, useState, type CSSProperties, type ElementType } from "react";

/* Display type rises out of a mask, line by line. Words are split on the server so the
   markup is the same before and after hydration; line indices are measured on the client
   and written as --l, which React does not own. Body copy never animates. */

export function Words({ text }: { text: string }) {
  const words = text.split(" ");
  return (
    <>
      {words.map((word, index) => (
        <Fragment key={index}>
          <span className="w" style={{ "--i": index } as CSSProperties}><span>{word}</span></span>
          {index < words.length - 1 ? " " : null}
        </Fragment>
      ))}
    </>
  );
}

export function markLines(root: HTMLElement) {
  let line = -1, top = -Infinity;
  root.querySelectorAll<HTMLElement>(".w").forEach((word) => {
    if (word.offsetTop > top + 2) { line++; top = word.offsetTop; }
    word.style.setProperty("--l", String(line));
  });
}

type TitleProps = { as?: ElementType; id?: string; text: string; className?: string };

export function Title({ as: Tag = "h2", id, text, className }: TitleProps) {
  const ref = useRef<HTMLElement>(null);
  const [shown, setShown] = useState(false);
  useEffect(() => {
    const node = ref.current;
    if (!node) return;
    markLines(node);
    if (!document.documentElement.classList.contains("motion") || typeof IntersectionObserver === "undefined") { setShown(true); return; }
    const observer = new IntersectionObserver(([entry]) => {
      if (entry.isIntersecting) { setShown(true); observer.disconnect(); }
    }, { rootMargin: "0px 0px -14% 0px" });
    observer.observe(node);
    const relayout = () => markLines(node);
    window.addEventListener("resize", relayout);
    return () => { observer.disconnect(); window.removeEventListener("resize", relayout); };
  }, []);
  return (
    <Tag ref={ref} id={id} className={`title ${className ?? ""}`} data-in={shown || undefined}>
      <Words text={text} />
    </Tag>
  );
}
