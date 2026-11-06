import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { api, type UpdateInfo } from "@/api";

const POLL_MS = 2000;
// The CLI writes its marker a moment after the click. Ignore "not updating" until then.
const GRACE_MS = 8000;

/** Update check on start, plus an "Update" action that polls until the install ends. */
export function useUpdate() {
  const [info, setInfo] = useState<UpdateInfo | null>(null);
  const [busy, setBusy] = useState(false);
  const startedAt = useRef(0);

  useEffect(() => {
    api
      .updateInfo()
      .then(setInfo)
      .catch(() => {
        // Offline or no CLI: no banner.
      });
  }, []);

  useEffect(() => {
    if (!busy) return;
    const timer = setInterval(() => {
      api
        .updateInfo()
        .then((next) => {
          setInfo(next);
          if (next.updating || Date.now() - startedAt.current < GRACE_MS) return;
          setBusy(false);
          if (next.updateAvailable) toast.error("Update did not finish. See update.log in the config dir.");
          else toast.success(`Updated to ${next.latest}`);
        })
        .catch(() => undefined);
    }, POLL_MS);
    return () => clearInterval(timer);
  }, [busy]);

  const start = useCallback(async () => {
    startedAt.current = Date.now();
    setBusy(true);
    try {
      await api.startUpdate();
    } catch (e) {
      setBusy(false);
      toast.error(String(e));
    }
  }, []);

  return { info, updating: busy || (info?.updating ?? false), start };
}
