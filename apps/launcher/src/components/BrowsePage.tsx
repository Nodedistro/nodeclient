import { Button } from "@/components/ui/button";
import { ModsPage } from "@/components/ModsPage";
import { VersionsPage } from "@/components/VersionsPage";
import type { Instance, Manifest } from "@/lib/models";

const tabs = ["Versions", "Mods"] as const;
export type BrowseTab = (typeof tabs)[number];

export function BrowsePage({
  tab,
  onTab,
  manifest,
  search,
  selected,
  active,
  desktop,
  instances,
  onSearch,
  onRefreshVersions,
  onSelectVersion,
  onSelectInstance,
  onAction,
}: {
  tab: BrowseTab;
  onTab: (tab: BrowseTab) => void;
  manifest: Manifest | null;
  search: string;
  selected?: Instance;
  active: boolean;
  desktop: boolean;
  instances: Instance[];
  onSearch: (value: string) => void;
  onRefreshVersions: () => void;
  onSelectVersion: (version: string) => Promise<void>;
  onSelectInstance: (id: string) => void;
  onAction: (task: () => Promise<unknown>) => Promise<void>;
}) {
  return (
    <div className="stack hub-page">
      <div className="hub-tabs" role="tablist" aria-label="Browse">
        {tabs.map((name) => (
          <Button
            key={name}
            role="tab"
            aria-selected={tab === name}
            variant={tab === name ? "secondary" : "outline"}
            onClick={() => onTab(name)}
          >
            {name === "Mods" ? "Mods (Modrinth)" : name}
          </Button>
        ))}
      </div>
      {tab === "Versions" && (
        <VersionsPage
          manifest={manifest}
          search={search}
          selectedVersion={selected?.minecraftVersion}
          active={active}
          onSearch={onSearch}
          onRefresh={onRefreshVersions}
          onSelectVersion={onSelectVersion}
          onAction={onAction}
        />
      )}
      {tab === "Mods" && (
        <ModsPage
          instances={instances}
          selectedId={selected?.id}
          desktop={desktop}
          initialTab="browse"
          onAction={onAction}
          onSelectInstance={onSelectInstance}
        />
      )}
    </div>
  );
}
