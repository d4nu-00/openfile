import type { Diagnostic } from "../api/types";
export function Diagnostics({ items }: { items: Diagnostic[] }) {
  return (
    <div className="diagnostics" role="status">
      {items.map((d, i) => (
        <p key={i}>
          {d.message}
          {d.offset ? ` (offset ${d.offset})` : ""}
        </p>
      ))}
    </div>
  );
}
