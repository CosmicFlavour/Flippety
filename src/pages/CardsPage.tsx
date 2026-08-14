import { useEffect, useMemo, useRef, useState } from "react";
import { Pencil, Search, Trash2, X } from "lucide-react";
import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";
import { api } from "@/lib/api";
import type { Card as CardType, Deck } from "@/types/models";
import { useDebouncedValue } from "@/hooks/useDebouncedValue";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
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

const PAGE_SIZE = 40;
const ROW_ESTIMATE_PX = 96;

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
  const [searchInput, setSearchInput] = useState("");
  const search = useDebouncedValue(searchInput, 300).trim();
  const [selectedTags, setSelectedTags] = useState<string[]>([]);
  const hasFilters = search.length > 0 || selectedTags.length > 0;

  const tagsQuery = useQuery({
    queryKey: ["cardTags", deck.id],
    queryFn: () => api.cards.listDeckTags(deck.id),
  });

  const cardsQuery = useInfiniteQuery({
    queryKey: ["cards", deck.id, search, selectedTags],
    queryFn: ({ pageParam }) =>
      api.cards.listByDeck(deck.id, {
        search: search || undefined,
        tags: selectedTags.length > 0 ? selectedTags : undefined,
        limit: PAGE_SIZE,
        offset: pageParam,
      }),
    initialPageParam: 0,
    getNextPageParam: (lastPage, allPages) => {
      const loaded = allPages.reduce((sum, page) => sum + page.items.length, 0);
      return loaded < lastPage.total ? loaded : undefined;
    },
  });

  const cards = useMemo(
    () => cardsQuery.data?.pages.flatMap((page) => page.items) ?? [],
    [cardsQuery.data],
  );
  const total = cardsQuery.data?.pages[0]?.total ?? 0;

  const parentRef = useRef<HTMLDivElement>(null);
  const rowCount = cardsQuery.hasNextPage ? cards.length + 1 : cards.length;
  const virtualizer = useVirtualizer({
    count: rowCount,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_ESTIMATE_PX,
    overscan: 8,
  });
  const virtualItems = virtualizer.getVirtualItems();

  useEffect(() => {
    const lastItem = virtualItems[virtualItems.length - 1];
    if (!lastItem) return;
    if (
      lastItem.index >= cards.length - 1 &&
      cardsQuery.hasNextPage &&
      !cardsQuery.isFetchingNextPage
    ) {
      cardsQuery.fetchNextPage();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [virtualItems, cards.length, cardsQuery.hasNextPage, cardsQuery.isFetchingNextPage]);

  const invalidateCards = () => {
    queryClient.invalidateQueries({ queryKey: ["cards", deck.id] });
    queryClient.invalidateQueries({ queryKey: ["cardTags", deck.id] });
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

  const toggleTag = (tag: string) => {
    setSelectedTags((prev) => (prev.includes(tag) ? prev.filter((t) => t !== tag) : [...prev, tag]));
  };

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

      <div className="flex flex-col gap-2">
        <div className="relative">
          <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            value={searchInput}
            onChange={(e) => setSearchInput(e.target.value)}
            placeholder="Search by title…"
            className="pl-9"
          />
        </div>

        {tagsQuery.data && tagsQuery.data.length > 0 && (
          <div className="flex flex-wrap items-center gap-1.5">
            {tagsQuery.data.map((tag) => (
              <Badge
                key={tag}
                render={<button type="button" />}
                variant={selectedTags.includes(tag) ? "default" : "outline"}
                aria-pressed={selectedTags.includes(tag)}
                onClick={() => toggleTag(tag)}
                className="cursor-pointer select-none"
              >
                {tag}
              </Badge>
            ))}
            {hasFilters && (
              <Badge
                render={<button type="button" />}
                variant="ghost"
                onClick={() => {
                  setSearchInput("");
                  setSelectedTags([]);
                }}
                className="cursor-pointer gap-0.5 select-none"
              >
                <X className="size-3" /> Clear filters
              </Badge>
            )}
          </div>
        )}
      </div>

      {cardsQuery.isLoading && <p className="text-muted-foreground">Loading…</p>}
      {!cardsQuery.isLoading && total === 0 && (
        <p className="text-muted-foreground">
          {hasFilters ? "No cards match your search." : "No cards yet. Add one to get started."}
        </p>
      )}
      {!cardsQuery.isLoading && total > 0 && (
        <p className="text-xs text-muted-foreground">
          Showing {cards.length} of {total} card{total === 1 ? "" : "s"}
        </p>
      )}

      {total > 0 && (
        <div ref={parentRef} className="h-[65vh] overflow-y-auto">
          <div className="relative w-full" style={{ height: virtualizer.getTotalSize() }}>
            {virtualItems.map((virtualRow) => {
              const isLoaderRow = virtualRow.index > cards.length - 1;
              const card = cards[virtualRow.index];

              return (
                <div
                  key={virtualRow.key}
                  data-index={virtualRow.index}
                  ref={virtualizer.measureElement}
                  className="absolute top-0 left-0 w-full pb-2"
                  style={{ transform: `translateY(${virtualRow.start}px)` }}
                >
                  {isLoaderRow ? (
                    <p className="py-4 text-center text-sm text-muted-foreground">Loading more…</p>
                  ) : (
                    <CardRow
                      card={card}
                      onUpdate={(values) => updateCard.mutateAsync({ id: card.id, ...values })}
                      onDelete={() => deleteCard.mutate(card.id)}
                      onResetProgress={() => resetProgress.mutateAsync(card.id)}
                    />
                  )}
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}

function CardRow({
  card,
  onUpdate,
  onDelete,
  onResetProgress,
}: {
  card: CardType;
  onUpdate: (values: CardFormValues) => Promise<CardType>;
  onDelete: () => void;
  onResetProgress: () => Promise<unknown>;
}) {
  return (
    <UiCard>
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
            onSubmit={onUpdate}
            onResetProgress={onResetProgress}
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
                <AlertDialogAction variant="destructive" onClick={onDelete}>
                  Delete
                </AlertDialogAction>
              </AlertDialogFooter>
            </AlertDialogContent>
          </AlertDialog>
        </CardAction>
      </CardHeader>
    </UiCard>
  );
}
