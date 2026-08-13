import { useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { api } from "@/lib/api";
import type { Deck, Rating } from "@/types/models";
import { Button } from "@/components/ui/button";
import { Card as UiCard, CardHeader, CardTitle, CardContent } from "@/components/ui/card";

const RATINGS: Rating[] = ["Again", "Hard", "Good", "Easy"];

export function StudyPage({ deck, onBack }: { deck: Deck; onBack: () => void }) {
  const queueQuery = useQuery({
    queryKey: ["due", deck.id],
    queryFn: () => api.study.dueQueue(deck.id, 20),
  });

  const [index, setIndex] = useState(0);
  const [revealed, setRevealed] = useState(false);

  const submitReview = useMutation({
    mutationFn: (rating: Rating) => {
      const item = queueQuery.data![index];
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

  if (queueQuery.isLoading) {
    return (
      <div className="mx-auto max-w-xl p-6">
        <p className="text-muted-foreground">Loading…</p>
      </div>
    );
  }

  const queue = queueQuery.data ?? [];
  const item = queue[index];

  return (
    <div className="mx-auto flex max-w-xl flex-col gap-4 p-6">
      <div>
        <Button variant="ghost" size="sm" onClick={onBack}>
          ← {deck.name}
        </Button>
      </div>

      {!item && <p className="text-muted-foreground">Nothing due right now. Nice work.</p>}

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
              {item.full.body && <p>{item.full.body}</p>}
              {item.full.foot && <p className="text-muted-foreground">{item.full.foot}</p>}
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

      {queue.length > 0 && (
        <p className="text-sm text-muted-foreground">
          {Math.min(index + 1, queue.length)} / {queue.length}
        </p>
      )}
    </div>
  );
}
