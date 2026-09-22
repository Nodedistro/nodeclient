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
  instanceSchema,
  defaultMemory,
  type Instance,
  type Manifest,
  type Runtime,
} from "@/lib/models";
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
  const [version, setVersion] = useState("");
  const [ram, setRam] = useState([1024, 4096]);
  const [java, setJava] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    if (open) {
      setName(instance?.name ?? "My world");
      setVersion(
        instance?.minecraftVersion ??
          preferredVersion ??
          manifest?.latest.release ??
          "",
      );
      setRam(
        instance
          ? [instance.memory.minimumMb, instance.memory.maximumMb]
          : [1024, defaultMemory(totalMemory)],
      );
      setJava(instance?.java.path ?? null);
    }
  }, [open, instance, preferredVersion, manifest, totalMemory]);
  async function save() {
    setSaving(true);
    try {
      const item = instanceSchema.parse({
        id: instance?.id ?? crypto.randomUUID(),
        name,
        minecraftVersion: version,
        loader: { type: "vanilla" },
        java: { mode: java ? "custom" : "automatic", path: java },
        memory: { minimumMb: ram[0], maximumMb: ram[1] },
        lastPlayed: instance?.lastPlayed ?? null,
        playtimeSeconds: instance?.playtimeSeconds ?? 0,
      });
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
          <Label>Java runtime</Label>
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
            disabled={saving || !version || !name.trim()}
          >
            Save instance
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
