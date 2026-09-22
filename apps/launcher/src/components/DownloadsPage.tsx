import { ArrowDownToLine, CheckCircle2, LoaderCircle } from "lucide-react";
import { Button } from "@/components/ui/button";
import { command } from "@/lib/api";
import type { Progress, Status } from "@/lib/models";

const phaseHint: Record<string, string> = {
  IDLE: "Nothing in progress",
  AUTHENTICATING: "Checking your Minecraft account",
  DOWNLOADING: "Downloading game files or content",
  LAUNCHING: "Starting Minecraft",
  RUNNING: "Minecraft is running",
};

export function DownloadsPage({
  status,
  progress,
  active,
}: {
  status: Status;
  progress: Progress | null;
  active: boolean;
}) {
  const busy = active && status.phase !== "RUNNING" && status.phase !== "IDLE";

  return (
    <div className="stack downloads-page">
      <div className="section-heading">
        <div>
          <h2>Downloads</h2>
          <p>Install and update progress for this session.</p>
        </div>
        {busy && (
          <Button
            variant="outline"
            onClick={() => void command("cancel_download")}
          >
            Cancel
          </Button>
        )}
      </div>

      <div className="download-card">
        <div className="row spread">
          <div className="row" style={{ gap: 10 }}>
            {busy ? (
              <LoaderCircle className="spin" size={20} />
            ) : status.phase === "RUNNING" ? (
              <LoaderCircle className="spin" size={20} />
            ) : (
              <CheckCircle2 size={20} />
            )}
            <div>
              <strong>{status.phase}</strong>
              <p>{status.message || phaseHint[status.phase] || "Ready"}</p>
            </div>
          </div>
          <ArrowDownToLine size={18} />
        </div>

        {progress ? (
          <>
            <progress max={progress.total || 1} value={progress.completed} />
            <div className="download-stats">
              <span>
                {progress.completed.toLocaleString()} /{" "}
                {progress.total.toLocaleString()} files
              </span>
              <span>{(progress.bytes / 1048576).toFixed(1)} MB</span>
              <span>
                {(progress.bytesPerSecond / 1048576).toFixed(1)} MB/s
              </span>
              <span>{progress.phase}</span>
            </div>
          </>
        ) : (
          <p className="downloads-idle">
            {busy
              ? "Working…"
              : "When you install Minecraft, Java, updates, or Modrinth mods, progress appears here."}
          </p>
        )}
      </div>

      <div className="download-stages">
        {[
          "AUTHENTICATING",
          "DOWNLOADING",
          "LAUNCHING",
          "RUNNING",
        ].map((phase) => (
          <div
            key={phase}
            className={
              status.phase === phase
                ? "download-stage current"
                : "download-stage"
            }
          >
            <span>{phase}</span>
            <small>{phaseHint[phase]}</small>
          </div>
        ))}
      </div>
    </div>
  );
}
