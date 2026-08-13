import { useState, type ReactElement } from "react";
import type { CardFull, Direction } from "@/types/models";
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

const ALL_DIRECTIONS: Direction[] = ["1->2", "2->1"];

export interface CardFormValues {
  face_1: string;
  face_2: string;
  full: CardFull;
  tags: string[];
  directions: Direction[];
  level: number;
}

const BLANK_VALUES: CardFormValues = {
  face_1: "",
  face_2: "",
  full: { title: "", subtitle: "", body: "", foot: "" },
  tags: [],
  directions: ALL_DIRECTIONS,
  level: 1,
};

export function CardFormDialog({
  trigger,
  title,
  submitLabel,
  initialValues,
  onSubmit,
  onResetProgress,
}: {
  trigger: ReactElement;
  title: string;
  submitLabel: string;
  initialValues?: CardFormValues;
  onSubmit: (values: CardFormValues) => Promise<unknown>;
  onResetProgress?: () => Promise<unknown>;
}) {
  const [open, setOpen] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [face1, setFace1] = useState("");
  const [face2, setFace2] = useState("");
  const [cardTitle, setCardTitle] = useState("");
  const [subtitle, setSubtitle] = useState("");
  const [body, setBody] = useState("");
  const [foot, setFoot] = useState("");
  const [tagsText, setTagsText] = useState("");
  const [directions, setDirections] = useState<Direction[]>(ALL_DIRECTIONS);
  const [levelText, setLevelText] = useState("1");

  const handleOpenChange = (next: boolean) => {
    setOpen(next);
    if (!next) return;
    setError(null);
    const values = initialValues ?? BLANK_VALUES;
    setFace1(values.face_1);
    setFace2(values.face_2);
    setCardTitle(values.full.title);
    setSubtitle(values.full.subtitle);
    setBody(values.full.body);
    setFoot(values.full.foot);
    setTagsText(values.tags.join(", "));
    setDirections(values.directions);
    setLevelText(String(values.level));
  };

  const toggleDirection = (direction: Direction) => {
    setDirections((prev) =>
      prev.includes(direction) ? prev.filter((d) => d !== direction) : [...prev, direction],
    );
  };

  const [resetting, setResetting] = useState(false);
  const [resetError, setResetError] = useState<string | null>(null);

  const handleResetProgress = async () => {
    if (!onResetProgress) return;
    setResetting(true);
    setResetError(null);
    try {
      await onResetProgress();
      setOpen(false);
    } catch (err) {
      setResetError(err instanceof Error ? err.message : String(err));
    } finally {
      setResetting(false);
    }
  };

  const handleSubmit = async () => {
    setSubmitting(true);
    setError(null);
    try {
      await onSubmit({
        face_1: face1,
        face_2: face2,
        full: { title: cardTitle, subtitle, body, foot },
        tags: tagsText
          .split(",")
          .map((t) => t.trim())
          .filter(Boolean),
        directions,
        level: Math.max(1, Number(levelText) || 1),
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
          <div className="grid grid-cols-2 gap-3">
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="face-1">Face 1</Label>
              <Input id="face-1" value={face1} onChange={(e) => setFace1(e.target.value)} />
            </div>
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="face-2">Face 2</Label>
              <Input id="face-2" value={face2} onChange={(e) => setFace2(e.target.value)} />
            </div>
          </div>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="card-title">Title</Label>
            <Input id="card-title" value={cardTitle} onChange={(e) => setCardTitle(e.target.value)} />
          </div>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="card-subtitle">Subtitle</Label>
            <Input id="card-subtitle" value={subtitle} onChange={(e) => setSubtitle(e.target.value)} />
          </div>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="card-body">Body</Label>
            <Textarea id="card-body" value={body} onChange={(e) => setBody(e.target.value)} />
          </div>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="card-foot">Foot</Label>
            <Input id="card-foot" value={foot} onChange={(e) => setFoot(e.target.value)} />
          </div>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="card-tags">Tags (comma-separated)</Label>
            <Input id="card-tags" value={tagsText} onChange={(e) => setTagsText(e.target.value)} />
          </div>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="card-level">Level</Label>
            <NumberInput id="card-level" min={1} value={levelText} onValueChange={setLevelText} />
          </div>
          <div className="flex flex-col gap-1.5">
            <Label>Directions to practice</Label>
            <div className="flex gap-4">
              <label className="flex items-center gap-1.5 text-sm">
                <input
                  type="checkbox"
                  checked={directions.includes("1->2")}
                  onChange={() => toggleDirection("1->2")}
                />
                Face 1 → Face 2
              </label>
              <label className="flex items-center gap-1.5 text-sm">
                <input
                  type="checkbox"
                  checked={directions.includes("2->1")}
                  onChange={() => toggleDirection("2->1")}
                />
                Face 2 → Face 1
              </label>
            </div>
          </div>
          {error && <p className="text-sm text-destructive">{error}</p>}
        </div>
        {initialValues && onResetProgress && (
          <div className="flex flex-col gap-2 border-t pt-3">
            <AlertDialog onOpenChange={(next) => next && setResetError(null)}>
              <AlertDialogTrigger render={<Button variant="outline" size="sm">Reset progress…</Button>} />
              <AlertDialogContent>
                <AlertDialogHeader>
                  <AlertDialogTitle>Reset this card's progress?</AlertDialogTitle>
                  <AlertDialogDescription>
                    Clears its current review schedule so it's treated as a brand-new card again — useful
                    after changing its level. Past review history isn't deleted, only current progress.
                  </AlertDialogDescription>
                </AlertDialogHeader>
                <AlertDialogFooter>
                  <AlertDialogCancel>Cancel</AlertDialogCancel>
                  <AlertDialogAction
                    variant="destructive"
                    onClick={handleResetProgress}
                    disabled={resetting}
                  >
                    Reset
                  </AlertDialogAction>
                </AlertDialogFooter>
              </AlertDialogContent>
            </AlertDialog>
            {resetError && <p className="text-sm text-destructive">{resetError}</p>}
          </div>
        )}
        <DialogFooter>
          <Button
            disabled={!face1.trim() || !face2.trim() || directions.length === 0 || submitting}
            onClick={handleSubmit}
          >
            {submitLabel}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
