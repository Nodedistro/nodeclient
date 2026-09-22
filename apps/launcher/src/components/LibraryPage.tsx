import { Button } from "@/components/ui/button";
import { BackupsPage } from "@/components/BackupsPage";
import { InstancesPage } from "@/components/InstancesPage";
import { ModsPage } from "@/components/ModsPage";
import { ScreenshotsPage } from "@/components/ScreenshotsPage";
import { WorldsPage } from "@/components/WorldsPage";
import type { Instance } from "@/lib/models";

const tabs = ["Instances", "Worlds", "Mods", "Screenshots", "Backups"] as const;
export type LibraryTab = (typeof tabs)[number];

export function LibraryPage({
  tab,
  onTab,
  instances,
  selectedId,
  installed,
  active,
  desktop,
  onSelectInstance,
  onEdit,
  onCreate,
  onRefresh,
  onAction,
  onPlayInstance,
  onRepair,
}: {
  tab: LibraryTab;
  onTab: (tab: LibraryTab) => void;
  instances: Instance[];
  selectedId?: string;
  installed: string[];
  active: boolean;
  desktop: boolean;
  onSelectInstance: (id: string) => void;
  onEdit: (instance?: Instance) => void;
  onCreate: () => void;
  onRefresh: () => Promise<void>;
  onAction: (task: () => Promise<unknown>) => Promise<void>;
  onPlayInstance: (id: string) => void;
  onRepair: (id: string) => void;
}) {
  return (
    <div className="stack hub-page">
      <div className="hub-tabs" role="tablist" aria-label="Library">
        {tabs.map((name) => (
          <Button
            key={name}
            role="tab"
            aria-selected={tab === name}
            variant={tab === name ? "secondary" : "outline"}
            onClick={() => onTab(name)}
          >
            {name}
          </Button>
        ))}
      </div>
      {tab === "Instances" && (
        <InstancesPage
          instances={instances}
          selectedId={selectedId}
          installed={installed}
          active={active}
          desktop={desktop}
          onSelect={onPlayInstance}
          onEdit={onEdit}
          onCreate={onCreate}
          onRefresh={onRefresh}
          onAction={onAction}
          onRepair={onRepair}
        />
      )}
      {tab === "Worlds" && (
        <WorldsPage
          instances={instances}
          selectedId={selectedId}
          desktop={desktop}
          onAction={onAction}
          onSelectInstance={onSelectInstance}
        />
      )}
      {tab === "Mods" && (
        <ModsPage
          instances={instances}
          selectedId={selectedId}
          desktop={desktop}
          onAction={onAction}
          onSelectInstance={onSelectInstance}
        />
      )}
      {tab === "Screenshots" && (
        <ScreenshotsPage
          instances={instances}
          selectedId={selectedId}
          desktop={desktop}
          onAction={onAction}
          onSelectInstance={onSelectInstance}
        />
      )}
      {tab === "Backups" && (
        <BackupsPage
          instances={instances}
          selectedId={selectedId}
          desktop={desktop}
          active={active}
          onAction={onAction}
          onSelectInstance={onSelectInstance}
          onRepair={onRepair}
        />
      )}
    </div>
  );
}
