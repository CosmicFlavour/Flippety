import { useCallback, useEffect, useRef, useState } from "react";
import { ArrowLeft } from "lucide-react";
import { useMutation } from "@tanstack/react-query";
import { api } from "@/lib/api";
import type { Deck, DueItem, Rating } from "@/types/models";
import { Button } from "@/components/ui/button";
import { Card as UiCard, CardHeader, CardTitle, CardContent } from "@/components/ui/card";

const RATINGS: Rating[] = ["Again", "Hard", "Good", "Easy"];
// How many items to pull per fetch. FSRS can make an item due again within
// minutes (short-term learning steps), so there's no fixed "today's set" to
// precompute — small batches keep what's on screen close to what's actually
// due right now, without fetching after every single card.
const BATCH_SIZE = 10;
// Start fetching the next batch this many items before the buffer runs out.
const PREFETCH_THRESHOLD = 3;

export function StudyPage({ deck, onBack }: { deck: Deck; onBack: () => void }) {
  const [queue, setQueue] = useState<DueItem[]>([]);
  const [index, setIndex] = useState(0);
  const [revealed, setRevealed] = useState(false);
  // True once a fetch has come back shorter than requested — stops
  // auto-prefetching until either more becomes due or the user asks for more.
  const [noMoreAvailable, setNoMoreAvailable] = useState(false);
  const [bonusNewAvailable, setBonusNewAvailable] = useState(0);
  const [loading, setLoading] = useState(false);
  const [loadError, setLoadError] = useState(false);
  // Guards against overlapping fetches — the auto-refill effect firing again
  // before a previous fetch (auto or manual) has settled.
  const inFlightRef = useRef(false);

  const loadMore = useCallback(
    async (bypassNewCardCap: boolean) => {
      if (inFlightRef.current) return;
      inFlightRef.current = true;
      setLoading(true);
      setLoadError(false);
      try {
        const batch = await api.study.studyBatch(deck.id, BATCH_SIZE, bypassNewCardCap);
        setBonusNewAvailable(batch.bonus_new_available);
        setQueue((q) => [...q, ...batch.items]);
        // A batch shorter than requested means the backend already exhausted
        // everything due/introducible right now — no point asking again
        // until more becomes due or the user asks for bonus cards.
        setNoMoreAvailable(batch.items.length < BATCH_SIZE);
      } catch {
        setLoadError(true);
      } finally {
        // Always runs, success or failure, so "loading" can never get stuck.
        setLoading(false);
        inFlightRef.current = false;
      }
    },
    [deck.id],
  );

  // Keeps the buffer topped up as the user studies through it. Not an
  // infinite-scroll "load more" — this is what makes the queue feel
  // never-ending: once nothing is due and today's new-card cap is reached,
  // it simply stops until the user asks for more.
  useEffect(() => {
    if (noMoreAvailable) return;
    if (queue.length - index > PREFETCH_THRESHOLD) return;
    loadMore(false);
  }, [index, queue.length, noMoreAvailable, loadMore]);

  const submitReview = useMutation({
    mutationFn: (rating: Rating) => {
      const item = queue[index];
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

  const initialLoading = queue.length === 0 && !noMoreAvailable && !loadError;
  if (initialLoading) {
    return (
      <div className="mx-auto flex max-w-xl flex-col gap-4 p-6">
        {header}
        <p className="text-muted-foreground">Loading…</p>
      </div>
    );
  }

  const item = queue[index];
  const exhausted = !item && (noMoreAvailable || loadError);
  const waitingForNextBatch = !item && !noMoreAvailable && !loadError;

  return (
    <div className="mx-auto flex max-w-xl flex-col gap-4 p-6">
      {header}

      {exhausted && (
        <div className="flex flex-col gap-3">
          <p className="text-muted-foreground">
            {loadError
              ? "Couldn't load more cards."
              : bonusNewAvailable > 0
                ? "Daily limit reached — nothing to review right now."
                : "Nothing due right now. Nice work."}
          </p>
          {(loadError || bonusNewAvailable > 0) && (
            <Button
              variant="outline"
              disabled={loading}
              onClick={() => loadMore(loadError ? bonusNewAvailable > 0 : true)}
            >
              {loadError ? "Try again" : "Continue"}
            </Button>
          )}
        </div>
      )}

      {waitingForNextBatch && <p className="text-muted-foreground">Loading…</p>}

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

      {index > 0 && <p className="text-sm text-muted-foreground">{index} studied this session</p>}
    </div>
  );
}
