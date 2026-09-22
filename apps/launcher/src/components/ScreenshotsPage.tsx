import { useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { FolderOpen, Image, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { command } from "@/lib/api";
import type { Instance } from "@/lib/models";

type FileEntry = {
  name: string;
  path: string;
  size: number;
  modified: number | null;
};

export function ScreenshotsPage({
  instances,
  selectedId,
  desktop,
  onAction,
  onSelectInstance,
}: {
  instances: Instance[];
  selectedId?: string;
  desktop: boolean;
  onAction: (task: () => Promise<unknown>) => Promise<void>;
  onSelectInstance: (id: string) => void;
}) {
  const [shots, setShots] = useState<FileEntry[]>([]);
  const id = selectedId ?? instances[0]?.id;

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      if (!desktop || !id) {
        if (!cancelled) setShots([]);
        return;
      }
      try {
        const next = await command<FileEntry[]>("list_screenshots", { id });
        if (!cancelled) setShots(next);
      } catch {
        if (!cancelled) setShots([]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [id, desktop]);

  const refresh = async () => {
    if (!desktop || !id) {
      setShots([]);
      return;
    }
    setShots(await command<FileEntry[]>("list_screenshots", { id }));
  };

  if (!id) {
    return (
      <div className="empty-state">
        <Image size={36} />
        <h2>Create an instance first</h2>
        <p>In-game screenshots (F2) appear here for the selected instance.</p>
      </div>
    );
  }

  return (
    <div className="stack">
      <div className="section-heading">
        <div>
          <h2>Screenshots</h2>
          <p>Captured from Minecraft into this instance’s screenshots folder.</p>
        </div>
        <div className="row">
          <select
            className="instance-picker"
            value={id}
            onChange={(e) => onSelectInstance(e.target.value)}
            aria-label="Instance"
          >
            {instances.map((i) => (
              <option key={i.id} value={i.id}>
                {i.name}
              </option>
            ))}
          </select>
          <Button
            variant="outline"
            onClick={() =>
              void onAction(() =>
                command("open_instance_folder", {
                  id,
                  folder: "screenshots",
                }),
              )
            }
          >
            <FolderOpen size={16} />
            Open folder
          </Button>
        </div>
      </div>
      {!shots.length ? (
        <div className="empty-state">
          <Image size={34} />
          <h2>No screenshots yet</h2>
          <p>Press F2 in Minecraft to capture one.</p>
        </div>
      ) : (
        <div className="shot-grid">
          {shots.map((shot) => (
            <figure key={shot.name} className="shot-card">
              <img
                src={desktop ? convertFileSrc(shot.path) : ""}
                alt={shot.name}
                loading="lazy"
              />
              <figcaption>
                <span>{shot.name}</span>
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label={`Delete ${shot.name}`}
                  onClick={() =>
                    void onAction(async () => {
                      await command("delete_screenshot", {
                        id,
                        name: shot.name,
                      });
                      await refresh();
                    })
                  }
                >
                  <Trash2 size={15} />
                </Button>
              </figcaption>
            </figure>
          ))}
        </div>
      )}
    </div>
  );
}
