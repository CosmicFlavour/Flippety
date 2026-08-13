import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DeckFormDialog, type DeckFormValues } from "./DeckFormDialog";

async function openDialog(props: Partial<Parameters<typeof DeckFormDialog>[0]> = {}) {
  const user = userEvent.setup();
  const onSubmit = vi.fn().mockResolvedValue(undefined);
  render(
    <DeckFormDialog
      trigger={<button>Open</button>}
      title="New deck"
      submitLabel="Create"
      onSubmit={onSubmit}
      {...props}
    />,
  );
  await user.click(screen.getByText("Open"));
  return { user, onSubmit };
}

describe("DeckFormDialog", () => {
  it("disables submit until a name is entered", async () => {
    const { user } = await openDialog();
    const submit = screen.getByRole("button", { name: "Create" });
    expect(submit).toBeDisabled();

    await user.type(screen.getByLabelText("Name"), "Chinese HSK1");
    expect(submit).toBeEnabled();

    await user.clear(screen.getByLabelText("Name"));
    expect(submit).toBeDisabled();
  });

  it("pre-fills name, description, and new cards per day when editing", async () => {
    const initialValues: DeckFormValues = {
      name: "Chinese HSK1",
      description: "Vocab deck",
      new_cards_per_day: 20,
    };
    await openDialog({ title: "Edit deck", submitLabel: "Save", initialValues });

    expect(screen.getByLabelText("Name")).toHaveValue("Chinese HSK1");
    expect(screen.getByLabelText("Description")).toHaveValue("Vocab deck");
    expect(screen.getByLabelText("New cards per day")).toHaveValue(20);
  });

  it("treats a null description and new_cards_per_day as empty fields", async () => {
    await openDialog({
      title: "Edit deck",
      submitLabel: "Save",
      initialValues: { name: "Chinese HSK1", description: null, new_cards_per_day: null },
    });

    expect(screen.getByLabelText("Description")).toHaveValue("");
    expect(screen.getByLabelText("New cards per day")).toHaveValue(null);
  });

  it("submits the trimmed name and description, converting blanks to null", async () => {
    const { user, onSubmit } = await openDialog();
    await user.type(screen.getByLabelText("Name"), "Chinese HSK1");

    await user.click(screen.getByRole("button", { name: "Create" }));

    expect(onSubmit).toHaveBeenCalledWith({
      name: "Chinese HSK1",
      description: null,
      new_cards_per_day: null,
    });
  });

  it("submits a non-empty description and new cards per day as-is", async () => {
    const { user, onSubmit } = await openDialog();
    await user.type(screen.getByLabelText("Name"), "Chinese HSK1");
    await user.type(screen.getByLabelText("Description"), "Vocab deck");
    await user.type(screen.getByLabelText("New cards per day"), "20");

    await user.click(screen.getByRole("button", { name: "Create" }));

    expect(onSubmit).toHaveBeenCalledWith({
      name: "Chinese HSK1",
      description: "Vocab deck",
      new_cards_per_day: 20,
    });
  });

  it("closes the dialog after a successful submit", async () => {
    const { user } = await openDialog();
    await user.type(screen.getByLabelText("Name"), "Chinese HSK1");

    await user.click(screen.getByRole("button", { name: "Create" }));

    expect(screen.queryByLabelText("Name")).not.toBeInTheDocument();
  });

  it("shows the error and keeps the dialog open when the submit rejects", async () => {
    const onSubmit = vi.fn().mockRejectedValue(new Error("deck name cannot be empty"));
    const { user } = await openDialog({ onSubmit });
    await user.type(screen.getByLabelText("Name"), "Chinese HSK1");

    await user.click(screen.getByRole("button", { name: "Create" }));

    expect(await screen.findByText("deck name cannot be empty")).toBeInTheDocument();
    expect(screen.getByLabelText("Name")).toHaveValue("Chinese HSK1");
  });
});
