import { useEffect, useMemo, useState } from "react";
import {
  ArrowUpCircle,
  FolderOpen,
  Package,
  Plus,
  Search,
  Trash2,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { command } from "@/lib/api";
import { loaderLabel, type Instance } from "@/lib/models";

type ModEntry = {
  name: string;
  path: string;
  size: number;
  modified: number | null;
  enabled: boolean;
  sha1: string | null;
};

type SearchHit = {
  projectId: string;
  slug: string;
  title: string;
  description: string;
  downloads: number;
  iconUrl: string | null;
  categories: string[];
};

type SearchResult = {
  hits: SearchHit[];
  totalHits: number;
};

export function ModsPage({
  instances,
  selectedId,
  desktop,
  onAction,
  onSelectInstance,
  initialTab = "installed",
}: {
  instances: Instance[];
  selectedId?: string;
  desktop: boolean;
  onAction: (task: () => Promise<unknown>) => Promise<void>;
  onSelectInstance: (id: string) => void;
  initialTab?: "installed" | "browse";
}) {
  const [mods, setMods] = useState<ModEntry[]>([]);
  const [tab, setTab] = useState<"installed" | "browse">(initialTab);
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [totalHits, setTotalHits] = useState(0);
  const [searching, setSearching] = useState(false);
  const id = selectedId ?? instances[0]?.id;
  const instance = useMemo(
    () => instances.find((i) => i.id === id),
    [instances, id],
  );

  useEffect(() => {
    setTab(initialTab);
  }, [initialTab]);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      if (!desktop || !id) {
        if (!cancelled) setMods([]);
        return;
      }
      try {
        const next = await command<ModEntry[]>("list_mods", { id });
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
    setMods(await command<ModEntry[]>("list_mods", { id }));
  };

  const runSearch = async () => {
    if (!desktop || !instance) return;
    setSearching(true);
    try {
      const result = await command<SearchResult>("modrinth_search", {
        query,
        gameVersion: instance.minecraftVersion,
        loader: instance.loader.type,
        limit: 24,
        offset: 0,
      });
      setHits(result.hits);
      setTotalHits(result.totalHits);
    } finally {
      setSearching(false);
    }
  };

  if (!id || !instance) {
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
            {instance.name} · Minecraft {instance.minecraftVersion} ·{" "}
            {loaderLabel(instance)}
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
            Add file
          </Button>
        </div>
      </div>

      <div className="row mods-tabs">
        <Button
          variant={tab === "installed" ? "secondary" : "outline"}
          onClick={() => setTab("installed")}
        >
          Installed ({mods.length})
        </Button>
        <Button
          variant={tab === "browse" ? "secondary" : "outline"}
          onClick={() => setTab("browse")}
        >
          Browse Modrinth
        </Button>
      </div>

      {tab === "installed" ? (
        !mods.length ? (
          <div className="empty-state">
            <Package size={34} />
            <h2>No mods in this instance</h2>
            <p>Browse Modrinth or add a local .jar / .zip.</p>
          </div>
        ) : (
          <div className="file-list">
            {mods.map((mod) => (
              <Card key={`${mod.name}:${mod.enabled ? "on" : "off"}`}>
                <CardContent className="file-row">
                  <Package size={18} />
                  <div className="grow">
                    <strong className={mod.enabled ? undefined : "mod-disabled"}>
                      {mod.name}
                    </strong>
                    <p>
                      {(mod.size / 1048576).toFixed(2)} MB
                      {!mod.enabled ? " · disabled" : ""}
                    </p>
                  </div>
                  <div className="row mod-actions">
                    <Switch
                      checked={mod.enabled}
                      aria-label={
                        mod.enabled ? `Disable ${mod.name}` : `Enable ${mod.name}`
                      }
                      onCheckedChange={(value) => {
                        if (typeof value !== "boolean") return;
                        void onAction(async () => {
                          await command("set_mod_enabled", {
                            id,
                            name: mod.name,
                            enabled: value,
                          });
                          await refresh();
                        });
                      }}
                    />
                    {mod.enabled && mod.sha1 && (
                      <Button
                        variant="ghost"
                        size="icon"
                        aria-label={`Update ${mod.name}`}
                        onClick={() =>
                          void onAction(async () => {
                            await command("update_modrinth_mod", {
                              id,
                              name: mod.name,
                            });
                            await refresh();
                          })
                        }
                      >
                        <ArrowUpCircle size={16} />
                      </Button>
                    )}
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
                  </div>
                </CardContent>
              </Card>
            ))}
          </div>
        )
      ) : (
        <div className="stack">
          <div className="row mods-search">
            <Input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search Modrinth mods"
              onKeyDown={(e) => {
                if (e.key === "Enter") void onAction(runSearch);
              }}
            />
            <Button
              disabled={!desktop || searching}
              onClick={() => void onAction(runSearch)}
            >
              <Search size={16} />
              {searching ? "Searching…" : "Search"}
            </Button>
          </div>
          <p className="mods-search-meta">
            Filtered for Minecraft {instance.minecraftVersion}
            {instance.loader.type !== "vanilla"
              ? ` · ${loaderLabel({ loader: { type: instance.loader.type } })}`
              : " · any loader"}
            {totalHits ? ` · ${totalHits.toLocaleString()} results` : ""}
          </p>
          {!hits.length ? (
            <div className="empty-state">
              <Search size={34} />
              <h2>Search Modrinth</h2>
              <p>
                Results match this instance’s game version
                {instance.loader.type !== "vanilla"
                  ? ` and ${loaderLabel({ loader: { type: instance.loader.type } })}`
                  : ""}
                .
              </p>
            </div>
          ) : (
            <div className="file-list">
              {hits.map((hit) => (
                <Card key={hit.projectId}>
                  <CardContent className="file-row modrinth-hit">
                    {hit.iconUrl ? (
                      <img
                        src={hit.iconUrl}
                        alt=""
                        className="modrinth-icon"
                        width={40}
                        height={40}
                      />
                    ) : (
                      <Package size={22} />
                    )}
                    <div className="grow">
                      <strong>{hit.title}</strong>
                      <p>{hit.description}</p>
                      <p className="modrinth-meta">
                        {hit.downloads.toLocaleString()} downloads
                        {hit.categories.length
                          ? ` · ${hit.categories.slice(0, 3).join(", ")}`
                          : ""}
                      </p>
                    </div>
                    <Button
                      disabled={!desktop}
                      onClick={() =>
                        void onAction(async () => {
                          await command("install_modrinth_mod", {
                            id,
                            projectId: hit.projectId,
                          });
                          await refresh();
                          setTab("installed");
                        })
                      }
                    >
                      Install
                    </Button>
                  </CardContent>
                </Card>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
