import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ThemeProvider, useTheme } from "./theme";

function createMatchMedia(initialMatches: boolean) {
  let matches = initialMatches;
  const listeners = new Set<(e: { matches: boolean }) => void>();
  const mql = {
    get matches() {
      return matches;
    },
    addEventListener: (_: string, cb: (e: { matches: boolean }) => void) => listeners.add(cb),
    removeEventListener: (_: string, cb: (e: { matches: boolean }) => void) => listeners.delete(cb),
  };
  return {
    mql,
    setMatches(next: boolean) {
      matches = next;
      listeners.forEach((cb) => cb({ matches: next }));
    },
    listenerCount: () => listeners.size,
  };
}

function TestConsumer() {
  const { theme, setTheme } = useTheme();
  return (
    <div>
      <span data-testid="theme">{theme}</span>
      <button onClick={() => setTheme("light")}>light</button>
      <button onClick={() => setTheme("dark")}>dark</button>
      <button onClick={() => setTheme("system")}>system</button>
    </div>
  );
}

describe("ThemeProvider", () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.classList.remove("dark");
  });

  it("defaults to system and applies the dark class when the OS prefers dark", () => {
    const { mql } = createMatchMedia(true);
    vi.stubGlobal("matchMedia", vi.fn().mockReturnValue(mql));

    render(
      <ThemeProvider>
        <TestConsumer />
      </ThemeProvider>,
    );

    expect(screen.getByTestId("theme")).toHaveTextContent("system");
    expect(document.documentElement.classList.contains("dark")).toBe(true);
  });

  it("defaults to system and skips the dark class when the OS prefers light", () => {
    const { mql } = createMatchMedia(false);
    vi.stubGlobal("matchMedia", vi.fn().mockReturnValue(mql));

    render(
      <ThemeProvider>
        <TestConsumer />
      </ThemeProvider>,
    );

    expect(document.documentElement.classList.contains("dark")).toBe(false);
  });

  it("reads a persisted theme from localStorage on mount", () => {
    localStorage.setItem("flippety-theme", "dark");
    const { mql } = createMatchMedia(false);
    vi.stubGlobal("matchMedia", vi.fn().mockReturnValue(mql));

    render(
      <ThemeProvider>
        <TestConsumer />
      </ThemeProvider>,
    );

    expect(screen.getByTestId("theme")).toHaveTextContent("dark");
    expect(document.documentElement.classList.contains("dark")).toBe(true);
  });

  it("setTheme persists the choice and updates the dark class immediately", async () => {
    const user = userEvent.setup();
    const { mql } = createMatchMedia(false);
    vi.stubGlobal("matchMedia", vi.fn().mockReturnValue(mql));

    render(
      <ThemeProvider>
        <TestConsumer />
      </ThemeProvider>,
    );
    await user.click(screen.getByText("dark"));

    expect(screen.getByTestId("theme")).toHaveTextContent("dark");
    expect(document.documentElement.classList.contains("dark")).toBe(true);
    expect(localStorage.getItem("flippety-theme")).toBe("dark");
  });

  it("reacts to OS preference changes while in system mode", () => {
    const { mql, setMatches } = createMatchMedia(false);
    vi.stubGlobal("matchMedia", vi.fn().mockReturnValue(mql));

    render(
      <ThemeProvider>
        <TestConsumer />
      </ThemeProvider>,
    );
    expect(document.documentElement.classList.contains("dark")).toBe(false);

    act(() => setMatches(true));

    expect(document.documentElement.classList.contains("dark")).toBe(true);
  });

  it("ignores OS preference changes once a theme is explicitly picked", async () => {
    const user = userEvent.setup();
    const { mql, setMatches } = createMatchMedia(false);
    vi.stubGlobal("matchMedia", vi.fn().mockReturnValue(mql));

    render(
      <ThemeProvider>
        <TestConsumer />
      </ThemeProvider>,
    );
    await user.click(screen.getByText("light"));

    act(() => setMatches(true));

    expect(document.documentElement.classList.contains("dark")).toBe(false);
  });

  it("throws when used outside a ThemeProvider", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    expect(() => render(<TestConsumer />)).toThrow("useTheme must be used within a ThemeProvider");
    spy.mockRestore();
  });
});
