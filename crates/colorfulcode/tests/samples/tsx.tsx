import { useMemo, useState, type ReactNode } from "react";

type Props<T> = {
  items: readonly T[];
  render: (item: T, index: number) => ReactNode;
  emptyText?: string;
};

// Generic arrow functions are tricky in TSX.
const identity = <T,>(value: T): T => value;
const byKey = <T extends object, K extends keyof T>(key: K) => (a: T, b: T) =>
  a[key] < b[key] ? -1 : a[key] > b[key] ? 1 : 0;
const withDefault = <T = string,>(value: T | undefined, fallback: T) => value ?? fallback;

export function List<T>({ items, render, emptyText = "Nothing here" }: Props<T>) {
  const [query, setQuery] = useState("");
  const visible = useMemo(
    () => items.filter((item) => String(item).toLowerCase().includes(query)),
    [items, query],
  );

  if (visible.length === 0) {
    return <p className="empty">{emptyText}</p>;
  }

  return (
    <section>
      <input value={query} onChange={(e) => setQuery(e.target.value)} />
      <ul>
        {visible.map((item, i) => (
          <li key={i} data-index={i}>
            {render(identity(item), i)}
          </li>
        ))}
      </ul>
      <Select<number> options={[1, 2, 3]} onPick={(n) => console.log(`picked ${n * 2}`)} />
    </section>
  );
}

const pattern = /^[a-z]+\d*$/gi;
const ratio = total / count / 2;
export default withDefault;
