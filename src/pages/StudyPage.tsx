import { useMemo, useState } from "react";
import { ArrowLeft } from "lucide-react";
import { useMutation, useQueries, useQuery } from "@tanstack/react-query";
import { api } from "@/lib/api";
import type { Deck, DueItem, Rating } from "@/types/models";
import { Button } from "@/components/ui/button";
import { Card as UiCard, CardHeader, CardTitle, CardContent } from "@/components/ui/card";

const RATINGS: Rating[] = ["Again", "Hard", "Good", "Easy"];
// How many (card_id, direction) refs to hydrate with full card content per
// request — keeps a session with hundreds of due items from fetching every
// card's text up front.
const BATCH_SIZE = 30;
// Start fetching the next batch this many items before the loaded ones run out.
const PREFETCH_THRESHOLD = 5;
const AHEAD_HOURS = 24;

export function StudyPage({ deck, onBack }: { deck: Deck; onBack: () => void }) {
  const [index, setIndex] = useState(0);
  const [revealed, setRevealed] = useState(false);
  // How many extra cap-sized batches of new cards the user has opted into —
  // 0 means none yet. Each click reveals one more batch rather than the
  // entire remaining backlog at once.
  const [bonusNewBatchesRequested, setBonusNewBatchesRequested] = useState(0);
  const [aheadActivated, setAheadActivated] = useState(false);

  // Fetched fresh on every visit to this page (so completing a session and
  // coming back later shows what's newly due, not a stale snapshot), but not
  // refetched while the page stays mounted — it's just lightweight
  // (card_id, direction) refs, and re-querying "what's due" as ratings are
  // submitted would reshuffle the remaining items out from under an
  // in-progress batch-by-batch hydration.
  const manifestQuery = useQuery({
    queryKey: ["due", deck.id, "manifest"],
    queryFn: () => api.study.dueQueue(deck.id),
    refetchOnWindowFocus: false,
  });

  const mainRefs = useMemo(
    () => (manifestQuery.data ? [...manifestQuery.data.due, ...manifestQuery.data.new] : []),
    [manifestQuery.data],
  );
  const mainNewCount = manifestQuery.data?.new.length ?? 0;

  // These double as a "peek" (to size the end-of-session buttons) and, once
  // the user opts in, the actual bonus data source — enabling the query
  // doesn't commit to using it until `*Activated`/`*BatchesRequested` flips.
  const mayBeExhausted = manifestQuery.isSuccess && index >= mainRefs.length;

  const bonusNewQuery = useQuery({
    queryKey: ["due", deck.id, "bonusNew"],
    queryFn: () => api.study.bonusNewCards(deck.id),
    enabled: mayBeExhausted || bonusNewBatchesRequested > 0,
    refetchOnWindowFocus: false,
  });
  // The main queue's new-card block is a guaranteed prefix of this uncapped
  // list (same deterministic order, cap just ignored) — drop it so this is
  // only the cards not already shown.
  const bonusNewRefs = useMemo(
    () => (bonusNewQuery.data ?? []).slice(mainNewCount),
    [bonusNewQuery.data, mainNewCount],
  );
  // Each "more new cards" click pulls one more batch the size of the deck's
  // own daily cap, rather than the entire remaining backlog at once — a
  // deck capped at 20/day should still only offer 20 more at a time. With no
  // cap set, there's nothing left in `bonusNewRefs` anyway (the main queue
  // already pulled every new card), so the fallback is moot.
  const bonusBatchSize = deck.new_cards_per_day ?? bonusNewRefs.length;
  const bonusNewVisibleCount = Math.min(bonusNewBatchesRequested * bonusBatchSize, bonusNewRefs.length);
  const bonusNewVisibleRefs = bonusNewRefs.slice(0, bonusNewVisibleCount);
  const nextBonusBatchSize = Math.min(bonusBatchSize, bonusNewRefs.length - bonusNewVisibleCount);

  const aheadQuery = useQuery({
    queryKey: ["due", deck.id, "ahead", AHEAD_HOURS],
    queryFn: () => api.study.aheadReviews(deck.id, AHEAD_HOURS),
    enabled: mayBeExhausted || aheadActivated,
    refetchOnWindowFocus: false,
  });
  const aheadRefs = aheadQuery.data ?? [];

  const allRefs = useMemo(
    () => [
      ...mainRefs,
      ...(bonusNewBatchesRequested > 0 ? bonusNewVisibleRefs : []),
      ...(aheadActivated ? aheadRefs : []),
    ],
    [mainRefs, bonusNewBatchesRequested, bonusNewVisibleRefs, aheadActivated, aheadRefs],
  );

  const neededBatches = Math.min(
    Math.ceil((index + PREFETCH_THRESHOLD + 1) / BATCH_SIZE),
    Math.ceil(allRefs.length / BATCH_SIZE),
  );
  const batchIndices = Array.from({ length: Math.max(neededBatches, 0) }, (_, i) => i);

  // Keyed by this visit's manifest fetch (not just the batch index) so a
  // fresh visit — with a freshly-refetched manifest — hydrates fresh card
  // content instead of reusing another visit's cached batches, which could
  // now correspond to different refs.
  const manifestVersion = manifestQuery.dataUpdatedAt;
  const batchQueries = useQueries({
    queries: batchIndices.map((i) => ({
      queryKey: ["due", deck.id, "hydrate", manifestVersion, i],
      queryFn: () => api.study.queueCards(allRefs.slice(i * BATCH_SIZE, (i + 1) * BATCH_SIZE)),
      staleTime: Infinity,
      refetchOnWindowFocus: false,
    })),
  });
  const hydrated: DueItem[] = batchQueries.flatMap((q) => q.data ?? []);
  const hydrating = batchQueries.some((q) => q.isLoading || q.isFetching);

  const submitReview = useMutation({
    mutationFn: (rating: Rating) => {
      const item = hydrated[index];
      return api.study.submitReview({
        card_id: item.card_id,
        direction: item.direction,
        rating,
      });
    },
    onSuccess: () => {
      setIndex((i) => i + 1);
      setRevealed(false);
    },
  });

  const header = (
    <div className="flex items-center gap-2">
      <Button variant="ghost" size="icon-sm" aria-label="Back" onClick={onBack}>
        <ArrowLeft className="size-4" />
      </Button>
      <h1 className="text-xl font-semibold">{deck.name}</h1>
    </div>
  );

  if (manifestQuery.isLoading) {
    return (
      <div className="mx-auto flex max-w-xl flex-col gap-4 p-6">
        {header}
        <p className="text-muted-foreground">Loading…</p>
      </div>
    );
  }

  const item = hydrated[index];
  const sessionExhausted = index >= allRefs.length && !hydrating;
  const aheadRemaining = aheadRefs.length;

  return (
    <div className="mx-auto flex max-w-xl flex-col gap-4 p-6">
      {header}

      {sessionExhausted && (
        <div className="flex flex-col gap-3">
          <p className="text-muted-foreground">Nothing due right now. Nice work.</p>
          {nextBonusBatchSize > 0 && (
            <Button variant="outline" onClick={() => setBonusNewBatchesRequested((n) => n + 1)}>
              Study {nextBonusBatchSize} more new card{nextBonusBatchSize === 1 ? "" : "s"} today
            </Button>
          )}
          {aheadRemaining > 0 && !aheadActivated && (
            <Button variant="outline" onClick={() => setAheadActivated(true)}>
              Study {aheadRemaining} card{aheadRemaining === 1 ? "" : "s"} early (next 24h)
            </Button>
          )}
        </div>
      )}

      {!sessionExhausted && !item && <p className="text-muted-foreground">Loading…</p>}

      {item && (
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
      )}

      {item && !revealed && <Button onClick={() => setRevealed(true)}>Reveal</Button>}

      {item && revealed && (
        <div className="grid grid-cols-4 gap-2">
          {RATINGS.map((rating) => (
            <Button
              key={rating}
              variant="outline"
              disabled={submitReview.isPending}
              onClick={() => submitReview.mutate(rating)}
            >
              {rating}
            </Button>
          ))}
        </div>
      )}

      {allRefs.length > 0 && (
        <p className="text-sm text-muted-foreground">
          {Math.min(index + 1, allRefs.length)} / {allRefs.length}
        </p>
      )}
    </div>
  );
}
