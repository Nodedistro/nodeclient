import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { ArrowDownToLine, Check, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { command, desktop, message } from "@/lib/api";

export type UpdateInfo = {
  currentVersion: string;
  latestVersion: string;
  available: boolean;
  title: string;
  notes: string;
  htmlUrl: string;
  assetName: string;
  downloadUrl: string;
  sha256: string;
  publishedAt: string | null;
};

type UpdateProgress = {
  downloaded: number;
  total: number | null;
};

export function UpdatePanel({ onError }: { onError: (s: string) => void }) {
  const [info, setInfo] = useState<UpdateInfo | null>(null);
  const [checking, setChecking] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [progress, setProgress] = useState<UpdateProgress | null>(null);
  const [done, setDone] = useState("");

  useEffect(() => {
    if (!desktop) return;
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void listen<UpdateProgress>("update-progress", (e) => {
      setProgress(e.payload);
    }).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  async function check() {
    setChecking(true);
    setDone("");
    setProgress(null);
    try {
      setInfo(await command<UpdateInfo>("check_for_updates"));
    } catch (e) {
      onError(message(e));
    } finally {
      setChecking(false);
    }
  }

  async function install() {
    if (!info?.available) return;
    setInstalling(true);
    setDone("");
    setProgress({ downloaded: 0, total: null });
    try {
      const path = await command<string>("install_update", { info });
      setDone(
        `Updating in place. NodeClient will restart when the installer finishes.`,
      );
    } catch (e) {
      onError(message(e));
    } finally {
      setInstalling(false);
    }
  }

  return (
    <div className="stack">
      <div className="setting-row">
        <div>
          <h3>NodeClient updates</h3>
          <p>
            Checks GitHub releases for{" "}
            <span className="mono">Nodedistro/nodeclient</span>. Downloads
            require HTTPS and a published SHA-256 checksum. Updates install
            in place — instances, worlds, and screenshots are kept.
          </p>
        </div>
        <Button
          variant="outline"
          disabled={!desktop || checking}
          onClick={() => void check()}
        >
          <RefreshCw size={16} className={checking ? "spin" : undefined} />
          {checking ? "Checking…" : "Check for updates"}
        </Button>
      </div>
      {info && (
        <>
          <p>
            Current <strong className="mono">{info.currentVersion}</strong>
            {" · "}
            Latest <strong className="mono">{info.latestVersion}</strong>
          </p>
          {info.available ? (
            <>
              <h3>{info.title}</h3>
              <pre className="update-notes">
                {info.notes || "No release notes."}
              </pre>
              <p className="mono">SHA-256 {info.sha256}</p>
              {progress && (
                <>
                  <progress
                    max={progress.total || 1}
                    value={progress.downloaded}
                  />
                  <p>
                    {(progress.downloaded / 1048576).toFixed(1)} MB
                    {progress.total
                      ? ` / ${(progress.total / 1048576).toFixed(1)} MB`
                      : ""}
                  </p>
                </>
              )}
              <div className="row">
                <Button disabled={installing} onClick={() => void install()}>
                  <ArrowDownToLine size={16} />
                  {installing ? "Downloading…" : "Download and update"}
                </Button>
                {installing && (
                  <Button
                    variant="ghost"
                    onClick={() => void command("cancel_update")}
                  >
                    Cancel
                  </Button>
                )}
              </div>
            </>
          ) : (
            <span className="verified">
              <Check size={15} />
              You are up to date
            </span>
          )}
        </>
      )}
      {done && <p>{done}</p>}
    </div>
  );
}
