import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { command, message } from "@/lib/api";
import type { Settings, Runtime } from "@/lib/models";
import { UpdatePanel } from "./UpdatePanel";
const sections = [
  "General",
  "Minecraft",
  "Java",
  "Performance",
  "Appearance",
  "Downloads",
  "Updates",
  "Accounts",
  "Privacy",
  "Advanced",
  "About",
];
export function SettingsPanel({
  settings,
  onSave,
  onError,
  onAccounts,
  onEdit,
  initialSection = "General",
}: {
  settings: Settings;
  onSave: (s: Settings) => Promise<void>;
  onError: (s: string) => void;
  onAccounts: () => void;
  onEdit: () => void;
  initialSection?: string;
}) {
  const [draft, setDraft] = useState(settings);
  const [section, setSection] = useState(initialSection);
  const [runtimes, setRuntimes] = useState<Runtime[]>([]);
  const [saved, setSaved] = useState(false);
  useEffect(() => setDraft(settings), [settings]);
  useEffect(() => setSection(initialSection), [initialSection]);
  const update = <K extends keyof Settings>(key: K, value: Settings[K]) => {
    setDraft({ ...draft, [key]: value });
    setSaved(false);
  };
  return (
    <div className="settings-layout">
      <div className="settings-nav">
        {sections.map((s) => (
          <Button
            key={s}
            variant={s === section ? "secondary" : "ghost"}
            onClick={() => setSection(s)}
          >
            {s}
          </Button>
        ))}
      </div>
      <div className="stack grow">
        <Card>
          <CardHeader>
            <CardTitle>{section}</CardTitle>
          </CardHeader>
          <CardContent className="stack">
            {section === "Updates" && <UpdatePanel onError={onError} />}
            {section === "General" && (
              <>
                <div className="setting-row">
                  <div>
                    <Label htmlFor="minimize">
                      Minimize when Minecraft starts
                    </Label>
                    <p>Keep process tracking active in the background.</p>
                  </div>
                  <Switch
                    id="minimize"
                    checked={draft.minimizeOnLaunch}
                    onCheckedChange={(v) => update("minimizeOnLaunch", v)}
                  />
                </div>
                <div className="setting-row">
                  <Label htmlFor="remember">Remember selected instance</Label>
                  <Switch
                    id="remember"
                    checked={draft.rememberInstance}
                    onCheckedChange={(v) => update("rememberInstance", v)}
                  />
                </div>
              </>
            )}
            {section === "Minecraft" && (
              <>
                <Label>Game resolution</Label>
                <div className="row">
                  <Input
                    aria-label="Resolution width"
                    type="number"
                    min={640}
                    max={7680}
                    value={draft.width}
                    onChange={(e) => update("width", Number(e.target.value))}
                  />
                  <span>×</span>
                  <Input
                    aria-label="Resolution height"
                    type="number"
                    min={480}
                    max={4320}
                    value={draft.height}
                    onChange={(e) => update("height", Number(e.target.value))}
                  />
                </div>
                <div className="setting-row">
                  <Label htmlFor="fullscreen">Fullscreen</Label>
                  <Switch
                    id="fullscreen"
                    checked={draft.fullscreen}
                    onCheckedChange={(v) => update("fullscreen", v)}
                  />
                </div>
                <Button variant="outline" onClick={onEdit}>
                  Instance memory and Java
                </Button>
              </>
            )}
            {section === "Java" && (
              <>
                <p>
                  NodeClient reads each version’s Java requirement from official
                  metadata. Automatic mode detects a compatible runtime or
                  installs one from Mojang on Windows x64.
                </p>
                <Button
                  variant="outline"
                  onClick={() => {
                    void command<Runtime[]>("java_runtimes")
                      .then(setRuntimes)
                      .catch((e) => onError(message(e)));
                  }}
                >
                  Detect installed runtimes
                </Button>
                {runtimes.map((r) => (
                  <div key={r.path}>
                    <h3>Java {r.major}</h3>
                    <p className="break-all">{r.path}</p>
                  </div>
                ))}
                <Button variant="outline" onClick={onEdit}>
                  Choose custom Java for this instance
                </Button>
              </>
            )}
            {section === "Performance" && (
              <>
                <p>
                  Memory limits are saved per instance. The default uses at most
                  one third of system RAM, up to 4 GB.
                </p>
                <Button onClick={onEdit}>Configure instance memory</Button>
              </>
            )}
            {section === "Appearance" && (
              <>
                <Label>Theme</Label>
                <Select
                  value={draft.theme}
                  onValueChange={(v) => update("theme", v)}
                >
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {["dark", "light", "system"].map((v) => (
                      <SelectItem key={v} value={v}>
                        {v[0].toUpperCase() + v.slice(1)}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </>
            )}
            {section === "Downloads" && (
              <>
                <Label htmlFor="concurrency">Parallel downloads</Label>
                <Input
                  id="concurrency"
                  type="number"
                  min={1}
                  max={16}
                  value={draft.concurrency}
                  onChange={(e) =>
                    update("concurrency", Number(e.target.value))
                  }
                />
                <p>
                  Files are checked against Mojang’s hashes before installation.
                  Failed downloads retry up to three times.
                </p>
              </>
            )}
            {section === "Accounts" && (
              <>
                <p>
                  Manage Microsoft accounts and select the Minecraft profile to
                  play with.
                </p>
                <Button onClick={onAccounts}>Manage accounts</Button>
              </>
            )}
            {section === "Privacy" && (
              <>
                <h3>Your launcher stays local.</h3>
                <p>
                  NodeClient has no telemetry, analytics, or required backend.
                  Only Microsoft, Xbox, and Minecraft services are used for
                  authentication and downloads.
                </p>
                <p>
                  Refresh tokens are stored in OS-protected credential storage.
                  Profiles, instances, and settings stay on this computer.
                </p>
              </>
            )}
            {section === "Advanced" && (
              <>
                <Label>Custom JVM tuning arguments</Label>
                <p>
                  Incorrect JVM arguments can prevent Minecraft from starting.
                  One argument per line. Supported: -XX:+UseG1GC, -XX:+UseZGC,
                  -XX:+UseStringDeduplication, -XX:+AlwaysPreTouch.
                </p>
                <textarea
                  className="text-area"
                  aria-label="JVM arguments"
                  value={draft.jvmArguments.join("\n")}
                  onChange={(e) =>
                    update(
                      "jvmArguments",
                      e.target.value.split("\n").filter(Boolean),
                    )
                  }
                />
              </>
            )}
            {section === "About" && (
              <>
                <h3>
                  NodeClient <span className="mono">0.1.1</span>
                </h3>
                <p>An independent, original Minecraft Java Edition launcher.</p>
                <p>
                  Not an official Minecraft product. Not approved by or
                  associated with Mojang or Microsoft.
                </p>
                <p>Vanilla milestone · Tauri 2 / Rust / React</p>
              </>
            )}
          </CardContent>
        </Card>
        <div className="row">
          <Button
            onClick={() => {
              void onSave(draft)
                .then(() => setSaved(true))
                .catch((e) => onError(message(e)));
            }}
          >
            Save settings
          </Button>
          {saved && <span className="verified">Settings saved</span>}
        </div>
      </div>
    </div>
  );
}
