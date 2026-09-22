import { ArrowRight, Check, Layers, Search } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import type { Manifest } from "@/lib/models";

export function VersionsPage({
  manifest,
  search,
  selectedVersion,
  active,
  onSearch,
  onRefresh,
  onSelectVersion,
  onAction,
}: {
  manifest: Manifest | null;
  search: string;
  selectedVersion?: string;
  active: boolean;
  onSearch: (value: string) => void;
  onRefresh: () => void;
  onSelectVersion: (version: string) => Promise<void>;
  onAction: (task: () => Promise<unknown>) => Promise<void>;
}) {
  return (
    <div className="stack">
      <div className="version-highlights">
        {(["release", "snapshot"] as const).map((kind) => (
          <Card key={kind}>
            <CardContent>
              <Badge variant="secondary">Latest {kind}</Badge>
              <h2>{manifest?.latest[kind] ?? "Unavailable"}</h2>
              <p>
                {kind === "release"
                  ? "Stable Minecraft for long-term worlds."
                  : "Preview builds from Mojang."}
              </p>
              <Button
                variant="outline"
                disabled={!manifest || active}
                onClick={() =>
                  void onAction(() => onSelectVersion(manifest!.latest[kind]))
                }
              >
                Use this version
                <ArrowRight size={16} />
              </Button>
            </CardContent>
          </Card>
        ))}
      </div>
      <div className="section-heading">
        <h2>All versions</h2>
        <div className="row">
          <Search size={17} />
          <Input
            aria-label="Search versions"
            placeholder="Find a version…"
            value={search}
            onChange={(e) => onSearch(e.target.value)}
          />
          <Button variant="outline" onClick={onRefresh}>
            Refresh
          </Button>
        </div>
      </div>
      <div className="version-list">
        {manifest?.versions
          .filter((v) => v.id.toLowerCase().includes(search.toLowerCase()))
          .map((v) => (
            <div className="version-row" key={v.id}>
              <Layers size={17} />
              <strong>{v.id}</strong>
              <Badge variant="outline">{v.type}</Badge>
              <span>{new Date(v.releaseTime).toLocaleDateString()}</span>
              <Button
                size="sm"
                variant="ghost"
                disabled={active}
                onClick={() => void onAction(() => onSelectVersion(v.id))}
              >
                {selectedVersion === v.id ? <Check size={16} /> : "Select"}
              </Button>
            </div>
          ))}
        {!manifest && (
          <p style={{ padding: "16px" }}>
            Load the official Mojang manifest to browse versions.
          </p>
        )}
      </div>
    </div>
  );
}
