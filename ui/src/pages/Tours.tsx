import { Briefcase, Luggage, Plus } from "lucide-react";
import { useState } from "react";
import { ClaimBadge, CompanySwitcher, useCompanyFilter } from "../components/shared";
import { Badge, Button, Card, EmptyState, List, ListRow, Money, PageHeader } from "../components/ui";
import { TourForm } from "../forms/TourForm";
import { dateRange, money } from "../format";
import { TOUR_STATUS } from "../labels";
import { useApp } from "../state";

export function Tours() {
  const { data } = useApp();
  const [adding, setAdding] = useState(false);
  const { tours } = useCompanyFilter();

  return (
    <>
      <PageHeader title="Tours" context="Events and trips, with what each one cost">
        <Button variant="primary" icon={Plus} onClick={() => setAdding(true)}>
          New tour
        </Button>
      </PageHeader>
      <CompanySwitcher />
      <Card>
        {tours.length ? (
          <List>
            {tours.map((t) => (
              <ListRow
                key={t.id}
                icon={t.kind === "company" ? Briefcase : Luggage}
                title={t.name}
                detail={[dateRange(t.start_date, t.end_date), t.city].filter(Boolean).join(" · ")}
                value={<Money paise={t.summary.total_cost} />}
                note={
                  <>
                    {t.kind === "company" && t.status !== "settled" && t.summary.net !== 0 && (
                      <span>{money(Math.abs(t.summary.net))} </span>
                    )}
                    {t.claim_state ? <ClaimBadge state={t.claim_state} /> : <Badge>{TOUR_STATUS[t.status]}</Badge>}
                  </>
                }
                href={`#/tours/${t.id}`}
              />
            ))}
          </List>
        ) : (
          <EmptyState
            icon={Luggage}
            title={
              data.tours.length
                ? "No tours here. Change Company above to see the others."
                : "No tours yet. Create one before you travel, then add costs as they happen."
            }
          />
        )}
      </Card>
      {adding && <TourForm onClose={() => setAdding(false)} />}
    </>
  );
}
