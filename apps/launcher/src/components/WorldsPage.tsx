import { useEffect, useMemo, useState } from "react";
import { FolderOpen, Globe2, Download } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { command } from "@/lib/api";
import type { Instance } from "@/lib/models";

type WorldEntry = {
  name: string;
  path: string;
  size: number;
  modified: number | null;
};

export function WorldsPage({
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
  const id = selectedId ?? instances[0]?.id;
  const instance = useMemo(
    () => instances.find((i) => i.id === id),
    [instances, id],
  );
  const [instanceWorlds, setInstanceWorlds] = useState<WorldEntry[]>([]);
  const [vanillaWorlds, setVanillaWorlds] = useState<WorldEntry[]>([]);
  const [loading, setLoading] = useState(false);

  const refresh = async (instanceId: string) => {
    setLoading(true);
    try {
      const [local, vanilla] = await Promise.all([
        command<WorldEntry[]>("list_instance_worlds", { id: instanceId }),
        command<WorldEntry[]>("list_vanilla_worlds"),
      ]);
      setInstanceWorlds(local);
      setVanillaWorlds(vanilla);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (!desktop || !id) {
      setInstanceWorlds([]);
      setVanillaWorlds([]);
      return;
    }
    void onAction(() => refresh(id));
  }, [desktop, id]);

  const importedNames = new Set(instanceWorlds.map((w) => w.name));
  const pending = vanillaWorlds.filter((w) => !importedNames.has(w.name));

  if (!instance) {
    return (
      <div className="empty-state">
        <Globe2 size={36} />
        <h2>Create an instance first</h2>
        <p>Worlds from AppData/.minecraft/saves can be imported into an instance.</p>
      </div>
    );
  }

  return (
    <div className="stack">
      <div className="row spread">
        <div className="row">
          <Select value={instance.id} onValueChange={onSelectInstance}>
            <SelectTrigger className="w-56">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {instances.map((i) => (
                <SelectItem key={i.id} value={i.id}>
                  {i.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Button
            variant="outline"
            disabled={!desktop || loading}
            onClick={() => void onAction(() => refresh(instance.id))}
          >
            Refresh
          </Button>
          <Button
            variant="outline"
            disabled={!desktop}
            onClick={() =>
              void onAction(() =>
                command("open_instance_folder", {
                  id: instance.id,
                  folder: "saves",
                }),
              )
            }
          >
            <FolderOpen size={16} />
            Open saves
          </Button>
        </div>
        <Button
          disabled={!desktop || !pending.length || loading}
          onClick={() =>
            void onAction(async () => {
              const n = await command<number>("import_vanilla_worlds", {
                id: instance.id,
                names: null,
              });
              await refresh(instance.id);
              if (n === 0) {
                throw new Error(
                  "No new worlds to import (already copied, or none found).",
                );
              }
            })
          }
        >
          <Download size={16} />
          Import all from .minecraft
          {pending.length ? ` (${pending.length})` : ""}
        </Button>
      </div>
      <p>
        Official launcher saves:{" "}
        <code>%AppData%\.minecraft\saves</code>. Import copies them into this
        instance (originals stay untouched). Play also auto-imports when this
        instance has no worlds yet.
      </p>

      <h3>In this instance</h3>
      {!instanceWorlds.length ? (
        <div className="empty-state">
          <Globe2 size={34} />
          <h2>No worlds in this instance</h2>
          <p>
            Import from AppData below, or create a world in Minecraft after
            pressing Play.
          </p>
        </div>
      ) : (
        <div className="file-list">
          {instanceWorlds.map((w) => (
            <Card key={w.name}>
              <CardContent className="file-row">
                <Globe2 size={20} />
                <div className="grow">
                  <strong>{w.name}</strong>
                  <p className="break-all">{w.path}</p>
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      )}

      <h3>AppData /.minecraft/saves</h3>
      {!vanillaWorlds.length ? (
        <div className="empty-state">
          <Globe2 size={34} />
          <h2>No AppData worlds found</h2>
          <p>
            Expected folders with <code>level.dat</code> under{" "}
            <code>%AppData%\.minecraft\saves</code>.
          </p>
        </div>
      ) : (
        <div className="file-list">
          {vanillaWorlds.map((w) => {
            const already = importedNames.has(w.name);
            return (
              <Card key={w.name}>
                <CardContent className="file-row">
                  <Globe2 size={20} />
                  <div className="grow">
                    <strong>{w.name}</strong>
                    <p>
                      {already
                        ? "Already in this instance"
                        : "Available to import"}
                    </p>
                  </div>
                  <Button
                    size="sm"
                    variant={already ? "outline" : "secondary"}
                    disabled={!desktop || already || loading}
                    onClick={() =>
                      void onAction(async () => {
                        await command("import_vanilla_worlds", {
                          id: instance.id,
                          names: [w.name],
                        });
                        await refresh(instance.id);
                      })
                    }
                  >
                    {already ? "Imported" : "Import"}
                  </Button>
                </CardContent>
              </Card>
            );
          })}
        </div>
      )}
    </div>
  );
}
