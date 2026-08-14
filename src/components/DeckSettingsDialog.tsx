import { useState, type ReactElement } from "react";
import { exportDeckToFile } from "@/lib/export-deck";
import type { Deck } from "@/types/models";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { NumberInput } from "@/components/ui/number-input";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
  DialogTrigger,
} from "@/components/ui/dialog";
import {
  AlertDialog,
  AlertDialogTrigger,
  AlertDialogContent,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogAction,
  AlertDialogCancel,
} from "@/components/ui/alert-dialog";

export interface DeckSettingsValues {
  name: string;
  description: string | null;
  new_cards_per_day: number | null;
}

type ExportState = "idle" | "exporting" | "done" | "error";

export function DeckSettingsDialog({
  trigger,
  deck,
  onSave,
  onDelete,
}: {
  trigger: ReactElement;
  deck: Deck;
  onSave: (values: DeckSettingsValues) => Promise<unknown>;
  onDelete: () => Promise<unknown>;
}) {
  const [open, setOpen] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [newCardsPerDayText, setNewCardsPerDayText] = useState("");
  const [exportState, setExportState] = useState<ExportState>("idle");

  const handleOpenChange = (next: boolean) => {
    setOpen(next);
    if (!next) return;
    setError(null);
    setExportState("idle");
    setName(deck.name);
    setDescription(deck.description ?? "");
    setNewCardsPerDayText(deck.new_cards_per_day?.toString() ?? "");
  };

  const handleSave = async () => {
    setSubmitting(true);
    setError(null);
    try {
      const trimmed = newCardsPerDayText.trim();
      await onSave({
        name,
        description: description || null,
        new_cards_per_day: trimmed === "" ? null : Math.max(0, Number(trimmed) || 0),
      });
      setOpen(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSubmitting(false);
    }
  };

  const handleExport = async () => {
    setExportState("exporting");
    try {
      const exported = await exportDeckToFile(deck);
      setExportState(exported ? "done" : "idle");
    } catch {
      setExportState("error");
    }
  };

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogTrigger render={trigger} />
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Deck settings</DialogTitle>
        </DialogHeader>
        <div className="flex flex-col gap-3">
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="deck-settings-name">Name</Label>
            <Input id="deck-settings-name" value={name} onChange={(e) => setName(e.target.value)} />
          </div>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="deck-settings-description">Description</Label>
            <Textarea
              id="deck-settings-description"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
            />
          </div>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="deck-settings-new-cards-per-day">New cards per day</Label>
            <NumberInput
              id="deck-settings-new-cards-per-day"
              min={0}
              placeholder="Unlimited"
              value={newCardsPerDayText}
              onValueChange={setNewCardsPerDayText}
            />
          </div>
          {error && <p className="text-sm text-destructive">{error}</p>}
          <Button variant="outline" onClick={handleExport} disabled={exportState === "exporting"}>
            {exportState === "done"
              ? "Exported ✓"
              : exportState === "error"
                ? "Export failed — try again"
                : "Export as JSON…"}
          </Button>
        </div>
        <DialogFooter>
          <Button disabled={!name.trim() || submitting} onClick={handleSave}>
            Save
          </Button>
        </DialogFooter>

        <div className="mt-2 flex items-center justify-between gap-3 rounded-lg border border-destructive/30 bg-destructive/5 p-3">
          <div>
            <p className="text-sm font-medium">Danger zone</p>
            <p className="text-xs text-muted-foreground">Delete this deck and all its cards.</p>
          </div>
          <AlertDialog>
            <AlertDialogTrigger render={<Button variant="destructive">Delete deck</Button>} />
            <AlertDialogContent>
              <AlertDialogHeader>
                <AlertDialogTitle>Delete "{deck.name}"?</AlertDialogTitle>
                <AlertDialogDescription>
                  This permanently deletes the deck and all its cards, including their study
                  history. This can't be undone.
                </AlertDialogDescription>
              </AlertDialogHeader>
              <AlertDialogFooter>
                <AlertDialogCancel>Cancel</AlertDialogCancel>
                <AlertDialogAction
                  variant="destructive"
                  onClick={async () => {
                    await onDelete();
                    setOpen(false);
                  }}
                >
                  Delete
                </AlertDialogAction>
              </AlertDialogFooter>
            </AlertDialogContent>
          </AlertDialog>
        </div>
      </DialogContent>
    </Dialog>
  );
}
