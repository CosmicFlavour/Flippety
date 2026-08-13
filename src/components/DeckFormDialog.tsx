import { useState, type ReactElement } from "react";
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

export interface DeckFormValues {
  name: string;
  description: string | null;
  new_cards_per_day: number | null;
}

const BLANK_VALUES: DeckFormValues = { name: "", description: "", new_cards_per_day: null };

export function DeckFormDialog({
  trigger,
  title,
  submitLabel,
  initialValues,
  onSubmit,
}: {
  trigger: ReactElement;
  title: string;
  submitLabel: string;
  initialValues?: DeckFormValues;
  onSubmit: (values: DeckFormValues) => Promise<unknown>;
}) {
  const [open, setOpen] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [newCardsPerDayText, setNewCardsPerDayText] = useState("");

  const handleOpenChange = (next: boolean) => {
    setOpen(next);
    if (!next) return;
    setError(null);
    const values = initialValues ?? BLANK_VALUES;
    setName(values.name);
    setDescription(values.description ?? "");
    setNewCardsPerDayText(values.new_cards_per_day?.toString() ?? "");
  };

  const handleSubmit = async () => {
    setSubmitting(true);
    setError(null);
    try {
      const trimmed = newCardsPerDayText.trim();
      await onSubmit({
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

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogTrigger render={trigger} />
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
        </DialogHeader>
        <div className="flex flex-col gap-3">
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="deck-name">Name</Label>
            <Input id="deck-name" value={name} onChange={(e) => setName(e.target.value)} />
          </div>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="deck-description">Description</Label>
            <Textarea
              id="deck-description"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
            />
          </div>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="deck-new-cards-per-day">New cards per day</Label>
            <NumberInput
              id="deck-new-cards-per-day"
              min={0}
              placeholder="Unlimited"
              value={newCardsPerDayText}
              onValueChange={setNewCardsPerDayText}
            />
          </div>
          {error && <p className="text-sm text-destructive">{error}</p>}
        </div>
        <DialogFooter>
          <Button disabled={!name.trim() || submitting} onClick={handleSubmit}>
            {submitLabel}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
