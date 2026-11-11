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

  it("pre-fills name, description, and intro direction when editing", async () => {
    const initialValues: DeckFormValues = {
      name: "Chinese HSK1",
      description: "Vocab deck",
      intro_direction: "2->1",
    };
    await openDialog({ title: "Edit deck", submitLabel: "Save", initialValues });

    expect(screen.getByLabelText("Name")).toHaveValue("Chinese HSK1");
    expect(screen.getByLabelText("Description")).toHaveValue("Vocab deck");
    expect(screen.getByLabelText("Face 2 → Face 1")).toBeChecked();
    expect(screen.getByLabelText("Face 1 → Face 2")).not.toBeChecked();
  });

  it("submits the trimmed name and description, converting blanks to null", async () => {
    const { user, onSubmit } = await openDialog();
    await user.type(screen.getByLabelText("Name"), "Chinese HSK1");

    await user.click(screen.getByRole("button", { name: "Create" }));

    expect(onSubmit).toHaveBeenCalledWith({
      name: "Chinese HSK1",
      description: null,
      intro_direction: "1->2",
    });
  });

  it("submits a non-empty description and the selected intro direction", async () => {
    const { user, onSubmit } = await openDialog();
    await user.type(screen.getByLabelText("Name"), "Chinese HSK1");
    await user.type(screen.getByLabelText("Description"), "Vocab deck");
    await user.click(screen.getByLabelText("Face 2 → Face 1"));

    await user.click(screen.getByRole("button", { name: "Create" }));

    expect(onSubmit).toHaveBeenCalledWith({
      name: "Chinese HSK1",
      description: "Vocab deck",
      intro_direction: "2->1",
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
