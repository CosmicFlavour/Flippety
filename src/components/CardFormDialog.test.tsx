import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CardFormDialog, type CardFormValues } from "./CardFormDialog";

const SAMPLE_VALUES: CardFormValues = {
  face_1: "dog",
  face_2: "狗",
  full: { title: "狗", subtitle: "gǒu", body: "Domestic dog.", foot: "这是我的狗。" },
  tags: ["animals", "hsk1"],
  directions: ["1->2", "2->1"],
  level: 3,
};

async function openDialog(props: Partial<Parameters<typeof CardFormDialog>[0]> = {}) {
  const user = userEvent.setup();
  const onSubmit = vi.fn().mockResolvedValue(undefined);
  render(
    <CardFormDialog
      trigger={<button>Open</button>}
      title="New card"
      submitLabel="Create"
      onSubmit={onSubmit}
      {...props}
    />,
  );
  await user.click(screen.getByText("Open"));
  return { user, onSubmit };
}

describe("CardFormDialog", () => {
  it("disables submit until both faces are filled", async () => {
    const { user } = await openDialog();
    const submit = screen.getByRole("button", { name: "Create" });
    expect(submit).toBeDisabled();

    await user.type(screen.getByLabelText("Face 1"), "dog");
    expect(submit).toBeDisabled();

    await user.type(screen.getByLabelText("Face 2"), "狗");
    expect(submit).toBeEnabled();
  });

  it("disables submit when no direction is selected", async () => {
    const { user } = await openDialog();
    await user.type(screen.getByLabelText("Face 1"), "dog");
    await user.type(screen.getByLabelText("Face 2"), "狗");
    const submit = screen.getByRole("button", { name: "Create" });
    expect(submit).toBeEnabled();

    await user.click(screen.getByLabelText("Face 1 → Face 2"));
    await user.click(screen.getByLabelText("Face 2 → Face 1"));

    expect(submit).toBeDisabled();
  });

  it("pre-fills every field from initialValues when editing", async () => {
    await openDialog({ title: "Edit card", submitLabel: "Save", initialValues: SAMPLE_VALUES });

    expect(screen.getByLabelText("Face 1")).toHaveValue("dog");
    expect(screen.getByLabelText("Face 2")).toHaveValue("狗");
    expect(screen.getByLabelText("Title")).toHaveValue("狗");
    expect(screen.getByLabelText("Subtitle")).toHaveValue("gǒu");
    expect(screen.getByLabelText("Body")).toHaveValue("Domestic dog.");
    expect(screen.getByLabelText("Foot")).toHaveValue("这是我的狗。");
    expect(screen.getByLabelText("Tags (comma-separated)")).toHaveValue("animals, hsk1");
    expect(screen.getByLabelText("Level")).toHaveValue(3);
    expect(screen.getByLabelText("Face 1 → Face 2")).toBeChecked();
    expect(screen.getByLabelText("Face 2 → Face 1")).toBeChecked();
  });

  it("defaults the level to 1 for a new card", async () => {
    await openDialog();

    expect(screen.getByLabelText("Level")).toHaveValue(1);
  });

  it("reflects a one-directional card's checkboxes correctly", async () => {
    await openDialog({
      title: "Edit card",
      submitLabel: "Save",
      initialValues: { ...SAMPLE_VALUES, directions: ["1->2"] },
    });

    expect(screen.getByLabelText("Face 1 → Face 2")).toBeChecked();
    expect(screen.getByLabelText("Face 2 → Face 1")).not.toBeChecked();
  });

  it("submits tags split, trimmed, and empty entries filtered out", async () => {
    const { user, onSubmit } = await openDialog();
    await user.type(screen.getByLabelText("Face 1"), "dog");
    await user.type(screen.getByLabelText("Face 2"), "狗");
    await user.type(screen.getByLabelText("Tags (comma-separated)"), "animals,  , hsk1 ,");

    await user.click(screen.getByRole("button", { name: "Create" }));

    expect(onSubmit).toHaveBeenCalledWith(
      expect.objectContaining({ tags: ["animals", "hsk1"] }),
    );
  });

  it("submits the full payload shape", async () => {
    const { user, onSubmit } = await openDialog();
    await user.type(screen.getByLabelText("Face 1"), "dog");
    await user.type(screen.getByLabelText("Face 2"), "狗");
    await user.type(screen.getByLabelText("Title"), "狗");

    await user.click(screen.getByRole("button", { name: "Create" }));

    expect(onSubmit).toHaveBeenCalledWith({
      face_1: "dog",
      face_2: "狗",
      full: { title: "狗", subtitle: "", body: "", foot: "" },
      tags: [],
      directions: ["1->2", "2->1"],
      level: 1,
    });
  });

  it("closes the dialog after a successful submit", async () => {
    const { user } = await openDialog();
    await user.type(screen.getByLabelText("Face 1"), "dog");
    await user.type(screen.getByLabelText("Face 2"), "狗");

    await user.click(screen.getByRole("button", { name: "Create" }));

    expect(screen.queryByLabelText("Face 1")).not.toBeInTheDocument();
  });

  it("shows the error and keeps the dialog open with the typed data intact when the submit rejects", async () => {
    const onSubmit = vi.fn().mockRejectedValue(new Error("backend says no"));
    const { user } = await openDialog({ onSubmit });
    await user.type(screen.getByLabelText("Face 1"), "dog");
    await user.type(screen.getByLabelText("Face 2"), "狗");

    await user.click(screen.getByRole("button", { name: "Create" }));

    expect(await screen.findByText("backend says no")).toBeInTheDocument();
    expect(screen.getByLabelText("Face 1")).toHaveValue("dog");
    expect(onSubmit).toHaveBeenCalledTimes(1);
  });

  it("does not show a reset-progress action when creating a new card", async () => {
    await openDialog();

    expect(screen.queryByText("Reset progress…")).not.toBeInTheDocument();
  });

  it("does not show a reset-progress action without an onResetProgress handler", async () => {
    await openDialog({ title: "Edit card", submitLabel: "Save", initialValues: SAMPLE_VALUES });

    expect(screen.queryByText("Reset progress…")).not.toBeInTheDocument();
  });

  it("resets progress after confirming and closes the dialog", async () => {
    const onResetProgress = vi.fn().mockResolvedValue(undefined);
    const { user } = await openDialog({
      title: "Edit card",
      submitLabel: "Save",
      initialValues: SAMPLE_VALUES,
      onResetProgress,
    });

    await user.click(screen.getByText("Reset progress…"));
    await user.click(screen.getByRole("button", { name: "Reset" }));

    expect(onResetProgress).toHaveBeenCalledTimes(1);
    expect(screen.queryByLabelText("Face 1")).not.toBeInTheDocument();
  });

  it("shows an error and keeps the dialog open when reset progress rejects", async () => {
    const onResetProgress = vi.fn().mockRejectedValue(new Error("reset failed"));
    const { user } = await openDialog({
      title: "Edit card",
      submitLabel: "Save",
      initialValues: SAMPLE_VALUES,
      onResetProgress,
    });

    await user.click(screen.getByText("Reset progress…"));
    await user.click(screen.getByRole("button", { name: "Reset" }));

    expect(await screen.findByText("reset failed")).toBeInTheDocument();
    expect(screen.getByLabelText("Face 1")).toBeInTheDocument();
  });
});
