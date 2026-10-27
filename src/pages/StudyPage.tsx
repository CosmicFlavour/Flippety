import { useCallback, useEffect, useState } from "react";
import { ArrowLeft } from "lucide-react";
import { useMutation } from "@tanstack/react-query";
import { api } from "@/lib/api";
import type { Deck, Direction, DueItem, Rating } from "@/types/models";
import { Button } from "@/components/ui/button";
import { Card as UiCard, CardHeader, CardTitle, CardContent } from "@/components/ui/card";

const RATINGS: Rating[] = ["Again", "Hard", "Good", "Easy"];

// Non-blocking encouragement/pacing copy, keyed by the deck's running count
// of new cards introduced today (see SubmitReviewResult.milestone) — the
// backend only decides *whether* a milestone fired (every multiple of 5);
// what it says is presentation, so it lives here. Descending thresholds
// mean any count past 25 naturally keeps the last tier's copy.
const MILESTONE_TIERS: { threshold: number; text: string }[] = [
  { threshold: 25, text: "That's probably too much" },
  { threshold: 20, text: "Maybe take a break" },
  { threshold: 15, text: "Going strong" },
  { threshold: 10, text: "On a streak!" },
  { threshold: 5, text: "Good job" },
];

function milestoneCopy(count: number): string {
  return MILESTONE_TIERS.find((tier) => count >= tier.threshold)?.text ?? "Good job";
}

type View =
  | { kind: "loading" }
  | { kind: "item"; item: DueItem }
  | { kind: "milestone"; count: number }
  | { kind: "nothing_available" }
  | { kind: "error" };

// Fetches the single next item to study. `exclude` asks for "the item after
// the one already on screen" — nothing in the DB changes between showing an
// item and submitting its review, so without this, fetching one item ahead
// of an unanswered one would deterministically return that same item again.
async function fetchNext(deckId: string, exclude?: DueItem): Promise<DueItem | "none" | "error"> {
  try {
    const item = await api.study.nextItem(
      deckId,
      exclude ? { card_id: exclude.card_id, direction: exclude.direction } : undefined,
    );
    return item ?? "none";
  } catch {
    return "error";
  }
}

export function StudyPage({ deck, onBack }: { deck: Deck; onBack: () => void }) {
  const [view, setView] = useState<View>({ kind: "loading" });
  // One item fetched ahead of whatever's showing, so the next card is
  // usually ready the instant the current one is rated.
  const [prefetched, setPrefetched] = useState<DueItem | null>(null);
  const [revealed, setRevealed] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [studiedCount, setStudiedCount] = useState(0);

  // The single place that decides what to show next — called right after a
  // non-milestone rating and from the milestone screen's "Continue" button.
  // Always resolves to a defined view (has an item, confirmed nothing
  // available, or confirmed error), never leaves the UI silently stuck.
  const advanceToNext = useCallback(async () => {
    if (prefetched) {
      const next = prefetched;
      setPrefetched(null);
      setView({ kind: "item", item: next });
      setRevealed(false);
      void fetchNext(deck.id, next).then((result) => {
        if (result !== "none" && result !== "error") setPrefetched(result);
      });
      return;
    }
    setView({ kind: "loading" });
    const result = await fetchNext(deck.id);
    if (result === "none") {
      setView({ kind: "nothing_available" });
    } else if (result === "error") {
      setView({ kind: "error" });
    } else {
      setView({ kind: "item", item: result });
      setRevealed(false);
      void fetchNext(deck.id, result).then((r) => {
        if (r !== "none" && r !== "error") setPrefetched(r);
      });
    }
  }, [deck.id, prefetched]);

  useEffect(() => {
    advanceToNext();
    // eslint-disable-next-line react-hooks/exhaustive-deps -- fetch once on mount
  }, []);

  const submitReview = useMutation({
    mutationFn: (input: { card_id: string; direction: Direction; rating: Rating }) =>
      api.study.submitReview(input),
  });

  const rate = async (rating: Rating) => {
    if (view.kind !== "item") return;
    const item = view.item;
    setSubmitting(true);
    try {
      const result = await submitReview.mutateAsync({
        card_id: item.card_id,
        direction: item.direction,
        rating,
      });
      setStudiedCount((c) => c + 1);
      if (result.milestone != null) {
        setView({ kind: "milestone", count: result.milestone });
      } else {
        await advanceToNext();
      }
    } catch {
      // Rating failed to save (surfaced via submitReview.isError below) —
      // still advance so the session doesn't stall on a transient error.
      await advanceToNext();
    } finally {
      setSubmitting(false);
    }
  };

  const header = (
    <div className="flex items-center gap-2">
      <Button variant="ghost" size="icon-sm" aria-label="Back" onClick={onBack}>
        <ArrowLeft className="size-4" />
      </Button>
      <h1 className="text-xl font-semibold">{deck.name}</h1>
    </div>
  );

  if (view.kind === "loading") {
    return (
      <div className="mx-auto flex max-w-xl flex-col gap-4 p-6">
        {header}
        <p className="text-muted-foreground">Loading…</p>
      </div>
    );
  }

  if (view.kind === "milestone") {
    return (
      <div className="mx-auto flex max-w-xl flex-col gap-4 p-6">
        {header}
        <p className="text-lg font-medium">
          {view.count} new cards today — {milestoneCopy(view.count)}
        </p>
        <Button onClick={() => void advanceToNext()}>Continue</Button>
      </div>
    );
  }

  if (view.kind === "nothing_available") {
    return (
      <div className="mx-auto flex max-w-xl flex-col gap-4 p-6">
        {header}
        <p className="text-muted-foreground">Nothing due right now. Nice work.</p>
        <Button variant="outline" onClick={() => void advanceToNext()}>
          Check again
        </Button>
        {studiedCount > 0 && (
          <p className="text-sm text-muted-foreground">{studiedCount} studied this session</p>
        )}
      </div>
    );
  }

  if (view.kind === "error") {
    return (
      <div className="mx-auto flex max-w-xl flex-col gap-4 p-6">
        {header}
        <p className="text-muted-foreground">Couldn't load more cards.</p>
        <Button variant="outline" onClick={() => void advanceToNext()}>
          Try again
        </Button>
      </div>
    );
  }

  const item = view.item;
  return (
    <div className="mx-auto flex max-w-xl flex-col gap-4 p-6">
      {header}

      <UiCard>
        <CardHeader>
          <CardTitle className="text-2xl">{item.prompt}</CardTitle>
        </CardHeader>
        {revealed && (
          <CardContent className="flex flex-col gap-2">
            <div className="text-lg font-medium">{item.full.title}</div>
            {item.full.subtitle && (
              <div className="text-muted-foreground">{item.full.subtitle}</div>
            )}
            {item.full.body && <p className="whitespace-pre-line">{item.full.body}</p>}
            {item.full.foot && (
              <p className="whitespace-pre-line text-muted-foreground">{item.full.foot}</p>
            )}
          </CardContent>
        )}
      </UiCard>

      {!revealed && <Button onClick={() => setRevealed(true)}>Reveal</Button>}

      {revealed && (
        <div className="grid grid-cols-4 gap-2">
          {RATINGS.map((rating) => (
            <Button
              key={rating}
              variant="outline"
              disabled={submitting}
              onClick={() => rate(rating)}
            >
              {rating}
            </Button>
          ))}
        </div>
      )}

      {submitReview.isError && (
        <p className="text-sm text-destructive">
          Couldn't save your last rating — it may need to be repeated.
        </p>
      )}

      {studiedCount > 0 && (
        <p className="text-sm text-muted-foreground">{studiedCount} studied this session</p>
      )}
    </div>
  );
}
