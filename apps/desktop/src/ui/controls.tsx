import type { ReactNode } from "react";

/** 分段选择：选中项是实心深色，一眼能看出来 */
export function Seg<V extends string | number>({
  value,
  options,
  onChange,
  label,
  disabled,
  small,
}: {
  value: V;
  options: [V, ReactNode][];
  onChange: (v: V) => void;
  label: string;
  disabled?: boolean;
  small?: boolean;
}) {
  return (
    <div className={`seg ${small ? "seg-sm" : ""}`} role="radiogroup" aria-label={label}>
      {options.map(([v, text]) => (
        <button
          key={String(v)}
          role="radio"
          aria-checked={value === v}
          disabled={disabled}
          onClick={() => onChange(v)}
        >
          {text}
        </button>
      ))}
    </div>
  );
}

/** 开关：打开时滑块上的指示灯亮起 */
export function Switch({ on, onChange, label }: { on: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <button className="switch" role="switch" aria-checked={on} aria-label={label} onClick={() => onChange(!on)}>
      <span className="knob" />
    </button>
  );
}

/** 设置页里的一行：左列标签、右列控件，说明文字对齐在控件下方 */
export function Row({ label, help, children, stack }: { label: ReactNode; help?: ReactNode; children: ReactNode; stack?: boolean }) {
  return (
    <div className={`setrow ${stack ? "stack" : ""}`}>
      <div className="setrow-label">{label}</div>
      <div className="setrow-control">{children}</div>
      {help && <div className="setrow-help">{help}</div>}
    </div>
  );
}
