import { z } from "zod";
export const instanceSchema = z.object({
  id: z.string().regex(/^[a-zA-Z0-9_-][a-zA-Z0-9._-]*$/),
  name: z.string().trim().min(1).max(100),
  minecraftVersion: z.string().min(1),
  loader: z.object({ type: z.literal("vanilla") }),
  java: z.object({
    mode: z.enum(["automatic", "custom"]),
    path: z.string().nullable(),
  }),
  memory: z
    .object({
      minimumMb: z.number().int().min(512),
      maximumMb: z.number().int().max(65536),
    })
    .refine(
      (v) => v.maximumMb >= v.minimumMb,
      "Maximum RAM must be at least minimum RAM.",
    ),
  lastPlayed: z.number().nullable().optional(),
  playtimeSeconds: z.number().default(0),
});
export type Instance = z.infer<typeof instanceSchema>;
export interface Profile {
  id: string;
  name: string;
  skins: { url?: string; state?: string }[];
  capes: { url?: string }[];
}
export interface Settings {
  setupComplete: boolean;
  selectedInstance: string | null;
  selectedAccount: string | null;
  theme: string;
  width: number;
  height: number;
  fullscreen: boolean;
  concurrency: number;
  minimizeOnLaunch: boolean;
  rememberInstance: boolean;
  jvmArguments: string[];
}
export interface ProcessInfo {
  pid: number | null;
  instance: string;
  minecraftVersion: string;
  startTime: number;
  endTime: number | null;
  exitCode: number | null;
}
export interface Status {
  phase: string;
  message: string;
  process: ProcessInfo | null;
}
export interface Snapshot {
  settings: Settings;
  instances: Instance[];
  accounts: Profile[];
  status: Status;
  totalMemoryMb: number;
  installed: string[];
}
export interface VersionEntry {
  id: string;
  type: string;
  releaseTime: string;
}
export interface Manifest {
  latest: { release: string; snapshot: string };
  versions: VersionEntry[];
}
export interface Runtime {
  path: string;
  major: number;
  version: string;
}
export interface Progress {
  completed: number;
  total: number;
  bytes: number;
  bytesPerSecond: number;
  phase: string;
}
export function defaultMemory(totalMb: number) {
  return Math.max(1024, Math.min(4096, Math.floor(totalMb / 1024 / 3) * 1024));
}
