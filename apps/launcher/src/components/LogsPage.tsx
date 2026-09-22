import { Copy, FolderOpen, Sparkles } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { command } from "@/lib/api";

export function LogsPage({
  logKind,
  logText,
  logFilter,
  search,
  selectedId,
  onLogKind,
  onLogFilter,
  onSearch,
  onRefresh,
  onAction,
  onExplainCrash,
}: {
  logKind: string;
  logText: string;
  logFilter: string;
  search: string;
  selectedId?: string;
  onLogKind: (value: string) => void;
  onLogFilter: (value: string) => void;
  onSearch: (value: string) => void;
  onRefresh: () => void;
  onAction: (task: () => Promise<unknown>) => Promise<void>;
  onExplainCrash: () => void;
}) {
  return (
    <div className="stack">
      <div className="row spread">
        <Tabs value={logKind} onValueChange={onLogKind}>
          <TabsList>
            {["NodeClient", "Minecraft", "Crash Reports"].map((s) => (
              <TabsTrigger key={s} value={s}>
                {s}
              </TabsTrigger>
            ))}
          </TabsList>
        </Tabs>
        <div className="row">
          <Button
            variant="secondary"
            disabled={!selectedId}
            onClick={onExplainCrash}
          >
            <Sparkles size={15} />
            Explain crash
          </Button>
          <Button variant="outline" onClick={onRefresh}>
            Refresh
          </Button>
          <Button
            variant="outline"
            onClick={() =>
              void onAction(() => navigator.clipboard.writeText(logText))
            }
          >
            <Copy size={15} />
            Copy
          </Button>
          {selectedId && (
            <Button
              variant="outline"
              onClick={() =>
                void onAction(() =>
                  command("open_instance", { id: selectedId }),
                )
              }
            >
              <FolderOpen size={15} />
              Open folder
            </Button>
          )}
        </div>
      </div>
      <div className="row">
        <Input
          aria-label="Search logs"
          placeholder="Search output…"
          value={search}
          onChange={(e) => onSearch(e.target.value)}
        />
        <Select value={logFilter} onValueChange={onLogFilter}>
          <SelectTrigger className="w-40">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {["All", "Errors", "Warnings"].map((s) => (
              <SelectItem key={s} value={s}>
                {s}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      <pre className="log-output">
        {logText
          .split("\n")
          .filter(
            (l) =>
              l.toLowerCase().includes(search.toLowerCase()) &&
              (logFilter === "All" ||
                (logFilter === "Errors" &&
                  /error|exception|fatal/i.test(l)) ||
                (logFilter === "Warnings" && /warn/i.test(l))),
          )
          .join("\n") || "No matching log entries."}
      </pre>
      <p>Credentials are redacted before display and copying.</p>
    </div>
  );
}
