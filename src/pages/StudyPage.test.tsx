import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { StudyPage } from "./StudyPage";
import { api } from "@/lib/api";
import type { Deck, DueItem, QueueRef } from "@/types/models";

vi.mock("@/lib/api", () => ({
  api: {
    study: {
      dueQueue: vi.fn(),
      bonusNewCards: vi.fn(),
      aheadReviews: vi.fn(),
      queueCards: vi.fn(),
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

function refOf(due: DueItem): QueueRef {
  return { card_id: due.card_id, direction: due.direction };
}

/** Wires dueQueue + queueCards so the given items come back as the main due-review queue. */
function mockDueItems(items: DueItem[]) {
  vi.mocked(api.study.dueQueue).mockResolvedValue({ due: items.map(refOf), new: [] });
  vi.mocked(api.study.queueCards).mockImplementation(async (refs: QueueRef[]) =>
    refs.map(
      (ref) => items.find((i) => i.card_id === ref.card_id && i.direction === ref.direction)!,
    ),
  );
}

function renderStudyPage(onBack = vi.fn()) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={queryClient}>
      <StudyPage deck={DECK} onBack={onBack} />
    </QueryClientProvider>,
  );
}

describe("StudyPage", () => {
  beforeEach(() => {
    vi.mocked(api.study.dueQueue).mockReset();
    vi.mocked(api.study.bonusNewCards).mockReset().mockResolvedValue([]);
    vi.mocked(api.study.aheadReviews).mockReset().mockResolvedValue([]);
    vi.mocked(api.study.queueCards).mockReset();
    vi.mocked(api.study.submitReview).mockReset();
  });

  it("shows an empty state when nothing is due", async () => {
    mockDueItems([]);
    renderStudyPage();

    expect(await screen.findByText("Nothing due right now. Nice work.")).toBeInTheDocument();
  });

  it("shows only the prompt, never the solution, before reveal", async () => {
    mockDueItems([item()]);
    renderStudyPage();

    expect(await screen.findByText("dog")).toBeInTheDocument();
    expect(screen.queryByText("狗")).not.toBeInTheDocument();
    expect(screen.queryByText("Domestic dog.")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Again" })).not.toBeInTheDocument();
  });

  it("reveals the full solution and the rating buttons after Reveal is clicked", async () => {
    const user = userEvent.setup();
    mockDueItems([item()]);
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
    mockDueItems([item({ card_id: "card-42", direction: "2->1" })]);
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

  it("advances to the next card and hides the solution again after rating", async () => {
    const user = userEvent.setup();
    mockDueItems([
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
    mockDueItems([item()]);
    vi.mocked(api.study.submitReview).mockResolvedValue(undefined);
    renderStudyPage();
    await screen.findByText("dog");
    await user.click(screen.getByRole("button", { name: "Reveal" }));

    await user.click(screen.getByRole("button", { name: "Easy" }));

    await waitFor(() =>
      expect(screen.getByText("Nothing due right now. Nice work.")).toBeInTheDocument(),
    );
  });

  it("shows a running progress count", async () => {
    mockDueItems([item(), item({ card_id: "card-2" })]);
    renderStudyPage();

    expect(await screen.findByText("1 / 2")).toBeInTheDocument();
  });

  it("preserves line breaks in the body and foot text", async () => {
    const user = userEvent.setup();
    mockDueItems([
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

  it("offers more new cards and studying ahead once the session is exhausted", async () => {
    mockDueItems([]);
    vi.mocked(api.study.bonusNewCards).mockResolvedValue([
      { card_id: "bonus-1", direction: "1->2" },
      { card_id: "bonus-2", direction: "1->2" },
    ]);
    vi.mocked(api.study.aheadReviews).mockResolvedValue([{ card_id: "ahead-1", direction: "1->2" }]);
    renderStudyPage();

    expect(await screen.findByText("Study 2 more new cards today")).toBeInTheDocument();
    expect(await screen.findByText("Study 1 card early (next 24h)")).toBeInTheDocument();
  });

  it("resumes studying with bonus new cards once that option is chosen", async () => {
    const user = userEvent.setup();
    mockDueItems([]);
    const bonusItem = item({ card_id: "bonus-1", prompt: "bonus card" });
    vi.mocked(api.study.bonusNewCards).mockResolvedValue([refOf(bonusItem)]);
    vi.mocked(api.study.queueCards).mockResolvedValue([bonusItem]);
    renderStudyPage();
    await screen.findByText("Study 1 more new card today");

    await user.click(screen.getByText("Study 1 more new card today"));

    expect(await screen.findByText("bonus card")).toBeInTheDocument();
    expect(screen.queryByText("Nothing due right now. Nice work.")).not.toBeInTheDocument();
  });
});
