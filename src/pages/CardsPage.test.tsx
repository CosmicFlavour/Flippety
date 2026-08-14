import { describe, it, expect, vi, beforeAll, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { CardsPage } from "./CardsPage";
import { api } from "@/lib/api";
import type { Card, CardPage as CardPageType, Deck } from "@/types/models";

vi.mock("@/lib/api", () => ({
  api: {
    cards: {
      listByDeck: vi.fn(),
      listDeckTags: vi.fn(),
      create: vi.fn(),
      update: vi.fn(),
      delete: vi.fn(),
      resetProgress: vi.fn(),
    },
  },
}));

const DECK: Deck = {
  id: "deck-1",
  name: "Chinese HSK1",
  description: null,
  new_cards_per_day: null,
  created_at: "",
  updated_at: "",
};

function card(overrides: Partial<Card> = {}): Card {
  return {
    id: "card-1",
    deck_id: DECK.id,
    face_1: "dog",
    face_2: "狗",
    full: { title: "Dog", subtitle: "gǒu", body: "Domestic dog.", foot: "" },
    tags: [],
    directions: ["1->2"],
    level: 1,
    created_at: "",
    updated_at: "",
    ...overrides,
  };
}

function page(items: Card[], total = items.length): CardPageType {
  return { items, total };
}

function renderCardsPage() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={queryClient}>
      <CardsPage deck={DECK} onBack={vi.fn()} onStudy={vi.fn()} />
    </QueryClientProvider>,
  );
}

describe("CardsPage", () => {
  // jsdom never lays anything out, so the virtualizer's offsetHeight-based
  // viewport measurement is always 0 and it renders nothing. Fake a real
  // viewport so virtualized rows actually mount in tests.
  beforeAll(() => {
    Object.defineProperty(HTMLElement.prototype, "offsetHeight", {
      configurable: true,
      value: 800,
    });
    Object.defineProperty(HTMLElement.prototype, "offsetWidth", {
      configurable: true,
      value: 800,
    });
  });

  beforeEach(() => {
    vi.mocked(api.cards.listByDeck).mockReset();
    vi.mocked(api.cards.listDeckTags).mockReset();
    vi.mocked(api.cards.listDeckTags).mockResolvedValue([]);
  });

  it("shows the empty state when the deck has no cards", async () => {
    vi.mocked(api.cards.listByDeck).mockResolvedValue(page([], 0));
    renderCardsPage();

    expect(await screen.findByText("No cards yet. Add one to get started.")).toBeInTheDocument();
  });

  it("renders cards from the first page", async () => {
    vi.mocked(api.cards.listByDeck).mockResolvedValue(page([card(), card({ id: "card-2", face_1: "cat" })]));
    renderCardsPage();

    expect(await screen.findByText("dog ↔ 狗")).toBeInTheDocument();
    expect(await screen.findByText("Showing 2 of 2 cards")).toBeInTheDocument();
  });

  it("debounces the search box before querying by title", async () => {
    const user = userEvent.setup();
    vi.mocked(api.cards.listByDeck).mockResolvedValue(page([card()]));
    renderCardsPage();
    await screen.findByText("dog ↔ 狗");
    vi.mocked(api.cards.listByDeck).mockClear();

    await user.type(screen.getByPlaceholderText("Search by title…"), "chi");

    await waitFor(() =>
      expect(api.cards.listByDeck).toHaveBeenCalledWith(
        DECK.id,
        expect.objectContaining({ search: "chi" }),
      ),
    );
  });

  it("shows the tag chips and filters by the selected tag", async () => {
    const user = userEvent.setup();
    vi.mocked(api.cards.listDeckTags).mockResolvedValue(["animals", "hsk1"]);
    vi.mocked(api.cards.listByDeck).mockResolvedValue(page([card({ tags: ["animals"] })]));
    renderCardsPage();
    await screen.findByText("dog ↔ 狗");
    vi.mocked(api.cards.listByDeck).mockClear();

    await user.click(screen.getByRole("button", { name: "animals" }));

    await waitFor(() =>
      expect(api.cards.listByDeck).toHaveBeenCalledWith(
        DECK.id,
        expect.objectContaining({ tags: ["animals"] }),
      ),
    );
  });

  it("shows a filtered empty state distinct from a truly empty deck", async () => {
    const user = userEvent.setup();
    vi.mocked(api.cards.listByDeck).mockResolvedValue(page([card()]));
    renderCardsPage();
    await screen.findByText("dog ↔ 狗");

    vi.mocked(api.cards.listByDeck).mockResolvedValue(page([], 0));
    await user.type(screen.getByPlaceholderText("Search by title…"), "nonexistent");

    expect(await screen.findByText("No cards match your search.")).toBeInTheDocument();
  });
});
