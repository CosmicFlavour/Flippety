import { useState, type ReactElement } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import type { Direction } from "@/types/models";
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
  intro_direction: Direction;
}

const BLANK_VALUES: DeckFormValues = { name: "", description: "", intro_direction: "1->2" };

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
  const [introDirection, setIntroDirection] = useState<Direction>("1->2");

  const handleOpenChange = (next: boolean) => {
    setOpen(next);
    if (!next) return;
    setError(null);
    const values = initialValues ?? BLANK_VALUES;
    setName(values.name);
    setDescription(values.description ?? "");
    setIntroDirection(values.intro_direction);
  };

  const handleSubmit = async () => {
    setSubmitting(true);
    setError(null);
    try {
      await onSubmit({
        name,
        description: description || null,
        intro_direction: introDirection,
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
            <Label>First direction introduced</Label>
            <div className="flex gap-4">
              <label className="flex items-center gap-1.5 text-sm">
                <input
                  type="radio"
                  name="deck-form-intro-direction"
                  checked={introDirection === "1->2"}
                  onChange={() => setIntroDirection("1->2")}
                />
                Face 1 → Face 2
              </label>
              <label className="flex items-center gap-1.5 text-sm">
                <input
                  type="radio"
                  name="deck-form-intro-direction"
                  checked={introDirection === "2->1"}
                  onChange={() => setIntroDirection("2->1")}
                />
                Face 2 → Face 1
              </label>
            </div>
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
