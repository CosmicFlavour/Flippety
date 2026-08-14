import { Pencil, Trash2 } from "lucide-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "@/lib/api";
import type { Deck } from "@/types/models";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Card as UiCard,
  CardHeader,
  CardTitle,
  CardDescription,
  CardAction,
} from "@/components/ui/card";
import {
  AlertDialog,
  AlertDialogTrigger,
  AlertDialogContent,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogAction,
  AlertDialogCancel,
} from "@/components/ui/alert-dialog";
import { CardFormDialog, type CardFormValues } from "@/components/CardFormDialog";

export function CardsPage({
  deck,
  onBack,
  onStudy,
}: {
  deck: Deck;
  onBack: () => void;
  onStudy: () => void;
}) {
  const queryClient = useQueryClient();
  const cardsQuery = useQuery({
    queryKey: ["cards", deck.id],
    queryFn: () => api.cards.listByDeck(deck.id),
  });

  const invalidateCards = () => {
    queryClient.invalidateQueries({ queryKey: ["cards", deck.id] });
    queryClient.invalidateQueries({ queryKey: ["due", deck.id] });
  };

  const createCard = useMutation({
    mutationFn: (values: CardFormValues) => api.cards.create({ deck_id: deck.id, ...values }),
    onSuccess: invalidateCards,
  });

  const updateCard = useMutation({
    mutationFn: (values: CardFormValues & { id: string }) => api.cards.update(values),
    onSuccess: invalidateCards,
  });

  const deleteCard = useMutation({
    mutationFn: (id: string) => api.cards.delete(id),
    onSuccess: invalidateCards,
  });

  const resetProgress = useMutation({
    mutationFn: (id: string) => api.cards.resetProgress(id),
    onSuccess: invalidateCards,
  });

  return (
    <div className="mx-auto flex max-w-2xl flex-col gap-4 p-6">
      <div className="flex items-center justify-between">
        <div>
          <Button variant="ghost" size="sm" onClick={onBack}>
            ← Decks
          </Button>
          <h1 className="text-xl font-semibold">{deck.name}</h1>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" onClick={onStudy}>
            Study
          </Button>
          <CardFormDialog
            trigger={<Button>New card</Button>}
            title="New card"
            submitLabel="Create"
            onSubmit={(values) => createCard.mutateAsync(values)}
          />
        </div>
      </div>

      {cardsQuery.isLoading && <p className="text-muted-foreground">Loading…</p>}
      {cardsQuery.data?.items.length === 0 && (
        <p className="text-muted-foreground">No cards yet. Add one to get started.</p>
      )}

      <div className="flex flex-col gap-2">
        {cardsQuery.data?.items.map((card) => (
          <UiCard key={card.id}>
            <CardHeader>
              <CardTitle>
                {card.face_1} ↔ {card.face_2}
              </CardTitle>
              {card.full.subtitle && <CardDescription>{card.full.subtitle}</CardDescription>}
              {card.tags.length > 0 && (
                <div className="flex gap-1.5 pt-1">
                  {card.tags.map((tag) => (
                    <Badge key={tag} variant="secondary">
                      {tag}
                    </Badge>
                  ))}
                </div>
              )}
              <CardAction className="flex gap-1">
                <CardFormDialog
                  trigger={
                    <Button variant="ghost" size="icon-sm" aria-label="Edit card">
                      <Pencil className="size-3.5" />
                    </Button>
                  }
                  title="Edit card"
                  submitLabel="Save"
                  initialValues={{
                    face_1: card.face_1,
                    face_2: card.face_2,
                    full: card.full,
                    tags: card.tags,
                    directions: card.directions,
                    level: card.level,
                  }}
                  onSubmit={(values) => updateCard.mutateAsync({ id: card.id, ...values })}
                  onResetProgress={() => resetProgress.mutateAsync(card.id)}
                />
                <AlertDialog>
                  <AlertDialogTrigger
                    render={
                      <Button variant="ghost" size="icon-sm" aria-label="Delete card">
                        <Trash2 className="size-3.5" />
                      </Button>
                    }
                  />
                  <AlertDialogContent>
                    <AlertDialogHeader>
                      <AlertDialogTitle>Delete this card?</AlertDialogTitle>
                      <AlertDialogDescription>
                        This also deletes its study history in both directions. This can't be undone.
                      </AlertDialogDescription>
                    </AlertDialogHeader>
                    <AlertDialogFooter>
                      <AlertDialogCancel>Cancel</AlertDialogCancel>
                      <AlertDialogAction variant="destructive" onClick={() => deleteCard.mutate(card.id)}>
                        Delete
                      </AlertDialogAction>
                    </AlertDialogFooter>
                  </AlertDialogContent>
                </AlertDialog>
              </CardAction>
            </CardHeader>
          </UiCard>
        ))}
      </div>
    </div>
  );
}
