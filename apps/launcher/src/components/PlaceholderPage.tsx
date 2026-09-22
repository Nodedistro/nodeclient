import { FolderOpen, Image, Package, Server } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { command } from "@/lib/api";

const copy = {
  Mods: {
    icon: Package,
    title: "Mods come after Vanilla",
    body: "Fabric and mod management will follow once sign-in and launch are verified.",
  },
  Servers: {
    icon: Server,
    title: "Servers stay in Minecraft for now",
    body: "Join multiplayer from Minecraft. Saved server management will follow later.",
  },
  Screenshots: {
    icon: Image,
    title: "Screenshots live in your instance",
    body: "Minecraft saves screenshots in the instance folder. A gallery will follow later.",
  },
} as const;

export function PlaceholderPage({
  page,
  selectedId,
  onAction,
}: {
  page: keyof typeof copy;
  selectedId?: string;
  onAction: (task: () => Promise<unknown>) => Promise<void>;
}) {
  const item = copy[page];
  const Icon = item.icon;
  return (
    <div className="empty-state">
      <span className="empty-symbol">
        <Icon size={38} />
      </span>
      <Badge variant="outline">AFTER VANILLA</Badge>
      <h2>{item.title}</h2>
      <p>{item.body}</p>
      {selectedId && (
        <Button
          variant="outline"
          onClick={() =>
            void onAction(() => command("open_instance", { id: selectedId }))
          }
        >
          <FolderOpen size={16} />
          Open instance folder
        </Button>
      )}
    </div>
  );
}
