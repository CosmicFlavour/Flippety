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
      studyBatch: vi.fn(),
      submitReview: vi.fn(),
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

function item(overrides: Partial<DueItem> = {}): DueItem {
  return {
    card_id: "card-1",
    direction: "1->2",
    prompt: "dog",
    full: { title: "狗", subtitle: "gǒu", body: "Domestic dog.", foot: "这是我的狗。" },
    ...overrides,
  };
}

/** Wires studyBatch to always resolve with the given items/bonus count, regardless of args. */
function mockStudyBatch(items: DueItem[], bonusNewAvailable = 0) {
  vi.mocked(api.study.studyBatch).mockResolvedValue({
    items,
    bonus_new_available: bonusNewAvailable,
  });
}

function renderStudyPage(onBack = vi.fn(), deck: Deck = DECK) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={queryClient}>
      <StudyPage deck={deck} onBack={onBack} />
    </QueryClientProvider>,
  );
}

describe("StudyPage", () => {
  beforeEach(() => {
    vi.mocked(api.study.studyBatch).mockReset();
    vi.mocked(api.study.submitReview).mockReset();
  });

  it("shows an empty state when nothing is due", async () => {
    mockStudyBatch([]);
    renderStudyPage();

    expect(await screen.findByText("Nothing due right now. Nice work.")).toBeInTheDocument();
  });

  it("shows only the prompt, never the solution, before reveal", async () => {
    mockStudyBatch([item()]);
    renderStudyPage();

    expect(await screen.findByText("dog")).toBeInTheDocument();
    expect(screen.queryByText("狗")).not.toBeInTheDocument();
    expect(screen.queryByText("Domestic dog.")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Again" })).not.toBeInTheDocument();
  });

  it("does not keep refetching once a batch smaller than the requested size signals nothing more is available right now", async () => {
    mockStudyBatch([item()]);
    renderStudyPage();
    await screen.findByText("dog");

    // Give any runaway refetch loop a chance to happen before asserting.
    await waitFor(() => expect(api.study.studyBatch).toHaveBeenCalledTimes(1));
  });

  it("reveals the full solution and the rating buttons after Reveal is clicked", async () => {
    const user = userEvent.setup();
    mockStudyBatch([item()]);
    renderStudyPage();
    await screen.findByText("dog");

    await user.click(screen.getByRole("button", { name: "Reveal" }));

    expect(screen.getByText("狗")).toBeInTheDocument();
    expect(screen.getByText("gǒu")).toBeInTheDocument();
    expect(screen.getByText("Domestic dog.")).toBeInTheDocument();
    expect(screen.getByText("这是我的狗。")).toBeInTheDocument();
    for (const rating of ["Again", "Hard", "Good", "Easy"]) {
      expect(screen.getByRole("button", { name: rating })).toBeInTheDocument();
    }
    expect(screen.queryByRole("button", { name: "Reveal" })).not.toBeInTheDocument();
  });

  it("submits the rating with the card's id and direction", async () => {
    const user = userEvent.setup();
    mockStudyBatch([item({ card_id: "card-42", direction: "2->1" })]);
    vi.mocked(api.study.submitReview).mockResolvedValue(undefined);
    renderStudyPage();
    await screen.findByText("dog");
    await user.click(screen.getByRole("button", { name: "Reveal" }));

    await user.click(screen.getByRole("button", { name: "Good" }));

    expect(api.study.submitReview).toHaveBeenCalledWith({
      card_id: "card-42",
      direction: "2->1",
      rating: "Good",
    });
  });

  it("advances to the next card immediately, without waiting for the rating to be saved", async () => {
    const user = userEvent.setup();
    mockStudyBatch([
      item({ card_id: "card-1", prompt: "dog" }),
      item({ card_id: "card-2", prompt: "cat", full: { title: "猫", subtitle: "", body: "", foot: "" } }),
    ]);
    let resolveSubmit: () => void = () => {};
    vi.mocked(api.study.submitReview).mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveSubmit = () => resolve(undefined);
        }),
    );
    renderStudyPage();
    await screen.findByText("dog");
    await user.click(screen.getByRole("button", { name: "Reveal" }));

    await user.click(screen.getByRole("button", { name: "Good" }));

    // The save hasn't resolved yet, but the next card should already be showing.
    expect(await screen.findByText("cat")).toBeInTheDocument();
    resolveSubmit();
  });

  it("shows an error if saving a rating fails, without blocking navigation to the next card", async () => {
    const user = userEvent.setup();
    mockStudyBatch([
      item({ card_id: "card-1", prompt: "dog" }),
      item({ card_id: "card-2", prompt: "cat", full: { title: "猫", subtitle: "", body: "", foot: "" } }),
    ]);
    vi.mocked(api.study.submitReview).mockRejectedValue(new Error("network error"));
    renderStudyPage();
    await screen.findByText("dog");
    await user.click(screen.getByRole("button", { name: "Reveal" }));

    await user.click(screen.getByRole("button", { name: "Good" }));

    expect(await screen.findByText("cat")).toBeInTheDocument();
    expect(
      await screen.findByText("Couldn't save your last rating — it may need to be repeated."),
    ).toBeInTheDocument();
  });

  it("advances to the next card and hides the solution again after rating", async () => {
    const user = userEvent.setup();
    mockStudyBatch([
      item({ card_id: "card-1", prompt: "dog" }),
      item({ card_id: "card-2", prompt: "cat", full: { title: "猫", subtitle: "", body: "", foot: "" } }),
    ]);
    vi.mocked(api.study.submitReview).mockResolvedValue(undefined);
    renderStudyPage();
    await screen.findByText("dog");
    await user.click(screen.getByRole("button", { name: "Reveal" }));
    await user.click(screen.getByRole("button", { name: "Good" }));

    expect(await screen.findByText("cat")).toBeInTheDocument();
    expect(screen.queryByText("猫")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Reveal" })).toBeInTheDocument();
  });

  it("shows the empty state once the last card has been rated", async () => {
    const user = userEvent.setup();
    mockStudyBatch([item()]);
    vi.mocked(api.study.submitReview).mockResolvedValue(undefined);
    renderStudyPage();
    await screen.findByText("dog");
    await user.click(screen.getByRole("button", { name: "Reveal" }));

    await user.click(screen.getByRole("button", { name: "Easy" }));

    await waitFor(() =>
      expect(screen.getByText("Nothing due right now. Nice work.")).toBeInTheDocument(),
    );
  });

  it("shows a running studied count instead of a fixed total", async () => {
    const user = userEvent.setup();
    mockStudyBatch([item(), item({ card_id: "card-2" })]);
    vi.mocked(api.study.submitReview).mockResolvedValue(undefined);
    renderStudyPage();
    await screen.findByText("dog");
    expect(screen.queryByText(/studied this session/)).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Reveal" }));
    await user.click(screen.getByRole("button", { name: "Good" }));

    expect(await screen.findByText("1 studied this session")).toBeInTheDocument();
  });

  it("preserves line breaks in the body and foot text", async () => {
    const user = userEvent.setup();
    mockStudyBatch([
      item({
        full: {
          title: "狗",
          subtitle: "gǒu",
          body: "Domestic dog.\nCommon household pet.",
          foot: "这是我的狗。\n我很喜欢它。",
        },
      }),
    ]);
    renderStudyPage();
    await screen.findByText("dog");

    await user.click(screen.getByRole("button", { name: "Reveal" }));

    const body = screen.getByText("Domestic dog.", { exact: false });
    expect(body).toHaveClass("whitespace-pre-line");
    expect(body).toHaveTextContent("Domestic dog.\nCommon household pet.", {
      normalizeWhitespace: false,
    });
    const foot = screen.getByText("这是我的狗。", { exact: false });
    expect(foot).toHaveClass("whitespace-pre-line");
    expect(foot).toHaveTextContent("这是我的狗。\n我很喜欢它。", { normalizeWhitespace: false });
  });

  it("offers to continue once the daily cap is reached", async () => {
    mockStudyBatch([], 3);
    renderStudyPage();

    expect(
      await screen.findByText("Daily limit reached — nothing to review right now."),
    ).toBeInTheDocument();
    expect(screen.getByText("Continue")).toBeInTheDocument();
  });

  it("does not offer a continue button when there truly is nothing left", async () => {
    mockStudyBatch([], 0);
    renderStudyPage();

    expect(await screen.findByText("Nothing due right now. Nice work.")).toBeInTheDocument();
    expect(screen.queryByText("Continue")).not.toBeInTheDocument();
  });

  it("resumes studying with bonus new cards once Continue is chosen, bypassing the cap for exactly that batch", async () => {
    const user = userEvent.setup();
    const bonusItem = item({ card_id: "bonus-1", prompt: "bonus card" });
    vi.mocked(api.study.studyBatch).mockImplementation(async (_deckId, _limit, bypass) => {
      if (bypass) return { items: [bonusItem], bonus_new_available: 0 };
      return { items: [], bonus_new_available: 1 };
    });
    renderStudyPage();
    await screen.findByText("Continue");

    await user.click(screen.getByText("Continue"));

    expect(await screen.findByText("bonus card")).toBeInTheDocument();
    expect(screen.queryByText("Nothing due right now. Nice work.")).not.toBeInTheDocument();
    expect(api.study.studyBatch).toHaveBeenCalledWith(DECK.id, 10, true);
  });

  it("the Continue button stays clickable and independent of the background auto-refill", async () => {
    const user = userEvent.setup();
    const bonusItem = item({ card_id: "bonus-1", prompt: "bonus card" });
    vi.mocked(api.study.studyBatch).mockImplementation(async (_deckId, _limit, bypass) => {
      if (bypass) return { items: [bonusItem], bonus_new_available: 0 };
      return { items: [], bonus_new_available: 1 };
    });
    renderStudyPage();
    const button = await screen.findByText("Continue");

    expect(button.closest("button")).not.toBeDisabled();
    await user.click(button);

    expect(await screen.findByText("bonus card")).toBeInTheDocument();
  });

  it("shows an error message and a retry button if loading more cards fails, without getting stuck on Loading…", async () => {
    const user = userEvent.setup();
    vi.mocked(api.study.studyBatch).mockImplementation(async (_deckId, _limit, bypass) => {
      if (bypass) throw new Error("network error");
      return { items: [], bonus_new_available: 2 };
    });
    renderStudyPage();
    await screen.findByText("Continue");

    await user.click(screen.getByText("Continue"));

    expect(await screen.findByText("Couldn't load more cards.")).toBeInTheDocument();
    expect(screen.getByText("Try again")).toBeInTheDocument();
    expect(screen.queryByText("Loading…")).not.toBeInTheDocument();
  });

  it("retries with the same bypass mode after a failed continue", async () => {
    const user = userEvent.setup();
    const bonusItem = item({ card_id: "bonus-1", prompt: "bonus card" });
    let bypassAttempts = 0;
    vi.mocked(api.study.studyBatch).mockImplementation(async (_deckId, _limit, bypass) => {
      if (bypass) {
        bypassAttempts++;
        if (bypassAttempts === 1) throw new Error("network error");
        return { items: [bonusItem], bonus_new_available: 0 };
      }
      return { items: [], bonus_new_available: 1 };
    });
    renderStudyPage();
    await screen.findByText("Continue");
    await user.click(screen.getByText("Continue"));
    await screen.findByText("Try again");

    await user.click(screen.getByText("Try again"));

    expect(await screen.findByText("bonus card")).toBeInTheDocument();
  });

  it("automatically fetches another batch as the buffer runs low", async () => {
    const user = userEvent.setup();
    const firstBatch = Array.from({ length: 10 }, (_, i) =>
      item({ card_id: `card-${i}`, prompt: `p${i}` }),
    );
    const secondBatch = [item({ card_id: "card-10", prompt: "p10" })];
    vi.mocked(api.study.studyBatch)
      .mockResolvedValueOnce({ items: firstBatch, bonus_new_available: 0 })
      .mockResolvedValueOnce({ items: secondBatch, bonus_new_available: 0 });
    vi.mocked(api.study.submitReview).mockResolvedValue(undefined);
    renderStudyPage();
    await screen.findByText("p0");

    // Rate down to within the prefetch threshold (3) of the first batch's end.
    for (let i = 0; i < 7; i++) {
      await user.click(screen.getByRole("button", { name: "Reveal" }));
      await user.click(screen.getByRole("button", { name: "Good" }));
    }

    await waitFor(() => expect(api.study.studyBatch).toHaveBeenCalledTimes(2));
  });

  it("fetches fresh data on a later visit instead of reusing a stale snapshot", async () => {
    // Regression test: the app keeps one QueryClient for its whole lifetime
    // and mounts/unmounts StudyPage as you navigate to and from it, so a
    // query cached indefinitely would show the same items forever.
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    mockStudyBatch([item({ card_id: "card-1", prompt: "dog" })]);
    const { unmount } = render(
      <QueryClientProvider client={queryClient}>
        <StudyPage deck={DECK} onBack={vi.fn()} />
      </QueryClientProvider>,
    );
    await screen.findByText("dog");
    unmount();

    // Simulate having rated "dog" and a different card being due by the time
    // the user comes back.
    mockStudyBatch([item({ card_id: "card-2", prompt: "cat" })]);
    render(
      <QueryClientProvider client={queryClient}>
        <StudyPage deck={DECK} onBack={vi.fn()} />
      </QueryClientProvider>,
    );

    expect(await screen.findByText("cat")).toBeInTheDocument();
    expect(screen.queryByText("dog")).not.toBeInTheDocument();
  });
});
