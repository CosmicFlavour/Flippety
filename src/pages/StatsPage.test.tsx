import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { StatsPage } from "./StatsPage";
import { api } from "@/lib/api";
import type { Deck, DeckStats } from "@/types/models";

vi.mock("@/lib/api", () => ({
  api: {
    stats: {
      getDeckStats: vi.fn(),
    },
  },
}));

const DECK: Deck = {
  id: "deck-1",
  name: "Chinese HSK1",
  description: null,
  intro_direction: "1->2",
  created_at: "",
  updated_at: "",
};

function stats(overrides: Partial<DeckStats> = {}): DeckStats {
  return {
    total_cards: 20,
    introduced_cards: 8,
    ratings: { again: 3, hard: 2, good: 7, easy: 4 },
    ...overrides,
  };
}

function renderStatsPage(onBack = vi.fn(), deck: Deck = DECK) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={queryClient}>
      <StatsPage deck={deck} onBack={onBack} />
    </QueryClientProvider>,
  );
  return { onBack };
}

describe("StatsPage", () => {
  it("shows the deck name and a loading state before stats arrive", async () => {
    vi.mocked(api.stats.getDeckStats).mockReturnValue(new Promise(() => {}));
    renderStatsPage();

    expect(screen.getByText("Chinese HSK1 — Stats")).toBeInTheDocument();
    expect(screen.getByText("Loading…")).toBeInTheDocument();
  });

  it("shows the introduced/total card counts once stats load", async () => {
    vi.mocked(api.stats.getDeckStats).mockResolvedValue(
      stats({ total_cards: 20, introduced_cards: 8 }),
    );
    renderStatsPage();

    expect(await screen.findByText("8")).toBeInTheDocument();
    expect(screen.getByText("of 20")).toBeInTheDocument();
  });

  it("shows every rating bucket's count in the legend", async () => {
    vi.mocked(api.stats.getDeckStats).mockResolvedValue(
      stats({ ratings: { again: 3, hard: 2, good: 7, easy: 4 } }),
    );
    renderStatsPage();

    await screen.findByText("Again");
    expect(screen.getByText("3")).toBeInTheDocument();
    expect(screen.getByText("Hard")).toBeInTheDocument();
    expect(screen.getByText("2")).toBeInTheDocument();
    expect(screen.getByText("Good")).toBeInTheDocument();
    expect(screen.getByText("7")).toBeInTheDocument();
    expect(screen.getByText("Easy")).toBeInTheDocument();
    expect(screen.getByText("4")).toBeInTheDocument();
  });

  it("calls onBack when the back button is clicked", async () => {
    const { onBack } = renderStatsPage();
    vi.mocked(api.stats.getDeckStats).mockResolvedValue(stats());

    screen.getByRole("button", { name: "Back" }).click();

    expect(onBack).toHaveBeenCalled();
  });

  it("shows a nothing-reviewed message instead of an empty chart when no items have a rating", async () => {
    vi.mocked(api.stats.getDeckStats).mockResolvedValue(
      stats({
        total_cards: 0,
        introduced_cards: 0,
        ratings: { again: 0, hard: 0, good: 0, easy: 0 },
      }),
    );
    renderStatsPage();

    expect(await screen.findByText("of 0")).toBeInTheDocument();
    expect(screen.getByText("Nothing reviewed yet.")).toBeInTheDocument();
  });

  it("omits a rating bucket from the legend entirely once it has zero items", async () => {
    vi.mocked(api.stats.getDeckStats).mockResolvedValue(
      stats({ ratings: { again: 0, hard: 0, good: 5, easy: 2 } }),
    );
    renderStatsPage();

    await screen.findByText("Good");
    expect(screen.getByText("Easy")).toBeInTheDocument();
    expect(screen.queryByText("Again")).not.toBeInTheDocument();
    expect(screen.queryByText("Hard")).not.toBeInTheDocument();
  });
});
