import { useEffect, useState } from "react";
import { Plus, Server, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Card, CardContent } from "@/components/ui/card";
import { command } from "@/lib/api";
import type { Instance } from "@/lib/models";

type ServerEntry = { name: string; ip: string };

export function ServersPage({
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
  const [servers, setServers] = useState<ServerEntry[]>([]);
  const [name, setName] = useState("");
  const [ip, setIp] = useState("");
  const id = selectedId ?? instances[0]?.id;

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      if (!desktop || !id) {
        if (!cancelled) setServers([]);
        return;
      }
      try {
        const next = await command<ServerEntry[]>("list_servers", { id });
        if (!cancelled) setServers(next);
      } catch {
        if (!cancelled) setServers([]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [id, desktop]);

  const persist = async (next: ServerEntry[]) => {
    await command("save_servers", { id, servers: next });
    setServers(next);
  };

  if (!id) {
    return (
      <div className="empty-state">
        <Server size={36} />
        <h2>Create an instance first</h2>
        <p>Multiplayer servers are saved in that instance’s servers.dat.</p>
      </div>
    );
  }

  return (
    <div className="stack">
      <div className="section-heading">
        <div>
          <h2>Servers</h2>
          <p>
            Edit the Minecraft multiplayer list for this instance. Changes write
            to servers.dat.
          </p>
        </div>
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
      </div>
      <Card>
        <CardContent className="stack" style={{ paddingTop: 18 }}>
          <div className="row">
            <Input
              aria-label="Server name"
              placeholder="Server name"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
            <Input
              aria-label="Server address"
              placeholder="Address (play.example.com)"
              value={ip}
              onChange={(e) => setIp(e.target.value)}
            />
            <Button
              disabled={!desktop || !name.trim() || !ip.trim()}
              onClick={() =>
                void onAction(async () => {
                  await persist([
                    ...servers,
                    { name: name.trim(), ip: ip.trim() },
                  ]);
                  setName("");
                  setIp("");
                })
              }
            >
              <Plus size={16} />
              Add
            </Button>
          </div>
        </CardContent>
      </Card>
      {!servers.length ? (
        <div className="empty-state">
          <Server size={34} />
          <h2>No saved servers</h2>
          <p>Add an address to appear in Minecraft’s Multiplayer screen.</p>
        </div>
      ) : (
        <div className="file-list">
          {servers.map((server, index) => (
            <Card key={`${server.ip}-${index}`}>
              <CardContent className="file-row">
                <Server size={18} />
                <div className="grow">
                  <strong>{server.name}</strong>
                  <p className="mono">{server.ip}</p>
                </div>
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label={`Remove ${server.name}`}
                  onClick={() =>
                    void onAction(async () => {
                      await persist(servers.filter((_, i) => i !== index));
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
