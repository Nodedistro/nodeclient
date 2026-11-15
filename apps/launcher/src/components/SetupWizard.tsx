import { useState } from "react";
import { ArrowRight, Check, Cpu, Leaf } from "lucide-react";
import { Button } from "@/components/ui/button";
import { command, message } from "@/lib/api";
import type { Instance, Manifest, Snapshot, Runtime } from "@/lib/models";
import { defaultMemory } from "@/lib/models";
import { AccountPanel } from "./AccountPanel";
export function SetupWizard({
  data,
  manifest,
  onRefresh,
  onDone,
  onError,
}: {
  data: Snapshot;
  manifest: Manifest | null;
  onRefresh: () => Promise<void>;
  onDone: () => Promise<void>;
  onError: (s: string) => void;
}) {
  const [step, setStep] = useState(0);
  const [engine, setEngine] = useState<"horizon" | "java">("horizon");
  const [channel, setChannel] = useState("release");
  const [runtimes, setRuntimes] = useState<Runtime[]>([]);
  const [scanned, setScanned] = useState(false);
  async function finish() {
    try {
      const version = engine === "horizon"
        ? "26.3"
        : manifest?.latest[channel as "release" | "snapshot"];
      if (!version)
        throw new Error(
          "Load the official Minecraft version list before finishing setup.",
        );
      const saved = await command<Instance>("save_instance", {
        instance: {
          id: null,
          engine,
          name:
            channel === "release" ? "My first adventure" : "Snapshot explorer",
          minecraftVersion: version,
          loader: { type: "vanilla" },
          java: { mode: "automatic", path: null },
          memory: {
            minimumMb: 1024,
            maximumMb: defaultMemory(data.totalMemoryMb),
          },
        },
      });
      await command("save_settings", {
        settings: {
          ...data.settings,
          setupComplete: true,
          selectedInstance: saved.id,
        },
      });
      await onDone();
    } catch (e) {
      onError(message(e));
    }
  }
  return (
    <div className="setup">
      <aside className="setup-aside">
        <img src="/nodeclient.svg" alt="" width="52" />
        <div>
          <span className="eyebrow">NodeClient</span>
          <h1>
            Set up
            <br />
            once. Play.
          </h1>
          <p>
            Account, engine, and a first instance.
            <br />
            Then you’re ready.
          </p>
        </div>
        <ol>
          {[
            "Welcome",
            "Minecraft account",
            "Game engine",
            "Starting version",
            "Ready",
          ].map((s, i) => (
            <li
              key={s}
              className={step === i ? "current" : step > i ? "complete" : ""}
            >
              <span>{step > i ? <Check size={12} /> : i + 1}</span>
              {s}
            </li>
          ))}
        </ol>
        <small>FIRST LAUNCH</small>
      </aside>
      <div className="setup-main">
        <span className="eyebrow">
          STEP {String(step + 1).padStart(2, "0")} / 05
        </span>
        {step === 0 && (
          <>
            <Leaf className="welcome-icon" size={54} />
            <h1>Welcome to NodeClient</h1>
            <p>
              Launch Minecraft Java Edition with isolated instances, automatic
              Java, and Microsoft sign-in.
            </p>
            <Button size="lg" onClick={() => setStep(1)}>
              Get started
              <ArrowRight size={18} />
            </Button>
          </>
        )}
        {step === 1 && (
          <>
            <AccountPanel
              accounts={data.accounts}
              selected={data.settings.selectedAccount}
              onSelect={(id) => {
                void command("save_settings", {
                  settings: { ...data.settings, selectedAccount: id },
                }).then(onRefresh);
              }}
              onRefresh={onRefresh}
              onError={onError}
            />
            <Button
              disabled={!data.settings.selectedAccount}
              onClick={() => setStep(2)}
            >
              Continue
              <ArrowRight size={16} />
            </Button>
            <Button variant="ghost" onClick={() => setStep(2)}>
              Set up now, sign in before playing
            </Button>
          </>
        )}
        {step === 2 && (
          <>
            <Cpu size={40} />
            <h1>Game engine</h1>
            <p>
              Horizon runs Minecraft natively in Rust. Java remains available
              for Java mod loaders and mods.
            </p>
            <div className="channel-options">
              <Button
                variant={engine === "horizon" ? "secondary" : "outline"}
                className="channel-option"
                onClick={() => setEngine("horizon")}
              >
                <span>
                  Horizon · Native Rust
                  <strong>Recommended · Minecraft 26.3</strong>
                </span>
                {engine === "horizon" && <Check size={18} />}
              </Button>
              <Button
                variant={engine === "java" ? "secondary" : "outline"}
                className="channel-option"
                onClick={() => setEngine("java")}
              >
                <span>
                  Minecraft · Java
                  <strong>Fabric, Quilt, Forge, NeoForge</strong>
                </span>
                {engine === "java" && <Check size={18} />}
              </Button>
            </div>
            <p>
              Horizon uses Java 25 only for first-time resource preparation;
              gameplay runs in the Rust executable.
            </p>
            <Button
              variant="outline"
              onClick={() => {
                void command<Runtime[]>("java_runtimes")
                  .then((r) => {
                    setRuntimes(r);
                    setScanned(true);
                  })
                  .catch((e) => onError(message(e)));
              }}
            >
              Detect preparation Java
            </Button>
            {scanned && (
              <p>
                {runtimes.length
                  ? `Found Java ${runtimes.map((r) => r.major).join(", ")}.`
                  : "No compatible local installations found. Automatic installation will run when needed."}
              </p>
            )}
            <Button onClick={() => setStep(3)}>
              Continue
              <ArrowRight size={16} />
            </Button>
          </>
        )}
        {step === 3 && (
          <>
            <h1>{engine === "horizon" ? "Minecraft 26.3" : "Starting version"}</h1>
            <p>
              {engine === "horizon"
                ? "This verified Horizon build is pinned to Minecraft 26.3."
                : "You can add more versions and instances anytime."}
            </p>
            {engine === "java" && <div className="channel-options">
              {["release", "snapshot"].map((c) => (
                <Button
                  key={c}
                  variant={c === channel ? "secondary" : "outline"}
                  className="channel-option"
                  onClick={() => setChannel(c)}
                >
                  <span>
                    Latest {c}
                    <strong>
                      {manifest?.latest[c as "release" | "snapshot"] ??
                        "Loading versions…"}
                    </strong>
                  </span>
                  {channel === c && <Check size={18} />}
                </Button>
              ))}
            </div>}
            <Button
              onClick={() => setStep(4)}
              disabled={engine === "java" && !manifest}
            >
              Continue
              <ArrowRight size={16} />
            </Button>
          </>
        )}
        {step === 4 && (
          <>
            <div className="ready-check">
              <Check size={40} />
            </div>
            <h1>Ready to launch</h1>
            <p>
              Your first instance will use{" "}
              {engine === "horizon"
                ? "Horizon’s native Rust engine with Minecraft 26.3"
                : `Minecraft ${manifest?.latest[channel as "release" | "snapshot"]} with Java`}.
              Game files download when you choose Install.
            </p>
            <Button size="lg" onClick={() => void finish()}>
              Open NodeClient
              <ArrowRight size={18} />
            </Button>
          </>
        )}
        {step > 0 && (
          <Button
            variant="ghost"
            className="setup-back"
            onClick={() => setStep(step - 1)}
          >
            Back
          </Button>
        )}
      </div>
    </div>
  );
}
