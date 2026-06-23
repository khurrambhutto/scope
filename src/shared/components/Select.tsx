import { useState, useRef, useEffect, useCallback } from "react";

export interface SelectOption<T extends string> {
  value: T;
  label: string;
}

export function Select<T extends string>({
  options,
  value,
  onChange,
  className,
  ariaLabel,
  iconTrigger,
}: {
  options: SelectOption<T>[];
  value: T;
  onChange: (v: T) => void;
  className?: string;
  ariaLabel?: string;
  iconTrigger?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [focusIdx, setFocusIdx] = useState(-1);
  const containerRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLUListElement>(null);

  const selected = options.find((o) => o.value === value);
  const selectedIdx = options.findIndex((o) => o.value === value);

  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (!open) {
        if (e.key === "Enter" || e.key === "ArrowDown" || e.key === " ") {
          e.preventDefault();
          setOpen(true);
          setFocusIdx(selectedIdx >= 0 ? selectedIdx : 0);
        }
        return;
      }
      switch (e.key) {
        case "ArrowDown":
          e.preventDefault();
          setFocusIdx((i) => Math.min(i + 1, options.length - 1));
          break;
        case "ArrowUp":
          e.preventDefault();
          setFocusIdx((i) => Math.max(i - 1, 0));
          break;
        case "Enter":
        case " ":
          e.preventDefault();
          if (focusIdx >= 0) {
            onChange(options[focusIdx].value);
            setOpen(false);
          }
          break;
        case "Escape":
          setOpen(false);
          break;
      }
    },
    [open, focusIdx, options, onChange, selectedIdx],
  );

  useEffect(() => {
    if (!open || focusIdx < 0 || !listRef.current) return;
    const items = listRef.current.querySelectorAll<HTMLLIElement>("[role='option']");
    items[focusIdx]?.scrollIntoView({ block: "nearest" });
  }, [focusIdx, open]);

  return (
    <div
      ref={containerRef}
      className={`select ${open ? "select--open" : ""} ${className ?? ""}`}
      onKeyDown={handleKeyDown}
    >
      <button
        type="button"
        className="select__trigger"
        onClick={() => {
          setOpen((o) => !o);
          setFocusIdx(selectedIdx >= 0 ? selectedIdx : 0);
        }}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={ariaLabel}
      >
        {iconTrigger ? (
          <svg className="select__icon" width="16" height="16" viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M4 4h16v2.172a2 2 0 0 1-.586 1.414L15 12v7l-6 2v-8.5L4.52 7.572A2 2 0 0 1 4 6.227z" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        ) : (
          <span className="select__value">{selected?.label ?? value}</span>
        )}
        <svg className="select__arrow" width="16" height="16" viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <path d="m6 9 6 6 6-6" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </button>
      {open && (
        <ul ref={listRef} className="select__menu" role="listbox" tabIndex={-1}>
          {options.map((opt, i) => (
            <li
              key={opt.value}
              role="option"
              aria-selected={opt.value === value}
              className={`select__option ${opt.value === value ? "select__option--selected" : ""} ${i === focusIdx ? "select__option--focused" : ""}`}
              onClick={() => {
                onChange(opt.value);
                setOpen(false);
              }}
              onMouseEnter={() => setFocusIdx(i)}
            >
              <span className="select__option-label">{opt.label}</span>
              {opt.value === value && (
                <svg className="select__check" width="14" height="14" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                  <path d="m5 12 5 5L20 7" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
                </svg>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
