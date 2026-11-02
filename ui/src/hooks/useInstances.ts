import { useCallback, useEffect, useState } from "react";
import { api, type InstanceView } from "@/api";

const POLL_MS = 2000;

/** Live list of instances: `instances` events plus a 2 s poll as fallback. */
export function useInstances() {
  const [views, setViews] = useState<InstanceView[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(
    () =>
      api
        .listInstances()
        .then((v) => {
          setViews(v);
          setError(null);
        })
        .catch((e: unknown) => setError(String(e)))
        .finally(() => setLoaded(true)),
    [],
  );

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    const timer = setInterval(() => void load(), POLL_MS);
    void load();
    void api.subscribe((v) => setViews(v)).then((u) => {
      if (cancelled) u();
      else unlisten = u;
    });
    return () => {
      cancelled = true;
      clearInterval(timer);
      unlisten?.();
    };
  }, [load]);

  return { views, loaded, error, refresh: load };
}
