import { Layers, Settings } from "lucide-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "@/lib/api";
import type { Deck } from "@/types/models";
import { Button } from "@/components/ui/button";
import {
  Card as UiCard,
  CardHeader,
  CardTitle,
  CardDescription,
  CardAction,
} from "@/components/ui/card";
import { DeckFormDialog, type DeckFormValues } from "@/components/DeckFormDialog";
import { DeckSettingsDialog } from "@/components/DeckSettingsDialog";
import { ImportDeckDialog } from "@/components/ImportDeckDialog";

export function DecksPage({
  onStudyDeck,
  onBrowseDeck,
}: {
  onStudyDeck: (deck: Deck) => void;
  onBrowseDeck: (deck: Deck) => void;
}) {
  const queryClient = useQueryClient();
  const decksQuery = useQuery({ queryKey: ["decks"], queryFn: api.decks.list });

  const invalidateDecks = () => queryClient.invalidateQueries({ queryKey: ["decks"] });

  const createDeck = useMutation({
    mutationFn: (values: DeckFormValues) => api.decks.create(values),
    onSuccess: invalidateDecks,
  });

  const renameDeck = useMutation({
    mutationFn: (values: DeckFormValues & { id: string }) => api.decks.rename(values),
    onSuccess: invalidateDecks,
  });

  const deleteDeck = useMutation({
    mutationFn: (id: string) => api.decks.delete(id),
    onSuccess: invalidateDecks,
  });

  return (
    <div className="mx-auto flex max-w-2xl flex-col gap-4 p-6">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">Decks</h1>
        <div className="flex gap-2">
          <ImportDeckDialog
            trigger={<Button variant="outline">Import deck</Button>}
            existingDecks={decksQuery.data ?? []}
            onImported={(deck) => {
              invalidateDecks();
              queryClient.invalidateQueries({ queryKey: ["cards", deck.id] });
            }}
          />
          <DeckFormDialog
            trigger={<Button>New deck</Button>}
            title="New deck"
            submitLabel="Create"
            onSubmit={(values) => createDeck.mutateAsync(values)}
          />
        </div>
      </div>

      {decksQuery.isLoading && <p className="text-muted-foreground">Loading…</p>}
      {decksQuery.data?.length === 0 && (
        <p className="text-muted-foreground">No decks yet. Create one to get started.</p>
      )}

      <div className="flex flex-col gap-2">
        {decksQuery.data?.map((deck) => (
          <UiCard
            key={deck.id}
            className="cursor-pointer transition-colors hover:bg-muted/50"
            onClick={() => onStudyDeck(deck)}
          >
            <CardHeader>
              <CardTitle>{deck.name}</CardTitle>
              {deck.description && <CardDescription>{deck.description}</CardDescription>}
              <CardAction className="flex gap-1" onClick={(e) => e.stopPropagation()}>
                <DeckSettingsDialog
                  trigger={
                    <Button variant="ghost" size="icon-sm" aria-label="Deck settings">
                      <Settings className="size-3.5" />
                    </Button>
                  }
                  deck={deck}
                  onSave={(values) => renameDeck.mutateAsync({ id: deck.id, ...values })}
                  onDelete={() => deleteDeck.mutateAsync(deck.id)}
                />
                <Button
                  variant="ghost"
                  size="icon-sm"
                  aria-label="Browse cards"
                  onClick={() => onBrowseDeck(deck)}
                >
                  <Layers className="size-3.5" />
                </Button>
              </CardAction>
            </CardHeader>
          </UiCard>
        ))}
      </div>
    </div>
  );
}
