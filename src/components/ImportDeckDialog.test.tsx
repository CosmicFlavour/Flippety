import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ImportDeckDialog } from "./ImportDeckDialog";
import { api } from "@/lib/api";
import { open } from "@tauri-apps/plugin-dialog";
import { readTextFile } from "@tauri-apps/plugin-fs";
import type { Deck } from "@/types/models";

vi.mock("@/lib/api", () => ({
  api: { importExport: { importDeck: vi.fn() } },
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-fs", () => ({
  readTextFile: vi.fn(),
}));

const EXISTING_DECK: Deck = {
  id: "deck-1",
  name: "Demo Deck",
  description: null,
  new_cards_per_day: null,
  created_at: "",
  updated_at: "",
};

const IMPORTED_DECK: Deck = {
  id: "deck-2",
  name: "Imported Deck",
  description: null,
  new_cards_per_day: null,
  created_at: "",
  updated_at: "",
};

async function openDialog(existingDecks: Deck[] = [EXISTING_DECK]) {
  const user = userEvent.setup();
  const onImported = vi.fn();
  render(
    <ImportDeckDialog
      trigger={<button>Import deck</button>}
      existingDecks={existingDecks}
      onImported={onImported}
    />,
  );
  await user.click(screen.getByText("Import deck"));
  return { user, onImported };
}

describe("ImportDeckDialog", () => {
  beforeEach(() => {
    vi.mocked(open).mockReset();
    vi.mocked(readTextFile).mockReset().mockResolvedValue("{}");
    vi.mocked(api.importExport.importDeck).mockReset();
  });

  it("disables Import until a file is chosen", async () => {
    const { user } = await openDialog();
    const importButton = screen.getByRole("button", { name: "Import" });
    expect(importButton).toBeDisabled();

    vi.mocked(open).mockResolvedValue("/home/user/deck.json");
    await user.click(screen.getByText("Choose file…"));

    expect(importButton).toBeEnabled();
  });

  it("shows just the file name once a file is chosen", async () => {
    const { user } = await openDialog();
    vi.mocked(open).mockResolvedValue("/home/user/decks/chinese.json");

    await user.click(screen.getByText("Choose file…"));

    expect(screen.getByText("chinese.json")).toBeInTheDocument();
  });

  it("leaves the file unset if the picker is cancelled", async () => {
    const { user } = await openDialog();
    vi.mocked(open).mockResolvedValue(null);

    await user.click(screen.getByText("Choose file…"));

    expect(screen.getByText("Choose file…")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Import" })).toBeDisabled();
  });

  it("lists existing decks as merge targets", async () => {
    await openDialog([EXISTING_DECK]);

    expect(screen.getByRole("option", { name: 'Merge into "Demo Deck"' })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "A new deck" })).toBeInTheDocument();
  });

  it("imports as a new deck by default", async () => {
    const { user, onImported } = await openDialog();
    vi.mocked(open).mockResolvedValue("/home/user/deck.json");
    vi.mocked(readTextFile).mockResolvedValue('{"deck":{"name":"Deck"},"cards":[]}');
    vi.mocked(api.importExport.importDeck).mockResolvedValue(IMPORTED_DECK);
    await user.click(screen.getByText("Choose file…"));

    await user.click(screen.getByRole("button", { name: "Import" }));

    // Reads via the fs plugin — required so this works with a `content://`
    // URI on Android, not just a real path — then hands the file's *content*
    // (not the path) to the backend.
    expect(readTextFile).toHaveBeenCalledWith("/home/user/deck.json");
    expect(api.importExport.importDeck).toHaveBeenCalledWith('{"deck":{"name":"Deck"},"cards":[]}', {
      mode: "new",
    });
    expect(onImported).toHaveBeenCalledWith(IMPORTED_DECK);
  });

  it("imports as a merge into the selected deck", async () => {
    const { user } = await openDialog();
    vi.mocked(open).mockResolvedValue("/home/user/deck.json");
    vi.mocked(readTextFile).mockResolvedValue('{"deck":{"name":"Deck"},"cards":[]}');
    vi.mocked(api.importExport.importDeck).mockResolvedValue(EXISTING_DECK);
    await user.click(screen.getByText("Choose file…"));
    await user.selectOptions(screen.getByLabelText("Import into"), "deck-1");

    await user.click(screen.getByRole("button", { name: "Import" }));

    expect(api.importExport.importDeck).toHaveBeenCalledWith('{"deck":{"name":"Deck"},"cards":[]}', {
      mode: "merge",
      deck_id: "deck-1",
    });
  });

  it("closes the dialog after a successful import", async () => {
    const { user } = await openDialog();
    vi.mocked(open).mockResolvedValue("/home/user/deck.json");
    vi.mocked(api.importExport.importDeck).mockResolvedValue(IMPORTED_DECK);
    await user.click(screen.getByText("Choose file…"));

    await user.click(screen.getByRole("button", { name: "Import" }));

    expect(screen.queryByText("chinese.json")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Import" })).not.toBeInTheDocument();
  });

  it("shows the backend's validation error and keeps the dialog open", async () => {
    const { user, onImported } = await openDialog();
    vi.mocked(open).mockResolvedValue("/home/user/deck.json");
    vi.mocked(api.importExport.importDeck).mockRejectedValue(
      "every card needs both face_1 and face_2",
    );
    await user.click(screen.getByText("Choose file…"));

    await user.click(screen.getByRole("button", { name: "Import" }));

    expect(await screen.findByText("every card needs both face_1 and face_2")).toBeInTheDocument();
    expect(onImported).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Import" })).toBeInTheDocument();
  });

  it("falls back to a generic message for a non-string rejection", async () => {
    const { user } = await openDialog();
    vi.mocked(open).mockResolvedValue("/home/user/deck.json");
    vi.mocked(api.importExport.importDeck).mockRejectedValue(new Error("boom"));
    await user.click(screen.getByText("Choose file…"));

    await user.click(screen.getByRole("button", { name: "Import" }));

    expect(await screen.findByText("Import failed. Check the file and try again.")).toBeInTheDocument();
  });
});
