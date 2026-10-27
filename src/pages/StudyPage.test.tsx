import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { StudyPage } from "./StudyPage";
import { api } from "@/lib/api";
import type { Deck, DueItem } from "@/types/models";

vi.mock("@/lib/api", () => ({
  api: {
    study: {
      nextItem: vi.fn(),
      submitReview: vi.fn(),
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

function item(overrides: Partial<DueItem> = {}): DueItem {
  return {
    card_id: "card-1",
    direction: "1->2",
    prompt: "dog",
    full: { title: "狗", subtitle: "gǒu", body: "Domestic dog.", foot: "" },
    ...overrides,
  };
}

/** Queues `items` to resolve one at a time (one per `nextItem` call), then
 * resolves `null` ("nothing available") for every call after. */
function queueItems(...items: (DueItem | null)[]) {
  const mock = vi.mocked(api.study.nextItem);
  for (const value of items) {
    mock.mockResolvedValueOnce(value);
  }
  mock.mockResolvedValue(null);
}

function renderStudyPage(onBack = vi.fn(), deck: Deck = DECK) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={queryClient}>
      <StudyPage deck={deck} onBack={onBack} />
    </QueryClientProvider>,
  );
  return { onBack };
}

describe("StudyPage", () => {
  beforeEach(() => {
    vi.mocked(api.study.nextItem).mockReset();
    vi.mocked(api.study.submitReview).mockReset();
  });

  it("renders the fetched item's prompt, then reveals the full solution and rating buttons", async () => {
    const user = userEvent.setup();
    queueItems(item({ prompt: "dog" }));
    renderStudyPage();

    expect(await screen.findByText("dog")).toBeInTheDocument();
    expect(screen.queryByText("狗")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Again" })).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Reveal" }));

    expect(screen.getByText("狗")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Again" })).toBeInTheDocument();
  });

  it("advances to the already-prefetched item when rating, without a visible loading gap", async () => {
    const user = userEvent.setup();
    const first = item({ card_id: "card-1", prompt: "dog" });
    const second = item({ card_id: "card-2", prompt: "cat" });
    queueItems(first, second);
    vi.mocked(api.study.submitReview).mockResolvedValue({ milestone: null });
    renderStudyPage();
    await screen.findByText("dog");
    // current + the one-ahead prefetch, both fired on mount.
    await waitFor(() => expect(api.study.nextItem).toHaveBeenCalledTimes(2));

    await user.click(screen.getByRole("button", { name: "Reveal" }));
    await user.click(screen.getByRole("button", { name: "Good" }));

    expect(await screen.findByText("cat")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Continue" })).not.toBeInTheDocument();
    // Promoting the buffered item plus refilling behind it: one more call.
    await waitFor(() => expect(api.study.nextItem).toHaveBeenCalledTimes(3));
  });

  it("shows the nothing-due message when nothing is available", async () => {
    queueItems(null);
    renderStudyPage();

    expect(await screen.findByText("Nothing due right now. Nice work.")).toBeInTheDocument();
  });

  it("shows an error state with a working retry when fetching rejects", async () => {
    const user = userEvent.setup();
    vi.mocked(api.study.nextItem).mockRejectedValueOnce(new Error("boom"));
    renderStudyPage();

    expect(await screen.findByText("Couldn't load more cards.")).toBeInTheDocument();
    expect(screen.queryByText("Loading…")).not.toBeInTheDocument();

    queueItems(item({ prompt: "dog" }));
    await user.click(screen.getByRole("button", { name: "Try again" }));

    expect(await screen.findByText("dog")).toBeInTheDocument();
  });

  it("shows the milestone interstitial before the next card, then advances on Continue", async () => {
    const user = userEvent.setup();
    const first = item({ card_id: "card-1", prompt: "dog" });
    const second = item({ card_id: "card-2", prompt: "cat" });
    queueItems(first, second);
    vi.mocked(api.study.submitReview).mockResolvedValue({ milestone: 5 });
    renderStudyPage();
    await screen.findByText("dog");

    await user.click(screen.getByRole("button", { name: "Reveal" }));
    await user.click(screen.getByRole("button", { name: "Good" }));

    expect(await screen.findByText("5 new cards today — Good job")).toBeInTheDocument();
    expect(screen.queryByText("cat")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Continue" }));

    expect(await screen.findByText("cat")).toBeInTheDocument();
  });

  it.each([
    [5, "Good job"],
    [10, "On a streak!"],
    [15, "Going strong"],
    [20, "Maybe take a break"],
    [25, "That's probably too much"],
    [30, "That's probably too much"],
  ])("maps a milestone of %i to %s", async (count, text) => {
    const user = userEvent.setup();
    queueItems(item());
    vi.mocked(api.study.submitReview).mockResolvedValue({ milestone: count });
    renderStudyPage();
    await screen.findByText("dog");

    await user.click(screen.getByRole("button", { name: "Reveal" }));
    await user.click(screen.getByRole("button", { name: "Good" }));

    expect(await screen.findByText(`${count} new cards today — ${text}`)).toBeInTheDocument();
  });

  it("shows a save-failed banner but still advances when submitReview rejects", async () => {
    const user = userEvent.setup();
    const first = item({ card_id: "card-1", prompt: "dog" });
    const second = item({ card_id: "card-2", prompt: "cat" });
    queueItems(first, second);
    vi.mocked(api.study.submitReview).mockRejectedValue(new Error("offline"));
    renderStudyPage();
    await screen.findByText("dog");

    await user.click(screen.getByRole("button", { name: "Reveal" }));
    await user.click(screen.getByRole("button", { name: "Good" }));

    expect(await screen.findByText("cat")).toBeInTheDocument();
    expect(
      screen.getByText("Couldn't save your last rating — it may need to be repeated."),
    ).toBeInTheDocument();
  });
});
