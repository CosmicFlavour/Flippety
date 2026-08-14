import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DeckSettingsDialog } from "./DeckSettingsDialog";
import { exportDeckToFile } from "@/lib/export-deck";
import type { Deck } from "@/types/models";

vi.mock("@/lib/export-deck", () => ({
  exportDeckToFile: vi.fn(),
}));

const DECK: Deck = {
  id: "deck-1",
  name: "Chinese HSK1",
  description: "Vocab deck",
  new_cards_per_day: 20,
  created_at: "",
  updated_at: "",
};

async function openDialog(deck: Deck = DECK) {
  const user = userEvent.setup();
  const onSave = vi.fn().mockResolvedValue(undefined);
  const onDelete = vi.fn().mockResolvedValue(undefined);
  render(
    <DeckSettingsDialog
      trigger={<button>Open settings</button>}
      deck={deck}
      onSave={onSave}
      onDelete={onDelete}
    />,
  );
  await user.click(screen.getByText("Open settings"));
  return { user, onSave, onDelete };
}

describe("DeckSettingsDialog", () => {
  beforeEach(() => {
    vi.mocked(exportDeckToFile).mockReset();
  });

  it("pre-fills the deck's name, description, and new cards per day", async () => {
    await openDialog();

    expect(screen.getByLabelText("Name")).toHaveValue("Chinese HSK1");
    expect(screen.getByLabelText("Description")).toHaveValue("Vocab deck");
    expect(screen.getByLabelText("New cards per day")).toHaveValue(20);
  });

  it("saves the edited values", async () => {
    const { user, onSave } = await openDialog();

    await user.clear(screen.getByLabelText("Name"));
    await user.type(screen.getByLabelText("Name"), "Renamed deck");
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(onSave).toHaveBeenCalledWith({
      name: "Renamed deck",
      description: "Vocab deck",
      new_cards_per_day: 20,
    });
  });

  it("exports the deck as JSON", async () => {
    vi.mocked(exportDeckToFile).mockResolvedValue(true);
    const { user } = await openDialog();

    await user.click(screen.getByRole("button", { name: "Export as JSON…" }));

    expect(exportDeckToFile).toHaveBeenCalledWith(DECK);
    expect(await screen.findByText("Exported ✓")).toBeInTheDocument();
  });

  it("does not delete the deck until the confirmation is accepted", async () => {
    const { user, onDelete } = await openDialog();

    await user.click(screen.getByRole("button", { name: "Delete deck" }));
    expect(await screen.findByText('Delete "Chinese HSK1"?')).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Cancel" }));

    expect(onDelete).not.toHaveBeenCalled();
  });

  it("deletes the deck once the confirmation is accepted", async () => {
    const { user, onDelete } = await openDialog();

    await user.click(screen.getByRole("button", { name: "Delete deck" }));
    await screen.findByText('Delete "Chinese HSK1"?');
    await user.click(screen.getByRole("button", { name: "Delete" }));

    expect(onDelete).toHaveBeenCalled();
  });
});
