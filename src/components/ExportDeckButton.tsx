import { useState } from "react";
import { Download, Check, X } from "lucide-react";
import { exportDeckToFile } from "@/lib/export-deck";
import type { Deck } from "@/types/models";
import { Button } from "@/components/ui/button";

type ExportState = "idle" | "exporting" | "done" | "error";

export function ExportDeckButton({ deck }: { deck: Deck }) {
  const [state, setState] = useState<ExportState>("idle");

  const handleClick = async () => {
    setState("exporting");
    try {
      const exported = await exportDeckToFile(deck);
      setState(exported ? "done" : "idle");
    } catch {
      setState("error");
    }
    setTimeout(() => setState("idle"), 2000);
  };

  return (
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label="Export deck as JSON"
      onClick={handleClick}
      disabled={state === "exporting"}
    >
      {state === "done" ? (
        <Check className="size-3.5" />
      ) : state === "error" ? (
        <X className="size-3.5 text-destructive" />
      ) : (
        <Download className="size-3.5" />
      )}
    </Button>
  );
}
