import { useEffect, useRef, useState } from "react";
import {
  ChevronLeft,
  ChevronRight,
  FolderOpen,
  Image,
  Maximize2,
  Minimize2,
  Trash2,
  X,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/dialog";
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
  const [viewerIndex, setViewerIndex] = useState<number | null>(null);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const lightboxRef = useRef<HTMLDivElement>(null);
  const previewsRef = useRef(previews);
  previewsRef.current = previews;
  const id = selectedId ?? instances[0]?.id;
  const active =
    viewerIndex !== null && shots[viewerIndex] ? shots[viewerIndex] : null;
  const activeSrc = active ? previews[active.name] : undefined;

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      if (!desktop || !id) {
        if (!cancelled) {
          revokeAll(previewsRef.current);
          setShots([]);
          setPreviews({});
          setViewerIndex(null);
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

  useEffect(() => {
    const onFullscreenChange = () => {
      setIsFullscreen(Boolean(document.fullscreenElement));
    };
    document.addEventListener("fullscreenchange", onFullscreenChange);
    return () =>
      document.removeEventListener("fullscreenchange", onFullscreenChange);
  }, []);

  useEffect(() => {
    if (viewerIndex === null) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "ArrowLeft") {
        e.preventDefault();
        setViewerIndex((i) =>
          i === null || shots.length === 0
            ? i
            : (i + shots.length - 1) % shots.length,
        );
      } else if (e.key === "ArrowRight") {
        e.preventDefault();
        setViewerIndex((i) =>
          i === null || shots.length === 0 ? i : (i + 1) % shots.length,
        );
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [viewerIndex, shots.length]);

  const refresh = async () => {
    if (!desktop || !id) {
      revokeAll(previewsRef.current);
      setShots([]);
      setPreviews({});
      setViewerIndex(null);
      return;
    }
    const next = await command<FileEntry[]>("list_screenshots", { id });
    const map = await previewMap(id, next);
    revokeAll(previewsRef.current);
    setShots(next);
    setPreviews(map);
  };

  const closeViewer = async () => {
    if (document.fullscreenElement) {
      try {
        await document.exitFullscreen();
      } catch {
        // Ignore fullscreen exit failures.
      }
    }
    setViewerIndex(null);
  };

  const toggleFullscreen = async () => {
    const node = lightboxRef.current;
    if (!node) return;
    try {
      if (document.fullscreenElement) {
        await document.exitFullscreen();
      } else {
        await node.requestFullscreen();
      }
    } catch {
      // Fullscreen may be blocked by the host; large dialog still works.
    }
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
          {shots.map((shot, index) => (
            <figure key={shot.name} className="shot-card">
              {previews[shot.name] ? (
                <button
                  type="button"
                  className="shot-thumb"
                  onClick={() => setViewerIndex(index)}
                  aria-label={`View ${shot.name}`}
                >
                  <img
                    src={previews[shot.name]}
                    alt={shot.name}
                    loading="lazy"
                  />
                </button>
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
                      if (viewerIndex === index) await closeViewer();
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

      <Dialog
        open={viewerIndex !== null && Boolean(activeSrc)}
        onOpenChange={(open) => {
          if (!open) void closeViewer();
        }}
      >
        <DialogContent
          showCloseButton={false}
          className="shot-lightbox-content border-0 bg-transparent p-0 shadow-none sm:max-w-none"
        >
          <div
            ref={lightboxRef}
            className={`shot-lightbox${isFullscreen ? " is-fullscreen" : ""}`}
          >
            <DialogTitle className="sr-only">
              {active?.name ?? "Screenshot"}
            </DialogTitle>
            <DialogDescription className="sr-only">
              Full-size screenshot preview. Use arrow keys to browse, Escape to
              close.
            </DialogDescription>
            <div className="shot-lightbox-bar">
              <span className="shot-lightbox-name">{active?.name}</span>
              <div className="row">
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label={
                    isFullscreen ? "Exit fullscreen" : "Enter fullscreen"
                  }
                  onClick={() => void toggleFullscreen()}
                >
                  {isFullscreen ? (
                    <Minimize2 size={18} />
                  ) : (
                    <Maximize2 size={18} />
                  )}
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label="Close"
                  onClick={() => void closeViewer()}
                >
                  <X size={18} />
                </Button>
              </div>
            </div>
            <div className="shot-lightbox-stage">
              {shots.length > 1 && (
                <Button
                  variant="ghost"
                  size="icon"
                  className="shot-lightbox-nav prev"
                  aria-label="Previous screenshot"
                  onClick={() =>
                    setViewerIndex((i) =>
                      i === null
                        ? i
                        : (i + shots.length - 1) % shots.length,
                    )
                  }
                >
                  <ChevronLeft size={28} />
                </Button>
              )}
              {activeSrc && (
                <img
                  src={activeSrc}
                  alt={active?.name ?? "Screenshot"}
                  className="shot-lightbox-image"
                />
              )}
              {shots.length > 1 && (
                <Button
                  variant="ghost"
                  size="icon"
                  className="shot-lightbox-nav next"
                  aria-label="Next screenshot"
                  onClick={() =>
                    setViewerIndex((i) =>
                      i === null ? i : (i + 1) % shots.length,
                    )
                  }
                >
                  <ChevronRight size={28} />
                </Button>
              )}
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}
