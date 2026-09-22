import { useEffect, useState } from "react";
import { FolderOpen, Package, Plus, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { command } from "@/lib/api";
import type { Instance } from "@/lib/models";

type FileEntry = {
  name: string;
  path: string;
  size: number;
  modified: number | null;
};

export function ModsPage({
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
  const [mods, setMods] = useState<FileEntry[]>([]);
  const id = selectedId ?? instances[0]?.id;

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      if (!desktop || !id) {
        if (!cancelled) setMods([]);
        return;
      }
      try {
        const next = await command<FileEntry[]>("list_mods", { id });
        if (!cancelled) setMods(next);
      } catch {
        if (!cancelled) setMods([]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [id, desktop]);

  const refresh = async () => {
    if (!desktop || !id) {
      setMods([]);
      return;
    }
    setMods(await command<FileEntry[]>("list_mods", { id }));
  };

  if (!id) {
    return (
      <div className="empty-state">
        <Package size={36} />
        <h2>Create an instance first</h2>
        <p>Mods are stored per instance in its mods folder.</p>
      </div>
    );
  }

  return (
    <div className="stack">
      <div className="section-heading">
        <div>
          <h2>Mods</h2>
          <p>
            Drop Fabric/Forge jars into this instance. A matching loader must be
            installed in the game for them to load.
          </p>
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
                command("open_instance_folder", { id, folder: "mods" }),
              )
            }
          >
            <FolderOpen size={16} />
            Open folder
          </Button>
          <Button
            disabled={!desktop}
            onClick={() =>
              void onAction(async () => {
                await command("add_mod", { id });
                await refresh();
              })
            }
          >
            <Plus size={16} />
            Add mod
          </Button>
        </div>
      </div>
      {!mods.length ? (
        <div className="empty-state">
          <Package size={34} />
          <h2>No mods in this instance</h2>
          <p>Add .jar or .zip files to get started.</p>
        </div>
      ) : (
        <div className="file-list">
          {mods.map((mod) => (
            <Card key={mod.name}>
              <CardContent className="file-row">
                <Package size={18} />
                <div className="grow">
                  <strong>{mod.name}</strong>
                  <p>{(mod.size / 1048576).toFixed(2)} MB</p>
                </div>
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label={`Remove ${mod.name}`}
                  onClick={() =>
                    void onAction(async () => {
                      await command("remove_mod", { id, name: mod.name });
                      await refresh();
                    })
                  }
                >
                  <Trash2 size={16} />
                </Button>
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}
