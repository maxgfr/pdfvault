import type { Field, Options } from "@pdfvault/engine";

export function visible(field: Field, options: Options): boolean {
  if (!field.when) return true;
  const v = options[field.when.field];
  return field.when.is.some((x) => String(x) === String(v));
}

interface Props {
  field: Field;
  value: Options[string];
  onChange: (value: Options[string]) => void;
  idPrefix: string;
}

export function FieldInput({ field, value, onChange, idPrefix }: Props) {
  const id = `${idPrefix}-${field.name}`;
  const help = field.help ? <small id={`${id}-help`}>{field.help}</small> : null;
  const describedBy = field.help ? `${id}-help` : undefined;

  if (field.kind === "boolean") {
    return (
      <label className="check" htmlFor={id}>
        <input id={id} type="checkbox" checked={value === true} onChange={(e) => onChange(e.currentTarget.checked)} aria-describedby={describedBy} />
        <span>{field.label}</span>
        {help}
      </label>
    );
  }

  let control;
  switch (field.kind) {
    case "select":
      control = (
        <select id={id} value={String(value ?? field.default)} onChange={(e) => onChange(e.currentTarget.value)} aria-describedby={describedBy}>
          {field.choices.map((c) => (
            <option key={c.value} value={c.value}>
              {c.label}
            </option>
          ))}
        </select>
      );
      break;
    case "number":
      control = (
        <div className="with-unit">
          <input
            id={id}
            type="number"
            inputMode="decimal"
            min={field.min}
            max={field.max}
            step={field.step ?? 1}
            value={value === undefined ? "" : String(value)}
            placeholder={field.default === undefined ? "auto" : undefined}
            onChange={(e) => onChange(e.currentTarget.value === "" ? undefined : Number(e.currentTarget.value))}
            aria-describedby={describedBy}
          />
          {field.unit ? <span className="unit">{field.unit}</span> : null}
        </div>
      );
      break;
    case "color":
      control = (
        <div className="color">
          <input id={id} type="color" value={String(value ?? field.default ?? "#000000")} onChange={(e) => onChange(e.currentTarget.value)} />
          <code>{String(value ?? field.default ?? "")}</code>
        </div>
      );
      break;
    case "textarea":
      control = <textarea id={id} value={String(value ?? "")} placeholder={field.placeholder} onChange={(e) => onChange(e.currentTarget.value || undefined)} />;
      break;
    default:
      control = (
        <input
          id={id}
          type={field.kind === "password" ? "password" : "text"}
          autoComplete={field.kind === "password" ? "off" : undefined}
          spellCheck={field.kind === "pages" ? false : undefined}
          value={String(value ?? "")}
          placeholder={field.placeholder}
          required={field.required}
          onChange={(e) => onChange(e.currentTarget.value === "" ? undefined : e.currentTarget.value)}
          aria-describedby={describedBy}
        />
      );
  }
  return (
    <div className="field">
      <div className="label-row">
        <label htmlFor={id}>{field.label}</label>
        {"required" in field && field.required ? <span className="req">required</span> : null}
      </div>
      {control}
      {help}
    </div>
  );
}
