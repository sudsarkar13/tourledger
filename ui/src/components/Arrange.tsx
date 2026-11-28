// Dragging a tour's entries into the order its report lists them. The headings
// and the starting order come from crates/core/src/arrange.rs.
import {
  closestCenter,
  DndContext,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
  type UniqueIdentifier,
} from "@dnd-kit/core";
import { arrayMove, SortableContext, sortableKeyboardCoordinates, useSortable, verticalListSortingStrategy } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { ArrowDownUp, GripVertical } from "lucide-react";
import { useState } from "react";
import { api } from "../api";
import { day } from "../format";
import { personName } from "../labels";
import { expenseTitle } from "../pages/ExpenseRow";
import { useApp } from "../state";
import type { Expense, Tour } from "../types";
import { Button, Money, Sheet } from "./ui";

function Row({ expense, payer, left }: { expense: Expense; payer: string; left: boolean }) {
  const { attributes, listeners, setNodeRef, setActivatorNodeRef, transform, transition, isDragging } = useSortable({
    id: expense.id,
  });
  const title = expenseTitle(expense);
  return (
    <li
      ref={setNodeRef}
      className={isDragging ? "dragging" : undefined}
      // Only up and down, and never stretched to the size of a neighbour.
      style={{ transform: CSS.Translate.toString(transform && { ...transform, x: 0 }), transition }}
    >
      <button type="button" className="drag-handle" ref={setActivatorNodeRef} {...attributes} {...listeners} aria-label={`Move ${title}`}>
        <GripVertical size={20} aria-hidden />
      </button>
      <span className="row-text">
        <span className="row-title">{title}</span>
        <span className="muted row-detail">
          {day(expense.date)} · Paid by {payer}
          {left && " · not on the claim"}
        </span>
      </span>
      <span className="row-value">
        <Money paise={expense.home} />
      </span>
    </li>
  );
}

/** A company claim lists what I ask back and what the company paid, no more. */
const leftOut = (e: Expense) => e.paid_by !== "company" && !(e.paid_by === "me" && e.claim === "reimbursable" && !e.rejected);

function Section({
  name,
  claim,
  entries,
  onMove,
}: {
  name: string;
  claim: boolean;
  entries: Expense[];
  onMove: (from: number, to: number) => void;
}) {
  const { data } = useApp();
  const sensors = useSensors(
    // A few pixels first, so a tap on the handle moves nothing.
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );
  const ids = entries.map((e) => e.id);
  // Read aloud by title and place, not by the entry's internal id.
  const named = (id: UniqueIdentifier) => {
    const entry = entries.find((e) => e.id === id);
    return entry ? expenseTitle(entry) : "the entry";
  };
  const place = (id: UniqueIdentifier) => `${ids.indexOf(String(id)) + 1} of ${ids.length}`;
  const end = ({ active, over }: DragEndEvent) => {
    if (over && active.id !== over.id) onMove(ids.indexOf(String(active.id)), ids.indexOf(String(over.id)));
  };
  return (
    <section aria-label={name}>
      <h3>{name}</h3>
      <DndContext
        sensors={sensors}
        collisionDetection={closestCenter}
        onDragEnd={end}
        accessibility={{
          screenReaderInstructions: { draggable: "Press space to pick up, the arrow keys to move, and space to drop." },
          announcements: {
            onDragStart: ({ active }) => `Picked up ${named(active.id)}, ${place(active.id)}.`,
            onDragOver: ({ active, over }) => (over ? `${named(active.id)} is at ${place(over.id)}.` : undefined),
            onDragEnd: ({ active, over }) => (over ? `${named(active.id)} dropped at ${place(over.id)}.` : undefined),
            onDragCancel: ({ active }) => `${named(active.id)} put back.`,
          },
        }}
      >
        <SortableContext items={ids} strategy={verticalListSortingStrategy}>
          <ol className="sortable">
            {entries.map((e) => (
              <Row
                key={e.id}
                expense={e}
                payer={e.paid_by === "me" ? "me" : personName(data, e.paid_by)}
                left={claim && leftOut(e)}
              />
            ))}
          </ol>
        </SortableContext>
      </DndContext>
    </section>
  );
}

function ArrangeSheet({ tour, onClose }: { tour: Tour; onClose: () => void }) {
  const { data, reload, notify, confirm } = useApp();
  const [error, setError] = useState("");
  const [sections, setSections] = useState(tour.sections);
  const [busy, setBusy] = useState(false);
  const byId = new Map(data.expenses.map((e) => [e.id, e]));

  const store = async (ids: string[], done: string) => {
    setBusy(true);
    try {
      await api({ op: "arrange_expenses", tour_id: tour.id, ids });
      await reload();
      notify(done);
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setBusy(false);
    }
  };

  const reset = async () => {
    const sure = await confirm("Go back to date order?", "The order you chose for this tour is forgotten.", "Use date order");
    if (sure) await store([], "Back in date order");
  };

  return (
    <Sheet title="Arrange entries" onClose={onClose}>
      <p className="muted">
        Drag by the handle. This is the order under each heading of the report: listed one by one with "Every entry", and
        followed by the bill picture numbers either way.
      </p>
      {sections.map((s, at) => (
        <Section
          key={s.name}
          name={s.name}
          claim={tour.kind === "company"}
          // One removed on another device meanwhile is simply not shown.
          entries={s.ids.flatMap((id) => byId.get(id) ?? [])}
          onMove={(from, to) =>
            setSections((current) =>
              current.map((c, i) => {
                if (i !== at) return c;
                const shown = c.ids.filter((id) => byId.has(id));
                return { ...c, ids: arrayMove(shown, from, to) };
              }),
            )
          }
        />
      ))}
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      <div className="form-actions">
        {tour.order.length > 0 && (
          <Button variant="ghost" disabled={busy} onClick={reset}>
            Back to date order
          </Button>
        )}
        <Button variant="primary" disabled={busy} onClick={() => store(sections.flatMap((s) => s.ids), "Order saved")}>
          Save order
        </Button>
      </div>
    </Sheet>
  );
}

/** Offered once a heading has two entries to put in order. */
export function Arrange({ tour }: { tour: Tour }) {
  const [open, setOpen] = useState(false);
  // Once open it stays, even if a sync leaves nothing to put in order.
  if (!open && !tour.sections.some((s) => s.ids.length > 1)) return null;
  return (
    <>
      <Button icon={ArrowDownUp} onClick={() => setOpen(true)}>
        Arrange
      </Button>
      {open && <ArrangeSheet tour={tour} onClose={() => setOpen(false)} />}
    </>
  );
}
