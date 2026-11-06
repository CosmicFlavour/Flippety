import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { ArrowLeft } from "lucide-react";
import { useMutation } from "@tanstack/react-query";
import { api } from "@/lib/api";
import type { Deck, Direction, DueItem, Rating } from "@/types/models";
import { Button } from "@/components/ui/button";
import { Card as UiCard, CardHeader, CardTitle, CardContent } from "@/components/ui/card";

const RATINGS: Rating[] = ["Again", "Hard", "Good", "Easy"];

type View =
  | { kind: "loading" }
  | { kind: "item"; item: DueItem }
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
  const [studiedCount, setStudiedCount] = useState(0);
  // Guards the background prefetch so at most one is ever in flight. Rating
  // faster than a prefetch resolves means `prefetched` is still null on the
  // next rate — without this guard, every one of those would fire another
  // overlapping prefetch on top of the still-unresolved previous one, none
  // of them ever cancelled, piling up an ever-growing backlog of requests
  // behind the single backend connection (what made the wait after rating
  // grow the longer a session went on).
  const prefetchInFlightRef = useRef(false);

  const refillPrefetch = useCallback(
    (exclude: DueItem) => {
      if (prefetchInFlightRef.current) return;
      prefetchInFlightRef.current = true;
      fetchNext(deck.id, exclude)
        .then((result) => {
          if (result !== "none" && result !== "error") setPrefetched(result);
        })
        .finally(() => {
          prefetchInFlightRef.current = false;
        });
    },
    [deck.id],
  );

  // The single place that decides what to show next — called right after
  // rating a card. Always resolves to a defined view (has an item,
  // confirmed nothing available, or confirmed error), never leaves the UI
  // silently stuck.
  const advanceToNext = useCallback(async () => {
    if (prefetched) {
      const next = prefetched;
      setPrefetched(null);
      setView({ kind: "item", item: next });
      setRevealed(false);
      refillPrefetch(next);
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
      refillPrefetch(result);
    }
  }, [deck.id, prefetched, refillPrefetch]);

  useEffect(() => {
    advanceToNext();
    // eslint-disable-next-line react-hooks/exhaustive-deps -- fetch once on mount
  }, []);

  const submitReview = useMutation({
    mutationFn: (input: { card_id: string; direction: Direction; rating: Rating }) =>
      api.study.submitReview(input),
  });

  // Optimistic: the next card shows immediately on click, without waiting
  // on the round-trip — the review is persisted in the background.
  const rate = (rating: Rating) => {
    if (view.kind !== "item") return;
    const item = view.item;
    submitReview.mutate(
      { card_id: item.card_id, direction: item.direction, rating },
      { onSuccess: () => setStudiedCount((c) => c + 1) },
    );
    void advanceToNext();
  };

  const header = (
    <div className="flex items-center gap-2">
      <Button variant="ghost" size="icon-sm" aria-label="Back" onClick={onBack}>
        <ArrowLeft className="size-4" />
      </Button>
      <h1 className="text-xl font-semibold">{deck.name}</h1>
    </div>
  );

  let body: ReactNode;
  if (view.kind === "loading") {
    body = <p className="text-muted-foreground">Loading…</p>;
  } else if (view.kind === "nothing_available") {
    body = (
      <>
        <p className="text-muted-foreground">Nothing due right now. Nice work.</p>
        <Button variant="outline" onClick={() => void advanceToNext()}>
          Check again
        </Button>
        {studiedCount > 0 && (
          <p className="text-sm text-muted-foreground">{studiedCount} studied this session</p>
        )}
      </>
    );
  } else if (view.kind === "error") {
    body = (
      <>
        <p className="text-muted-foreground">Couldn't load more cards.</p>
        <Button variant="outline" onClick={() => void advanceToNext()}>
          Try again
        </Button>
      </>
    );
  } else {
    const item = view.item;
    body = (
      <>
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
              <Button key={rating} variant="outline" onClick={() => rate(rating)}>
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
      </>
    );
  }

  return (
    <div className="mx-auto flex max-w-xl flex-col gap-4 p-6">
      {header}
      {body}
    </div>
  );
}
