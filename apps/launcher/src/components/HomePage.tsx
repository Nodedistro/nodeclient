import { useEffect, useState } from "react";
import {
  ArrowDownToLine,
  FolderOpen,
  LoaderCircle,
  Pencil,
  Play,
  Shield,
  Wrench,
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
import {
  loaderLabel,
  type Instance,
  type Progress,
  type Snapshot,
} from "@/lib/models";

type CrashExplanation = {
  title: string;
  summary: string;
  details: string[];
  actions: string[];
  reportName: string | null;
};

export function HomePage({
  data,
  selected,
  accountReady,
  installed,
  active,
  desktop,
  progress,
  playLabel,
  modCount,
  onChooseInstance,
  onPlay,
  onSafePlay,
  onEdit,
  onOpenLogs,
  onAction,
  onRefresh,
  onRepair,
}: {
  data: Snapshot;
  selected?: Instance;
  accountReady: boolean;
  installed: boolean;
  active: boolean;
  desktop: boolean;
  progress: Progress | null;
  playLabel: string;
  modCount: number | null;
  onChooseInstance: (id: string) => void;
  onPlay: () => void;
  onSafePlay: () => void;
  onEdit: () => void;
  onOpenLogs: (kind: string) => void;
  onAction: (task: () => Promise<unknown>) => Promise<void>;
  onRefresh: () => Promise<void>;
  onRepair: () => void;
}) {
  const [explanation, setExplanation] = useState<CrashExplanation | null>(
    null,
  );
  const summary = selected
    ? [
        modCount == null
          ? null
          : `${modCount} mod${modCount === 1 ? "" : "s"}`,
        selected.java.mode === "custom" ? "Custom Java" : "Automatic Java",
        `${selected.memory.maximumMb / 1024} GB RAM`,
      ]
        .filter(Boolean)
        .join(" · ")
    : null;

  const crashed =
    data.status.process?.exitCode != null &&
    data.status.process.exitCode !== 0;

  useEffect(() => {
    setExplanation(null);
  }, [selected?.id, data.status.process?.exitCode]);

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
          <div className="home-instance">
            {selected ? (
              <>
                <Select
                  value={selected.id}
                  onValueChange={onChooseInstance}
                >
                  <SelectTrigger className="home-instance-select">
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
                <h2 className="home-instance-title">
                  {selected.name}
                  <span>
                    {loaderLabel(selected)} · {selected.minecraftVersion}
                  </span>
                </h2>
                <p>{summary}</p>
              </>
            ) : (
              <h3>No instance yet</h3>
            )}
            {!selected && (
              <p>Create an instance to install and play.</p>
            )}
          </div>
          <div className="home-cta-row">
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
            {selected && (
              <>
                {installed && (
                  <Button
                    variant="outline"
                    disabled={active || !accountReady}
                    onClick={onSafePlay}
                    title="Launch with all mods temporarily disabled"
                  >
                    <Shield size={16} />
                    Safe mode
                  </Button>
                )}
                <Button
                  variant="outline"
                  disabled={active}
                  onClick={onEdit}
                >
                  <Pencil size={16} />
                  Edit
                </Button>
                <Button
                  variant="outline"
                  disabled={!desktop}
                  onClick={() =>
                    void onAction(() =>
                      command("open_instance", { id: selected.id }),
                    )
                  }
                >
                  <FolderOpen size={16} />
                  Folder
                </Button>
              </>
            )}
          </div>
          {selected && (
            <div className="row" style={{ flexWrap: "wrap", gap: 8 }}>
              <Button
                size="sm"
                variant="ghost"
                disabled={active || !desktop}
                onClick={onRepair}
              >
                <Wrench size={14} />
                Repair files
              </Button>
              <Button
                size="sm"
                variant="ghost"
                disabled={!desktop}
                onClick={() =>
                  void onAction(async () => {
                    const n = await command<number>("import_vanilla_worlds", {
                      id: selected.id,
                      names: null,
                    });
                    if (n === 0) {
                      throw new Error(
                        "No new worlds to import from AppData/.minecraft/saves (already imported, or none found).",
                      );
                    }
                  })
                }
              >
                Import .minecraft worlds
              </Button>
              <Button
                size="sm"
                variant="ghost"
                disabled={!desktop}
                onClick={() =>
                  void onAction(async () => {
                    await command("create_backup", { id: selected.id });
                  })
                }
              >
                Backup worlds
              </Button>
            </div>
          )}
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
      {crashed && (
        <div className="crash-notice">
          <strong>Minecraft appears to have crashed.</strong>
          {explanation && (
            <div className="stack" style={{ gap: 6 }}>
              <p>
                <strong>{explanation.title}</strong> — {explanation.summary}
              </p>
              {explanation.details.length > 0 && (
                <ul>
                  {explanation.details.map((d) => (
                    <li key={d}>{d}</li>
                  ))}
                </ul>
              )}
            </div>
          )}
          <div className="row" style={{ flexWrap: "wrap" }}>
            <Button
              variant="outline"
              onClick={() =>
                void onAction(async () => {
                  if (!selected) return;
                  setExplanation(
                    await command<CrashExplanation>("explain_crash", {
                      id: selected.id,
                    }),
                  );
                })
              }
            >
              Explain crash
            </Button>
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
              disabled={active || !accountReady}
              onClick={onSafePlay}
            >
              <Shield size={16} />
              Safe mode
            </Button>
            <Button
              variant="outline"
              disabled={active}
              onClick={onRepair}
            >
              <Wrench size={16} />
              Repair
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
                    `NodeClient\nMinecraft ${selected?.minecraftVersion}\nExit ${data.status.process?.exitCode}\n${text}`,
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

export function useModCount(
  instanceId: string | undefined,
  desktop: boolean,
): number | null {
  const [count, setCount] = useState<number | null>(null);
  useEffect(() => {
    let cancelled = false;
    if (!desktop || !instanceId) {
      setCount(null);
      return;
    }
    void command<{ name: string }[]>("list_mods", { id: instanceId })
      .then((mods) => {
        if (!cancelled) setCount(mods.length);
      })
      .catch(() => {
        if (!cancelled) setCount(null);
      });
    return () => {
      cancelled = true;
    };
  }, [desktop, instanceId]);
  return count;
}
