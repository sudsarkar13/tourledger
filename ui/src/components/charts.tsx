// One measure per chart, one hue; categories are told apart by their labels.
import { useState } from "react";
import { money, monthName, shortMoney } from "../format";
import { categoryName } from "../labels";

export function MonthBars({ months, start = "last" }: { months: { month: string; value: number }[]; start?: "first" | "last" }) {
  const [active, setActive] = useState<string | null>(null);
  const max = Math.max(...months.map((m) => m.value), 1);
  const shown = months.find((m) => m.month === active) ?? months.at(start === "last" ? -1 : 0);
  const peak = months.reduce((a, b) => (b.value > a.value ? b : a), months[0]);

  return (
    <figure className="month-chart">
      <figcaption aria-live="polite">
        {shown && (
          <>
            <span className="muted">{monthName(shown.month, true)}</span> <strong>{money(shown.value)}</strong>
          </>
        )}
      </figcaption>
      <div className="month-bars">
        {months.map((m, i) => (
          <button
            key={m.month}
            type="button"
            className={m.month === shown?.month ? "month active" : "month"}
            aria-label={`${monthName(m.month, true)}: ${money(m.value)}`}
            onPointerEnter={() => setActive(m.month)}
            onFocus={() => setActive(m.month)}
            onClick={() => setActive(m.month)}
          >
            <span className="plot">
              {m.value > 0 && m.month === peak.month && <small>{shortMoney(m.value)}</small>}
              <span className="bar" style={{ height: `${(m.value / max) * 100}%` }} />
            </span>
            {/* Every other label on narrow screens, ending with this month. */}
            <span className={(months.length - 1 - i) % 2 ? "tick odd" : "tick"}>{monthName(m.month)}</span>
          </button>
        ))}
      </div>
      <details>
        <summary>Show as table</summary>
        <table>
          <thead>
            <tr>
              <th>Month</th>
              <th>Amount</th>
            </tr>
          </thead>
          <tbody>
            {[...months].reverse().map((m) => (
              <tr key={m.month}>
                <td>{monthName(m.month, true)}</td>
                <td>{money(m.value)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </details>
    </figure>
  );
}

export function CategoryBars({
  values,
  name = categoryName,
}: {
  values: Record<string, number>;
  name?: (id: string) => string;
}) {
  const rows = Object.entries(values)
    .filter(([, v]) => v > 0)
    .sort((a, b) => b[1] - a[1]);
  const max = rows[0]?.[1] ?? 1;
  const total = rows.reduce((sum, [, v]) => sum + v, 0);
  return (
    <ul className="category-bars">
      {rows.map(([id, value]) => (
        <li key={id} title={`${name(id)}: ${money(value)}`}>
          <span>{name(id)}</span>
          <span className="money">
            {money(value)} <small className="muted">{Math.round((value / total) * 100)}%</small>
          </span>
          <span className="track">
            <span className="fill" style={{ transform: `scaleX(${value / max})` }} />
          </span>
        </li>
      ))}
    </ul>
  );
}

/** Part of a limit used. The words carry the state; colour only repeats it. */
export function Meter({
  label,
  used,
  limit,
  plain,
}: {
  label: string;
  used: number;
  limit: number;
  /** Progress, not a limit: filling up is no warning. */
  plain?: boolean;
}) {
  const share = limit > 0 ? used / limit : 0;
  const tone = plain ? "ok" : share > 1 ? "bad" : share >= 0.8 ? "warn" : "ok";
  return (
    <div className={`meter ${tone}`}>
      <span>{label}</span>
      <span className="money">
        {money(used)} <small className="muted">of {money(limit)}</small>
      </span>
      <span className="track" role="meter" aria-label={label} aria-valuemin={0} aria-valuemax={limit} aria-valuenow={used}>
        <span className="fill" style={{ transform: `scaleX(${Math.min(share, 1)})` }} />
      </span>
      {tone !== "ok" && <small>{tone === "bad" ? `▲ Over by ${money(used - limit)}` : "● Nearly used"}</small>}
    </div>
  );
}
