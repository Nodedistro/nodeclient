import { useEffect, useRef, useState } from "react";
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

function mimeFor(name: string) {
  const lower = name.toLowerCase();
  if (lower.endsWith(".jpg") || lower.endsWith(".jpeg")) return "image/jpeg";
  return "image/png";
}

async function previewMap(id: string, shots: FileEntry[]) {
  const map: Record<string, string> = {};
  await Promise.all(
    shots.map(async (shot) => {
      try {
        const bytes = await command<number[]>("read_screenshot", {
          id,
          name: shot.name,
        });
        map[shot.name] = URL.createObjectURL(
          new Blob([new Uint8Array(bytes)], { type: mimeFor(shot.name) }),
        );
      } catch {
        // Preview stays empty; card still lists the file.
      }
    }),
  );
  return map;
}

function revokeAll(urls: Record<string, string>) {
  Object.values(urls).forEach((url) => URL.revokeObjectURL(url));
}

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
  const [previews, setPreviews] = useState<Record<string, string>>({});
  const previewsRef = useRef(previews);
  previewsRef.current = previews;
  const id = selectedId ?? instances[0]?.id;

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      if (!desktop || !id) {
        if (!cancelled) {
          revokeAll(previewsRef.current);
          setShots([]);
          setPreviews({});
        }
        return;
      }
      try {
        const next = await command<FileEntry[]>("list_screenshots", { id });
        if (cancelled) return;
        setShots(next);
        const map = await previewMap(id, next);
        if (cancelled) {
          revokeAll(map);
          return;
        }
        revokeAll(previewsRef.current);
        setPreviews(map);
      } catch {
        if (!cancelled) {
          revokeAll(previewsRef.current);
          setShots([]);
          setPreviews({});
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [id, desktop]);

  useEffect(() => {
    return () => revokeAll(previewsRef.current);
  }, []);

  const refresh = async () => {
    if (!desktop || !id) {
      revokeAll(previewsRef.current);
      setShots([]);
      setPreviews({});
      return;
    }
    const next = await command<FileEntry[]>("list_screenshots", { id });
    const map = await previewMap(id, next);
    revokeAll(previewsRef.current);
    setShots(next);
    setPreviews(map);
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
              {previews[shot.name] ? (
                <img src={previews[shot.name]} alt={shot.name} loading="lazy" />
              ) : (
                <div className="shot-card-fallback" aria-hidden>
                  <Image size={28} />
                </div>
              )}
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
