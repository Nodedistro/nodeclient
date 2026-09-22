import { useEffect, useState } from "react";
import {
  Archive,
  FolderOpen,
  RotateCcw,
  ShieldAlert,
  Trash2,
  Wrench,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { command } from "@/lib/api";
import type { Instance } from "@/lib/models";

type BackupEntry = {
  name: string;
  path: string;
  size: number;
  modified: number | null;
};

export function BackupsPage({
  instances,
  selectedId,
  desktop,
  active,
  onAction,
  onSelectInstance,
  onRepair,
}: {
  instances: Instance[];
  selectedId?: string;
  desktop: boolean;
  active: boolean;
  onAction: (task: () => Promise<unknown>) => Promise<void>;
  onSelectInstance: (id: string) => void;
  onRepair: (id: string) => void;
}) {
  const id = selectedId ?? instances[0]?.id;
  const instance = instances.find((i) => i.id === id);
  const [backups, setBackups] = useState<BackupEntry[]>([]);

  async function refresh() {
    if (!desktop || !id) {
      setBackups([]);
      return;
    }
    setBackups(await command<BackupEntry[]>("list_backups", { id }));
  }

  useEffect(() => {
    void refresh().catch(() => setBackups([]));
  }, [desktop, id]);

  if (!instances.length) {
    return (
      <div className="empty-state">
        <Archive size={34} />
        <h2>No instances yet</h2>
        <p>Create an instance before backing up worlds and configs.</p>
      </div>
    );
  }

  return (
    <div className="stack">
      <div className="row spread">
        <Select
          value={id}
          onValueChange={onSelectInstance}
        >
          <SelectTrigger className="w-56">
            <SelectValue placeholder="Instance" />
          </SelectTrigger>
          <SelectContent>
            {instances.map((i) => (
              <SelectItem key={i.id} value={i.id}>
                {i.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <div className="row">
          <Button
            variant="outline"
            disabled={!desktop || !id || active}
            onClick={() =>
              void onAction(async () => {
                await command("create_backup", { id });
                await refresh();
              })
            }
          >
            <Archive size={16} />
            Back up now
          </Button>
          <Button
            variant="outline"
            disabled={!desktop || !id || active}
            onClick={() => id && onRepair(id)}
          >
            <Wrench size={16} />
            Repair files
          </Button>
          {id && (
            <Button
              variant="ghost"
              onClick={() =>
                void onAction(() => command("open_instance", { id }))
              }
            >
              <FolderOpen size={16} />
              Folder
            </Button>
          )}
        </div>
      </div>
      <p>
        Backups include worlds, <code>options.txt</code>, servers, and config
        for {instance?.name ?? "this instance"}. Mods are not included.
      </p>
      {!backups.length ? (
        <div className="empty-state">
          <ShieldAlert size={34} />
          <h2>No backups yet</h2>
          <p>Create a backup before big mod changes or Minecraft updates.</p>
        </div>
      ) : (
        <div className="file-list">
          {backups.map((b) => (
            <Card key={b.name}>
              <CardContent className="file-row">
                <Archive size={20} />
                <div className="grow">
                  <strong>{b.name}</strong>
                  <p>
                    {(b.size / 1048576).toFixed(1)} MB
                    {b.modified
                      ? ` · ${new Date(b.modified * 1000).toLocaleString()}`
                      : ""}
                  </p>
                </div>
                <AlertDialog>
                  <AlertDialogTrigger asChild>
                    <Button variant="outline" size="sm" disabled={active}>
                      <RotateCcw size={14} />
                      Restore
                    </Button>
                  </AlertDialogTrigger>
                  <AlertDialogContent>
                    <AlertDialogHeader>
                      <AlertDialogTitle>Restore {b.name}?</AlertDialogTitle>
                      <AlertDialogDescription>
                        This overwrites matching worlds and config files in the
                        instance. Close Minecraft first.
                      </AlertDialogDescription>
                    </AlertDialogHeader>
                    <AlertDialogFooter>
                      <AlertDialogCancel>Cancel</AlertDialogCancel>
                      <AlertDialogAction
                        onClick={() =>
                          void onAction(async () => {
                            await command("restore_backup", {
                              id,
                              name: b.name,
                            });
                          })
                        }
                      >
                        Restore backup
                      </AlertDialogAction>
                    </AlertDialogFooter>
                  </AlertDialogContent>
                </AlertDialog>
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label={`Delete ${b.name}`}
                  disabled={active}
                  onClick={() =>
                    void onAction(async () => {
                      await command("delete_backup", { id, name: b.name });
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
