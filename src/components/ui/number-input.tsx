import * as React from "react";
import { ChevronUp, ChevronDown } from "lucide-react";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";

/** A number input with the native browser spin buttons replaced by a themed stepper. */
function NumberInput({
  className,
  value,
  onValueChange,
  min,
  max,
  step = 1,
  ...props
}: Omit<React.ComponentProps<"input">, "onChange" | "value" | "type"> & {
  value: string;
  onValueChange: (value: string) => void;
  min?: number;
  max?: number;
  step?: number;
}) {
  const adjust = (delta: number) => {
    const current = Number(value);
    const base = value !== "" && Number.isFinite(current) ? current : (min ?? 0);
    let next = base + delta;
    if (min !== undefined) next = Math.max(min, next);
    if (max !== undefined) next = Math.min(max, next);
    onValueChange(String(next));
  };

  const atMin = min !== undefined && value !== "" && Number(value) <= min;
  const atMax = max !== undefined && value !== "" && Number(value) >= max;

  return (
    <div className={cn("relative", className)}>
      <Input
        type="number"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onValueChange(e.target.value)}
        className="pr-8 [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
        {...props}
      />
      <div className="absolute inset-y-1 right-1 flex w-6 flex-col overflow-hidden rounded-full">
        <button
          type="button"
          tabIndex={-1}
          aria-label="Increase"
          disabled={atMax}
          onClick={() => adjust(step)}
          className="flex flex-1 items-center justify-center text-muted-foreground transition-colors hover:bg-muted hover:text-foreground disabled:pointer-events-none disabled:opacity-40"
        >
          <ChevronUp className="size-3" />
        </button>
        <button
          type="button"
          tabIndex={-1}
          aria-label="Decrease"
          disabled={atMin}
          onClick={() => adjust(-step)}
          className="flex flex-1 items-center justify-center text-muted-foreground transition-colors hover:bg-muted hover:text-foreground disabled:pointer-events-none disabled:opacity-40"
        >
          <ChevronDown className="size-3" />
        </button>
      </div>
    </div>
  );
}

export { NumberInput };
