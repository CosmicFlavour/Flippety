import { useQuery } from "@tanstack/react-query";
import { ArrowLeft } from "lucide-react";
import { api } from "@/lib/api";
import type { Deck, RatingCounts } from "@/types/models";
import { Button } from "@/components/ui/button";

const RADIUS = 60;
const STROKE_WIDTH = 28;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

const SEGMENTS: { key: keyof RatingCounts; label: string; chart: 1 | 2 | 3 | 4 }[] = [
  { key: "again", label: "Again", chart: 1 },
  { key: "hard", label: "Hard", chart: 2 },
  { key: "good", label: "Good", chart: 3 },
  { key: "easy", label: "Easy", chart: 4 },
];

function RatingPieChart({ ratings }: { ratings: RatingCounts }) {
  const total = ratings.again + ratings.hard + ratings.good + ratings.easy;

  if (total === 0) {
    return <p className="text-muted-foreground">Nothing reviewed yet.</p>;
  }

  let cumulative = 0;
  const arcs = SEGMENTS.map((segment) => {
    const value = ratings[segment.key];
    const fraction = value / total;
    const length = fraction * CIRCUMFERENCE;
    const offset = cumulative;
    cumulative += length;
    return { ...segment, value, length, offset };
  });

  return (
    <div className="flex items-center gap-6">
      <svg viewBox="0 0 160 160" className="size-40 shrink-0">
        <g transform="rotate(-90 80 80)">
          {arcs.map(
            (arc) =>
              arc.value > 0 && (
                <circle
                  key={arc.key}
                  cx={80}
                  cy={80}
                  r={RADIUS}
                  fill="none"
                  style={{ stroke: `var(--chart-${arc.chart})` }}
                  strokeWidth={STROKE_WIDTH}
                  strokeDasharray={`${arc.length} ${CIRCUMFERENCE - arc.length}`}
                  strokeDashoffset={-arc.offset}
                />
              ),
          )}
        </g>
      </svg>

      <div className="flex flex-col gap-2">
        {arcs
          .filter((arc) => arc.value > 0)
          .map((arc) => (
            <div key={arc.key} className="flex items-center gap-2 text-sm">
              <span
                className="size-2.5 rounded-full"
                style={{ backgroundColor: `var(--chart-${arc.chart})` }}
              />
              <span className="text-muted-foreground">{arc.label}</span>
              <span className="font-medium">{arc.value}</span>
            </div>
          ))}
      </div>
    </div>
  );
}

export function StatsPage({ deck, onBack }: { deck: Deck; onBack: () => void }) {
  const statsQuery = useQuery({
    queryKey: ["deckStats", deck.id],
    queryFn: () => api.stats.getDeckStats(deck.id),
  });

  return (
    <div className="mx-auto flex max-w-xl flex-col gap-6 p-6">
      <div className="flex items-center gap-2">
        <Button variant="ghost" size="icon-sm" aria-label="Back" onClick={onBack}>
          <ArrowLeft className="size-4" />
        </Button>
        <h1 className="text-xl font-semibold">{deck.name} — Stats</h1>
      </div>

      {statsQuery.isLoading && <p className="text-muted-foreground">Loading…</p>}

      {statsQuery.data && (
        <>
          <div className="flex flex-col gap-1.5">
            <p className="text-sm text-muted-foreground">Cards introduced</p>
            <p className="text-2xl font-semibold">
              {statsQuery.data.introduced_cards}{" "}
              <span className="text-base font-normal text-muted-foreground">
                of {statsQuery.data.total_cards}
              </span>
            </p>
            <div className="h-2 w-full overflow-hidden rounded-full bg-muted">
              <div
                className="h-full rounded-full bg-primary"
                style={{
                  width:
                    statsQuery.data.total_cards > 0
                      ? `${(statsQuery.data.introduced_cards / statsQuery.data.total_cards) * 100}%`
                      : "0%",
                }}
              />
            </div>
          </div>

          <div className="flex flex-col gap-2">
            <p className="text-sm text-muted-foreground">Last rating per card face</p>
            <RatingPieChart ratings={statsQuery.data.ratings} />
          </div>
        </>
      )}
    </div>
  );
}
