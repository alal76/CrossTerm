import { useState, useEffect } from "react";
import type { Breakpoint } from "@/types";

function getBreakpoint(width: number): Breakpoint {
  if (width < 600) return "compact";
  if (width < 900) return "medium";
  if (width < 1200) return "expanded";
  return "large";
}

export function useBreakpoint(): Breakpoint {
  const [breakpoint, setBreakpoint] = useState<Breakpoint>(() =>
    getBreakpoint(globalThis.innerWidth),
  );

  useEffect(() => {
    function handleResize() {
      setBreakpoint(getBreakpoint(globalThis.innerWidth));
    }
    globalThis.addEventListener("resize", handleResize);
    return () => globalThis.removeEventListener("resize", handleResize);
  }, []);

  return breakpoint;
}
