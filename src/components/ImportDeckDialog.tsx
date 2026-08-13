import { useState, type ReactElement } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api } from "@/lib/api";
import type { Deck, ImportMode } from "@/types/models";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
  DialogTrigger,
} from "@/components/ui/dialog";

export function ImportDeckDialog({
  trigger,
  existingDecks,
  onImported,
}: {
  trigger: ReactElement;
  existingDecks: Deck[];
  onImported: (deck: Deck) => void;
}) {
  const [open_, setOpen] = useState(false);
  const [path, setPath] = useState<string | null>(null);
  const [target, setTarget] = useState<"new" | string>("new");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const reset = () => {
    setPath(null);
    setTarget("new");
    setError(null);
  };

  const handleChooseFile = async () => {
    const selected = await open({
      title: "Choose a deck to import",
      multiple: false,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (selected) setPath(selected);
  };

  const handleImport = async () => {
    if (!path) return;
    setSubmitting(true);
    setError(null);
    try {
      const mode: ImportMode = target === "new" ? { mode: "new" } : { mode: "merge", deck_id: target };
      const deck = await api.importExport.importDeck(path, mode);
      onImported(deck);
      setOpen(false);
      reset();
    } catch (err) {
      setError(typeof err === "string" ? err : "Import failed. Check the file and try again.");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Dialog
      open={open_}
      onOpenChange={(next) => {
        setOpen(next);
        if (next) reset();
      }}
    >
      <DialogTrigger render={trigger} />
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Import deck</DialogTitle>
        </DialogHeader>
        <div className="flex flex-col gap-3">
          <div className="flex flex-col gap-1.5">
            <Label>File</Label>
            <Button variant="outline" onClick={handleChooseFile}>
              {path ? path.split("/").pop() : "Choose file…"}
            </Button>
          </div>

          <div className="flex flex-col gap-1.5">
            <Label htmlFor="import-target">Import into</Label>
            <select
              id="import-target"
              className="h-8 rounded-lg border border-input bg-transparent px-2.5 text-sm"
              value={target}
              onChange={(e) => setTarget(e.target.value)}
            >
              <option value="new">A new deck</option>
              {existingDecks.map((deck) => (
                <option key={deck.id} value={deck.id}>
                  Merge into "{deck.name}"
                </option>
              ))}
            </select>
          </div>

          {error && <p className="text-sm text-destructive">{error}</p>}
        </div>
        <DialogFooter>
          <Button disabled={!path || submitting} onClick={handleImport}>
            Import
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
