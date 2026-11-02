import { useCallback, useEffect, useRef, useState } from "react";
import { api, type InstanceView } from "@/api";

const POLL_MS = 2000;

/** Live list of instances: `instances` events plus a 2 s poll as fallback. */
export function useInstances() {
  const [views, setViews] = useState<InstanceView[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // StrictMode mounts, unmounts and remounts; the effect below resets this on each mount.
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const load = useCallback(
    () =>
      api
        .listInstances()
        .then((v) => {
          if (!mounted.current) return;
          setViews(v);
          setError(null);
        })
        .catch((e: unknown) => {
          if (mounted.current) setError(String(e));
        })
        .finally(() => {
          if (mounted.current) setLoaded(true);
        }),
    [],
  );

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    const timer = setInterval(() => void load(), POLL_MS);
    void load();
    api
      .subscribe((v) => {
        if (mounted.current) setViews(v);
      })
      .then((u) => {
        if (cancelled) u();
        else unlisten = u;
      })
      .catch((e: unknown) => {
        if (mounted.current) setError(String(e));
      });
    return () => {
      cancelled = true;
      clearInterval(timer);
      unlisten?.();
    };
  }, [load]);

  return { views, loaded, error, refresh: load };
}
