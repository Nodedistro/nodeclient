import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";
import { command, message } from "@/lib/api";
import {
  defaultMemory,
  loaderLabel,
  type LoaderType,
  type LoaderVersion,
  type Instance,
  type Manifest,
  type Runtime,
} from "@/lib/models";

const LOADER_OPTIONS: { type: LoaderType; label: string }[] = [
  { type: "vanilla", label: "Vanilla" },
  { type: "fabric", label: "Fabric" },
  { type: "quilt", label: "Quilt" },
  { type: "forge", label: "Forge" },
  { type: "neoforge", label: "NeoForge" },
];

export function InstanceEditor({
  open,
  onClose,
  instance,
  preferredVersion,
  manifest,
  totalMemory,
  onSaved,
  onError,
}: {
  open: boolean;
  onClose: () => void;
  instance?: Instance;
  preferredVersion?: string;
  manifest: Manifest | null;
  totalMemory: number;
  onSaved: () => Promise<void>;
  onError: (s: string) => void;
}) {
  const [name, setName] = useState("");
  const [engine, setEngine] = useState<"java" | "horizon">("horizon");
  const [version, setVersion] = useState("");
  const [loaderType, setLoaderType] = useState<LoaderType>("vanilla");
  const [loaderVersion, setLoaderVersion] = useState("");
  const [loaderVersions, setLoaderVersions] = useState<LoaderVersion[]>([]);
  const [loadingLoaders, setLoadingLoaders] = useState(false);
  const [ram, setRam] = useState([1024, 4096]);
  const [java, setJava] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (open) {
      setName(instance?.name ?? "My world");
      setEngine(instance?.engine ?? (preferredVersion ? "java" : "horizon"));
      setVersion(
        instance?.minecraftVersion ??
          preferredVersion ??
          manifest?.latest.release ??
          "",
      );
      const type = instance?.loader.type;
      setLoaderType(
        type && LOADER_OPTIONS.some((o) => o.type === type)
          ? (type as LoaderType)
          : "vanilla",
      );
      setLoaderVersion(instance?.loader.version ?? "");
      setRam(
        instance
          ? [instance.memory.minimumMb, instance.memory.maximumMb]
          : [1024, defaultMemory(totalMemory)],
      );
      setJava(instance?.java.path ?? null);
    }
  }, [open, instance, preferredVersion, manifest, totalMemory]);

  useEffect(() => {
    if (!open || engine === "horizon" || loaderType === "vanilla" || !version) {
      setLoaderVersions([]);
      return;
    }
    let cancelled = false;
    setLoadingLoaders(true);
    void command<LoaderVersion[]>("loader_versions", {
      loader: loaderType,
      gameVersion: version,
    })
      .then((list) => {
        if (cancelled) return;
        setLoaderVersions(list);
        setLoaderVersion((current) => {
          if (current && list.some((l) => l.version === current)) return current;
          const stable = list.find((l) => l.stable);
          return stable?.version ?? list[0]?.version ?? "";
        });
      })
      .catch((e) => {
        if (!cancelled) {
          setLoaderVersions([]);
          setLoaderVersion("");
          onError(message(e));
        }
      })
      .finally(() => {
        if (!cancelled) setLoadingLoaders(false);
      });
    return () => {
      cancelled = true;
    };
  }, [open, engine, loaderType, version, onError]);

  async function save() {
    setSaving(true);
    try {
      const item = {
        id: instance?.id ?? null,
        name,
        engine,
        minecraftVersion: engine === "horizon" ? "26.3" : version,
        loader:
          engine === "horizon" || loaderType === "vanilla"
            ? { type: "vanilla", version: null }
            : { type: loaderType, version: loaderVersion },
        java: { mode: java ? "custom" : "automatic", path: java },
        memory: { minimumMb: ram[0], maximumMb: ram[1] },
      };
      await command("save_instance", { instance: item });
      await onSaved();
      onClose();
    } catch (e) {
      onError(message(e));
    } finally {
      setSaving(false);
    }
  }

  const maxRam = Math.min(
    65536,
    Math.max(2048, Math.floor((totalMemory * 0.75) / 512) * 512),
  );
  const canSave =
    Boolean(name.trim()) && (engine === "horizon" || (Boolean(version) &&
    (loaderType === "vanilla" || Boolean(loaderVersion))));
  const loaderName =
    LOADER_OPTIONS.find((o) => o.type === loaderType)?.label ?? loaderType;

  return (
    <Dialog
      open={open}
      onOpenChange={(v) => {
        if (!v) onClose();
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>
            {instance ? "Edit instance" : "Create an instance"}
          </DialogTitle>
          <DialogDescription>
            Keep worlds, settings, and game files in their own space.
          </DialogDescription>
        </DialogHeader>
        <div className="stack">
          <Label htmlFor="instance-name">Name</Label>
          <Input
            id="instance-name"
            value={name}
            onChange={(e) => setName(e.target.value)}
            maxLength={100}
          />
          <Label>Game engine</Label>
          <div className="row">
            <Button disabled={!!instance} variant={engine === "horizon" ? "secondary" : "outline"} onClick={() => setEngine("horizon")}>Horizon · Native Rust</Button>
            <Button disabled={!!instance} variant={engine === "java" ? "secondary" : "outline"} onClick={() => setEngine("java")}>Minecraft · Java</Button>
          </div>
          {instance && <p>Create a new instance to use another engine. Existing worlds stay with this engine.</p>}
          {engine === "horizon" ? (
            <p>Horizon runs Minecraft 26.3 natively in Rust. Sign in once when NodeClient opens. Initial resource preparation uses Java 25; gameplay runs natively. Java mods and loaders require a Java instance. Horizon compatibility is still evolving.</p>
          ) : (<>
          <Label>Minecraft version</Label>
          <Select value={version} onValueChange={setVersion}>
            <SelectTrigger>
              <SelectValue placeholder="Choose a version" />
            </SelectTrigger>
            <SelectContent>
              {manifest?.versions
                .filter((v) => ["release", "snapshot"].includes(v.type))
                .map((v) => (
                  <SelectItem key={v.id} value={v.id}>
                    {v.id} · {v.type}
                  </SelectItem>
                ))}
            </SelectContent>
          </Select>
          <Label>Loader</Label>
          <div className="row" style={{ flexWrap: "wrap", gap: 8 }}>
            {LOADER_OPTIONS.map((opt) => (
              <Button
                key={opt.type}
                variant={loaderType === opt.type ? "secondary" : "outline"}
                onClick={() => setLoaderType(opt.type)}
              >
                {opt.label}
              </Button>
            ))}
          </div>
          {loaderType !== "vanilla" && (
            <>
              <Label>{loaderName} version</Label>
              <Select
                value={loaderVersion}
                onValueChange={setLoaderVersion}
                disabled={loadingLoaders || !loaderVersions.length}
              >
                <SelectTrigger>
                  <SelectValue
                    placeholder={
                      loadingLoaders
                        ? `Loading ${loaderName} versions…`
                        : `Choose ${loaderName} version`
                    }
                  />
                </SelectTrigger>
                <SelectContent>
                  {loaderVersions.map((l) => (
                    <SelectItem key={l.version} value={l.version}>
                      {l.version}
                      {l.stable
                        ? l.version.includes("beta") || l.version.includes("alpha")
                          ? " · latest beta"
                          : " · recommended"
                        : l.version.includes("beta") || l.version.includes("alpha")
                          ? " · beta"
                          : ""}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <p>
                {loaderLabel({
                  loader: { type: loaderType, version: loaderVersion || null },
                })}{" "}
                installs into this instance only. Mods go in the instance mods
                folder.
              </p>
            </>
          )}
          </>)}
          {engine === "java" && (<>
          <Label>
            Memory · {ram[0]}–{ram[1]} MB
          </Label>
          <Slider
            aria-label="Minimum and maximum memory"
            min={512}
            max={maxRam}
            step={512}
            minStepsBetweenThumbs={0}
            value={ram}
            onValueChange={setRam}
          />
          <p>
            {Math.round(totalMemory / 1024)} GB system RAM. Leave room for
            Windows and other apps.
          </p>
          </>)}
          <Label>{engine === "horizon" ? "Preparation Java (25)" : "Java runtime"}</Label>
          <div className="row">
            <Button
              variant={!java ? "secondary" : "outline"}
              onClick={() => setJava(null)}
            >
              Automatic
            </Button>
            <Button
              variant="outline"
              onClick={() => {
                void command<Runtime | null>("browse_java")
                  .then((r) => {
                    if (r) setJava(r.path);
                  })
                  .catch((e) => onError(message(e)));
              }}
            >
              Browse for Java
            </Button>
          </div>
          <p className="break-all">
            {java ??
              "Detect compatible Java, or install a verified runtime from Mojang."}
          </p>

        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            Cancel
          </Button>
          <Button
            onClick={() => void save()}
            disabled={saving || !canSave}
          >
            Save instance
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
