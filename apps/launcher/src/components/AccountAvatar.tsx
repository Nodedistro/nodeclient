import type { Profile } from "@/lib/models";
import { UserRound } from "lucide-react";

export function AccountAvatar({ profile }: { profile?: Profile }) {
  const rawSkin =
    profile?.skins?.find((s) => s.state === "ACTIVE")?.url ??
    profile?.skins?.[0]?.url;
  const secureSkin = rawSkin?.replace(
    /^http:\/\/textures\.minecraft\.net\//,
    "https://textures.minecraft.net/",
  );
  const safe = secureSkin?.startsWith("https://textures.minecraft.net/texture/")
    ? secureSkin
    : undefined;

  return (
    <span className="avatar">
      {safe ? (
        <span
          className="skin-head"
          style={{ backgroundImage: `url("${safe}")` }}
        >
          <span
            className="skin-hat"
            style={{ backgroundImage: `url("${safe}")` }}
          />
        </span>
      ) : (
        <UserRound size={20} />
      )}
    </span>
  );
}
