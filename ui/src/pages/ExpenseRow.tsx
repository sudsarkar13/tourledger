// How an expense reads in any list.
import { Paperclip, ReceiptText } from "lucide-react";
import { Badge, ListRow, Money } from "../components/ui";
import { day } from "../format";
import { CATEGORIES, categoryName, personName } from "../labels";
import { useApp } from "../state";
import type { Expense } from "../types";

export function ExpenseRow({ expense: e, showTour, onClick }: { expense: Expense; showTour?: boolean; onClick: () => void }) {
  const { data } = useApp();
  const tour = showTour ? (data.tours.find((t) => t.id === e.tour_id)?.name ?? "No tour") : null;
  const route = e.from && e.to ? `${e.from} → ${e.to}` : "";
  const title = e.description || route || categoryName(e.category);
  const trip = data.tours.find((t) => t.id === e.tour_id);
  const stage = !trip ? "" : e.date < trip.start_date ? "journey" : e.date > trip.end_date ? "return" : "";
  // The category is named when the title is the person's own note.
  const kind = e.description || route ? categoryName(e.category) : "";
  const detail = [day(e.date), stage, kind, e.description && route, e.mode, tour, `Paid by ${e.paid_by === "me" ? "me" : personName(data, e.paid_by)}`]
    .filter(Boolean)
    .join(" · ");
  const pictures = data.attachments.filter((a) => a.expense_id === e.id).length;
  return (
    <ListRow
      icon={CATEGORIES[e.category]?.icon ?? ReceiptText}
      title={title}
      detail={
        <>
          {detail}
          {pictures > 0 && (
            <span className="clip">
              <Paperclip size={13} aria-hidden /> {pictures} {pictures === 1 ? "picture" : "pictures"}
            </span>
          )}
        </>
      }
      value={<Money paise={e.home} />}
      note={
        e.paid_by === "company" ? (
          "Company paid"
        ) : e.rejected ? (
          <Badge tone="bad">Rejected</Badge>
        ) : e.claim === "reimbursable" && e.paid_by === "me" ? (
          "To claim"
        ) : e.split_with.length > 1 ? (
          "Shared"
        ) : undefined
      }
      onClick={onClick}
    />
  );
}
