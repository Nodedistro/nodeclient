import { useEffect, useState } from "react";
import { APP_VERSION } from "@/lib/version";
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
  selectedInstanceId = null,
}: {
  settings: Settings;
  onSave: (s: Settings) => Promise<void>;
  onError: (s: string) => void;
  onAccounts: () => void;
  onEdit: () => void;
  initialSection?: string;
  selectedInstanceId?: string | null;
}) {
  const [draft, setDraft] = useState(settings);
  const [section, setSection] = useState(initialSection);
  const [runtimes, setRuntimes] = useState<Runtime[]>([]);
  const [saved, setSaved] = useState(false);
  const [busy, setBusy] = useState(false);
  const [perfNote, setPerfNote] = useState<string | null>(null);
  useEffect(() => setDraft(settings), [settings]);
  useEffect(() => setSection(initialSection), [initialSection]);
  const update = <K extends keyof Settings>(key: K, value: Settings[K]) => {
    setDraft({ ...draft, [key]: value });
    setSaved(false);
  };
  const applyJvmPreset = async (preset: string) => {
    try {
      const args = await command<string[]>("jvm_performance_preset", { preset });
      setDraft((d) => ({ ...d, jvmArguments: args }));
      setSaved(false);
      setPerfNote(
        preset === "default"
          ? "Cleared JVM tuning flags. Save settings to apply."
          : `Applied ${preset} JVM preset. Save settings to apply.`,
      );
    } catch (e) {
      onError(message(e));
    }
  };
  const applyVideo = async () => {
    if (!selectedInstanceId) {
      onError("Select an instance first.");
      return;
    }
    setBusy(true);
    setPerfNote(null);
    try {
      await command("apply_fps_video_settings", { id: selectedInstanceId });
      setPerfNote(
        "Patched options.txt: VSync off, Fast graphics, higher FPS cap.",
      );
    } catch (e) {
      onError(message(e));
    } finally {
      setBusy(false);
    }
  };
  const installPerfMods = async () => {
    if (!selectedInstanceId) {
      onError("Select an instance first.");
      return;
    }
    setBusy(true);
    setPerfNote(null);
    try {
      const n = await command<number>("install_performance_mods", {
        id: selectedInstanceId,
      });
      setPerfNote(`Installed ${n} performance mod(s) into this instance.`);
    } catch (e) {
      onError(message(e));
    } finally {
      setBusy(false);
    }
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
                  Memory is per instance. JVM presets and video tweaks apply to
                  all launches; performance mods install into the selected
                  instance
                  {selectedInstanceId ? ` (${selectedInstanceId})` : ""}.
                </p>
                <Button variant="outline" onClick={onEdit}>
                  Configure instance memory
                </Button>
                <h3>JVM presets</h3>
                <p>
                  Safe GC tuning only. Save settings after choosing a preset.
                </p>
                <div className="row">
                  <Button
                    variant="outline"
                    disabled={busy}
                    onClick={() => void applyJvmPreset("default")}
                  >
                    Default
                  </Button>
                  <Button
                    variant="outline"
                    disabled={busy}
                    onClick={() => void applyJvmPreset("balanced")}
                  >
                    Balanced
                  </Button>
                  <Button
                    disabled={busy}
                    onClick={() => void applyJvmPreset("high")}
                  >
                    High FPS
                  </Button>
                </div>
                <h3>Minecraft options</h3>
                <p>
                  Turns off VSync, Fast graphics, fewer particles/shadows, FPS
                  cap 260. Keeps your render distance and language.
                </p>
                <Button
                  variant="outline"
                  disabled={busy || !selectedInstanceId}
                  onClick={() => void applyVideo()}
                >
                  Apply FPS video settings
                </Button>
                <h3>Performance mods</h3>
                <p>
                  Installs Sodium, Lithium, FerriteCore, ImmediatelyFast, and
                  Entity Culling when your loader supports them (Fabric / Quilt
                  / NeoForge). Forge gets the subset that has Forge builds.
                  Vanilla needs a loader first.
                </p>
                <Button
                  disabled={busy || !selectedInstanceId}
                  onClick={() => void installPerfMods()}
                >
                  Install FPS mod pack
                </Button>
                {perfNote && <span className="verified">{perfNote}</span>}
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
                  -XX:+UseStringDeduplication, -XX:+AlwaysPreTouch,
                  -XX:+DisableExplicitGC, -XX:+ParallelRefProcEnabled,
                  -XX:+PerfDisableSharedMem, -XX:MaxGCPauseMillis=50|200,
                  -XX:MaxTenuringThreshold=1, -XX:G1NewSizePercent=30,
                  -XX:G1MaxNewSizePercent=40, -XX:G1HeapRegionSize=8M,
                  -XX:G1ReservePercent=20, -XX:InitiatingHeapOccupancyPercent=15.
                  Prefer Settings → Performance presets.
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
                  NodeClient <span className="mono">{APP_VERSION}</span>
                </h3>
                <p>An independent, original Minecraft Java Edition launcher.</p>
                <p>NodeClient is owned by Nodedistro.</p>
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
