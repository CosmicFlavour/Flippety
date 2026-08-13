import { useState } from "react";
import type { Deck } from "@/types/models";
import { DecksPage } from "@/pages/DecksPage";
import { CardsPage } from "@/pages/CardsPage";
import { StudyPage } from "@/pages/StudyPage";
import { SettingsDialog } from "@/components/SettingsDialog";

type View =
  | { name: "decks" }
  | { name: "cards"; deck: Deck }
  | { name: "study"; deck: Deck };

function App() {
  const [view, setView] = useState<View>({ name: "decks" });

  return (
    <div className="flex min-h-screen flex-col">
      <header className="flex items-center justify-between border-b px-6 py-2.5">
        <span className="text-sm font-semibold">Flippety</span>
        <SettingsDialog />
      </header>

      <div className="flex-1">
        {view.name === "cards" && (
          <CardsPage
            deck={view.deck}
            onBack={() => setView({ name: "decks" })}
            onStudy={() => setView({ name: "study", deck: view.deck })}
          />
        )}
        {view.name === "study" && (
          <StudyPage deck={view.deck} onBack={() => setView({ name: "cards", deck: view.deck })} />
        )}
        {view.name === "decks" && <DecksPage onOpenDeck={(deck) => setView({ name: "cards", deck })} />}
      </div>
    </div>
  );
}

export default App;
