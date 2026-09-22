import {
  ArrowRight,
  Box,
  Copy,
  FolderOpen,
  MoreHorizontal,
  Trash2,
  Wrench,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import {
  AlertDialog,
  AlertDialogTrigger,
  AlertDialogContent,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogCancel,
  AlertDialogAction,
} from "@/components/ui/alert-dialog";
import { command } from "@/lib/api";
import { loaderLabel, type Instance } from "@/lib/models";

export function InstancesPage({
  instances,
  selectedId,
  installed,
  active,
  desktop,
  onSelect,
  onEdit,
  onCreate,
  onRefresh,
  onAction,
  onRepair,
}: {
  instances: Instance[];
  selectedId?: string;
  installed: string[];
  active: boolean;
  desktop: boolean;
  onSelect: (id: string) => void;
  onEdit: (instance?: Instance) => void;
  onCreate: () => void;
  onRefresh: () => Promise<void>;
  onAction: (task: () => Promise<unknown>) => Promise<void>;
  onRepair: (id: string) => void;
}) {
  return (
    <div className="instance-grid">
      {!instances.length && (
        <div className="empty-state">
          <Box size={36} />
          <h2>Create your first instance</h2>
          <p>
            Each instance is an isolated Minecraft install with its own worlds,
            settings, and Java configuration.
          </p>
          <Button onClick={onCreate} disabled={!desktop}>
            Create instance
          </Button>
        </div>
      )}
      {instances.map((i) => (
        <Card key={i.id} className="instance-card">
          <CardContent>
            <div className="row spread">
              <div className="instance-icon">
                <Box size={26} />
              </div>
              {selectedId === i.id && (
                <Badge variant="secondary">Selected</Badge>
              )}
            </div>
            <h2>{i.name}</h2>
            <p>
              Minecraft {i.minecraftVersion} · {loaderLabel(i)}
            </p>
            <div className="instance-meta">
              <span>{i.memory.maximumMb / 1024} GB RAM</span>
              <span>
                {installed.includes(i.id) ? "Installed" : "Not installed"}
              </span>
            </div>
            <div className="row">
              <Button
                variant="secondary"
                onClick={() => onSelect(i.id)}
              >
                Select
                <ArrowRight size={15} />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                aria-label="Edit instance"
                disabled={active}
                onClick={() => onEdit(i)}
              >
                <MoreHorizontal size={18} />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                aria-label="Clone instance"
                disabled={active}
                onClick={() =>
                  void onAction(async () => {
                    await command("clone_instance", { id: i.id });
                    await onRefresh();
                  })
                }
              >
                <Copy size={16} />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                aria-label="Repair instance files"
                disabled={active || !desktop}
                onClick={() => onRepair(i.id)}
              >
                <Wrench size={16} />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                aria-label="Open instance folder"
                onClick={() =>
                  void onAction(() => command("open_instance", { id: i.id }))
                }
              >
                <FolderOpen size={16} />
              </Button>
              <Button
                variant="ghost"
                size="sm"
                disabled={active || !desktop}
                onClick={() =>
                  void onAction(async () => {
                    const n = await command<number>("import_vanilla_worlds", {
                      id: i.id,
                      names: null,
                    });
                    if (n === 0) {
                      throw new Error(
                        "No new worlds imported from AppData/.minecraft/saves.",
                      );
                    }
                    await onRefresh();
                  })
                }
              >
                Import worlds
              </Button>
              <AlertDialog>
                <AlertDialogTrigger asChild>
                  <Button
                    variant="ghost"
                    size="icon"
                    aria-label="Delete instance"
                    disabled={active}
                  >
                    <Trash2 size={16} />
                  </Button>
                </AlertDialogTrigger>
                <AlertDialogContent>
                  <AlertDialogHeader>
                    <AlertDialogTitle>Delete {i.name}?</AlertDialogTitle>
                    <AlertDialogDescription>
                      This permanently deletes this instance, including its
                      worlds, screenshots, and configuration. Back up any worlds
                      you want to keep.
                    </AlertDialogDescription>
                  </AlertDialogHeader>
                  <AlertDialogFooter>
                    <AlertDialogCancel>Keep instance</AlertDialogCancel>
                    <AlertDialogAction
                      onClick={() =>
                        void onAction(async () => {
                          await command("delete_instance", {
                            id: i.id,
                            confirmed: true,
                          });
                          await onRefresh();
                        })
                      }
                    >
                      Delete instance
                    </AlertDialogAction>
                  </AlertDialogFooter>
                </AlertDialogContent>
              </AlertDialog>
            </div>
          </CardContent>
        </Card>
      ))}
    </div>
  );
}
