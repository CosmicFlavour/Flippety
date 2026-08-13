import { useState, type ReactElement } from "react";
import { exportDeckToFile } from "@/lib/export-deck";
import type { Deck } from "@/types/models";
import { Button } from "@/components/ui/button";
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

type ExportState = "idle" | "exporting" | "done" | "error";

export function DeleteDeckDialog({
  trigger,
  deck,
  onDelete,
}: {
  trigger: ReactElement;
  deck: Deck;
  onDelete: () => Promise<unknown>;
}) {
  const [exportState, setExportState] = useState<ExportState>("idle");

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
    <AlertDialog
      onOpenChange={(open) => {
        if (open) setExportState("idle");
      }}
    >
      <AlertDialogTrigger render={trigger} />
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Delete "{deck.name}"?</AlertDialogTitle>
          <AlertDialogDescription>
            This permanently deletes the deck and all its cards, including their study history.
            This can't be undone.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <div className="flex flex-col gap-2">
          <Button variant="outline" onClick={handleExport} disabled={exportState === "exporting"}>
            {exportState === "done" ? "Exported ✓" : "Export as JSON…"}
          </Button>
          {exportState === "error" && (
            <p className="text-sm text-destructive">Export failed. Try again before deleting.</p>
          )}
        </div>
        <AlertDialogFooter>
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          <AlertDialogAction variant="destructive" onClick={() => onDelete()}>
            Delete
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
