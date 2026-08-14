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
  const [bonusNewActivated, setBonusNewActivated] = useState(false);
  const [aheadActivated, setAheadActivated] = useState(false);

  // The manifest is fetched once and never refetched mid-session: it's just
  // lightweight (card_id, direction) refs, and re-querying "what's due" as
  // ratings are submitted would reshuffle the remaining items out from under
  // an in-progress batch-by-batch hydration.
  const manifestQuery = useQuery({
    queryKey: ["due", deck.id, "manifest"],
    queryFn: () => api.study.dueQueue(deck.id),
    staleTime: Infinity,
    refetchOnWindowFocus: false,
  });

  const mainRefs = useMemo(
    () => (manifestQuery.data ? [...manifestQuery.data.due, ...manifestQuery.data.new] : []),
    [manifestQuery.data],
  );
  const mainNewCount = manifestQuery.data?.new.length ?? 0;

  // These double as a "peek" (to size the end-of-session buttons) and, once
  // the user opts in, the actual bonus data source — enabling the query
  // doesn't commit to using it until `*Activated` flips.
  const mayBeExhausted = manifestQuery.isSuccess && index >= mainRefs.length;

  const bonusNewQuery = useQuery({
    queryKey: ["due", deck.id, "bonusNew"],
    queryFn: () => api.study.bonusNewCards(deck.id),
    enabled: mayBeExhausted || bonusNewActivated,
    staleTime: Infinity,
    refetchOnWindowFocus: false,
  });
  // The main queue's new-card block is a guaranteed prefix of this uncapped
  // list (same deterministic order, cap just ignored) — drop it so this is
  // only the cards not already shown.
  const bonusNewRefs = useMemo(
    () => (bonusNewQuery.data ?? []).slice(mainNewCount),
    [bonusNewQuery.data, mainNewCount],
  );

  const aheadQuery = useQuery({
    queryKey: ["due", deck.id, "ahead", AHEAD_HOURS],
    queryFn: () => api.study.aheadReviews(deck.id, AHEAD_HOURS),
    enabled: mayBeExhausted || aheadActivated,
    staleTime: Infinity,
    refetchOnWindowFocus: false,
  });
  const aheadRefs = aheadQuery.data ?? [];

  const allRefs = useMemo(
    () => [
      ...mainRefs,
      ...(bonusNewActivated ? bonusNewRefs : []),
      ...(aheadActivated ? aheadRefs : []),
    ],
    [mainRefs, bonusNewActivated, bonusNewRefs, aheadActivated, aheadRefs],
  );

  const neededBatches = Math.min(
    Math.ceil((index + PREFETCH_THRESHOLD + 1) / BATCH_SIZE),
    Math.ceil(allRefs.length / BATCH_SIZE),
  );
  const batchIndices = Array.from({ length: Math.max(neededBatches, 0) }, (_, i) => i);

  const batchQueries = useQueries({
    queries: batchIndices.map((i) => ({
      queryKey: ["due", deck.id, "hydrate", i],
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
  const bonusNewRemaining = bonusNewRefs.length;
  const aheadRemaining = aheadRefs.length;

  return (
    <div className="mx-auto flex max-w-xl flex-col gap-4 p-6">
      {header}

      {sessionExhausted && (
        <div className="flex flex-col gap-3">
          <p className="text-muted-foreground">Nothing due right now. Nice work.</p>
          {bonusNewRemaining > 0 && !bonusNewActivated && (
            <Button variant="outline" onClick={() => setBonusNewActivated(true)}>
              Study {bonusNewRemaining} more new card{bonusNewRemaining === 1 ? "" : "s"} today
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
