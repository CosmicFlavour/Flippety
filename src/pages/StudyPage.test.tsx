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
      dueQueue: vi.fn(),
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
    vi.mocked(api.study.submitReview).mockReset();
  });

  it("shows an empty state when nothing is due", async () => {
    vi.mocked(api.study.dueQueue).mockResolvedValue([]);
    renderStudyPage();

    expect(await screen.findByText("Nothing due right now. Nice work.")).toBeInTheDocument();
  });

  it("shows only the prompt, never the solution, before reveal", async () => {
    vi.mocked(api.study.dueQueue).mockResolvedValue([item()]);
    renderStudyPage();

    expect(await screen.findByText("dog")).toBeInTheDocument();
    expect(screen.queryByText("狗")).not.toBeInTheDocument();
    expect(screen.queryByText("Domestic dog.")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Again" })).not.toBeInTheDocument();
  });

  it("reveals the full solution and the rating buttons after Reveal is clicked", async () => {
    const user = userEvent.setup();
    vi.mocked(api.study.dueQueue).mockResolvedValue([item()]);
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
    vi.mocked(api.study.dueQueue).mockResolvedValue([
      item({ card_id: "card-42", direction: "2->1" }),
    ]);
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
    vi.mocked(api.study.dueQueue).mockResolvedValue([
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
    vi.mocked(api.study.dueQueue).mockResolvedValue([item()]);
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
    vi.mocked(api.study.dueQueue).mockResolvedValue([item(), item({ card_id: "card-2" })]);
    renderStudyPage();

    expect(await screen.findByText("1 / 2")).toBeInTheDocument();
  });
});
