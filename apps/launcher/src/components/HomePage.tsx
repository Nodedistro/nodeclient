import {
  ArrowDownToLine,
  FolderOpen,
  LoaderCircle,
  Play,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { command } from "@/lib/api";
import type { Instance, Progress, Snapshot } from "@/lib/models";

export function HomePage({
  data,
  selected,
  accountReady,
  installed,
  active,
  desktop,
  progress,
  playLabel,
  onChooseInstance,
  onPlay,
  onOpenLogs,
  onAction,
}: {
  data: Snapshot;
  selected?: Instance;
  accountReady: boolean;
  installed: boolean;
  active: boolean;
  desktop: boolean;
  progress: Progress | null;
  playLabel: string;
  onChooseInstance: (id: string) => void;
  onPlay: () => void;
  onOpenLogs: (kind: string) => void;
  onAction: (task: () => Promise<unknown>) => Promise<void>;
}) {
  return (
    <>
      <section className="home-stage">
        <div className="home-atmosphere" aria-hidden="true">
          <div className="home-ridge mid" />
          <div className="home-ridge" />
        </div>
        <div className="home-content">
          <div className="home-brand">
            <img src="/nodeclient.svg" alt="" />
            <div className="home-brand-text">
              NodeClient
              <span>Java Edition</span>
            </div>
          </div>
          <p className="home-line">Launch Minecraft with your worlds, your way.</p>
        </div>
        <div className="home-launch">
          <div className="home-launch-meta">
            <label htmlFor="home-instance">Instance</label>
            {selected ? (
              <Select
                value={selected.id}
                onValueChange={onChooseInstance}
                disabled={active}
              >
                <SelectTrigger id="home-instance" className="instance-select">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {data.instances.map((i) => (
                    <SelectItem key={i.id} value={i.id}>
                      {i.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            ) : (
              <h3>No instance yet</h3>
            )}
            <p>
              {selected
                ? `Minecraft ${selected.minecraftVersion} · Vanilla · ${
                    selected.java.mode === "custom"
                      ? "Custom Java"
                      : "Automatic Java"
                  } · ${selected.memory.maximumMb / 1024} GB`
                : "Create an instance to install and play."}
            </p>
          </div>
          <Button
            className={
              playLabel === "PLAY" ? "play-button play-ready" : "play-button"
            }
            disabled={active || !desktop}
            onClick={onPlay}
          >
            {active ? (
              <LoaderCircle className="spin" />
            ) : installed ? (
              <Play fill="currentColor" />
            ) : (
              <ArrowDownToLine />
            )}
            {playLabel}
          </Button>
          <span className="home-status">
            <span
              className={
                data.status.phase === "RUNNING" ? "live-dot" : "status-dot"
              }
            />
            {data.status.phase === "RUNNING"
              ? "Minecraft running"
              : !accountReady && installed
                ? "Sign in to play"
                : data.status.message}
          </span>
        </div>
      </section>
      {active && (
        <div className="download-panel">
          <div className="row spread">
            <strong>{data.status.message}</strong>
            {data.status.phase !== "RUNNING" && (
              <Button
                size="sm"
                variant="ghost"
                onClick={() => void command("cancel_download")}
              >
                Cancel
              </Button>
            )}
          </div>
          {progress && (
            <>
              <progress max={progress.total || 1} value={progress.completed} />
              <p>
                {progress.completed.toLocaleString()} /{" "}
                {progress.total.toLocaleString()} files ·{" "}
                {(progress.bytes / 1048576).toFixed(1)} MB transferred ·{" "}
                {(progress.bytesPerSecond / 1048576).toFixed(1)} MB/s
              </p>
            </>
          )}
        </div>
      )}
      {data.status.process?.exitCode != null &&
        data.status.process.exitCode !== 0 && (
          <div className="crash-notice">
            <strong>Minecraft appears to have crashed.</strong>
            <div className="row">
              <Button variant="outline" onClick={() => onOpenLogs("Minecraft")}>
                View logs
              </Button>
              <Button
                variant="outline"
                onClick={() => onOpenLogs("Crash Reports")}
              >
                View crash report
              </Button>
              <Button
                variant="outline"
                onClick={() =>
                  void onAction(async () => {
                    const text = await command<string>("read_logs", {
                      id: selected?.id,
                      kind: "Minecraft",
                    });
                    await navigator.clipboard.writeText(
                      `NodeClient 0.1.1\nMinecraft ${selected?.minecraftVersion}\nExit ${data.status.process?.exitCode}\n${text}`,
                    );
                  })
                }
              >
                Copy diagnostics
              </Button>
              <Button
                variant="ghost"
                onClick={() =>
                  void onAction(() =>
                    command("open_instance", { id: selected?.id }),
                  )
                }
              >
                <FolderOpen size={16} />
                Open folder
              </Button>
            </div>
          </div>
        )}
    </>
  );
}
