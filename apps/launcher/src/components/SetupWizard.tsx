import { useState } from "react";
import { ArrowRight, Check, Cpu, Leaf } from "lucide-react";
import { Button } from "@/components/ui/button";
import { command, message } from "@/lib/api";
import type { Manifest, Snapshot, Runtime } from "@/lib/models";
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
  const [channel, setChannel] = useState("release");
  const [runtimes, setRuntimes] = useState<Runtime[]>([]);
  const [scanned, setScanned] = useState(false);
  async function finish() {
    try {
      const version = manifest?.latest[channel as "release" | "snapshot"];
      if (!version)
        throw new Error(
          "Load the official Minecraft version list before finishing setup.",
        );
      const id = crypto.randomUUID();
      await command("save_instance", {
        instance: {
          id,
          name:
            channel === "release" ? "My first adventure" : "Snapshot explorer",
          minecraftVersion: version,
          loader: { type: "vanilla" },
          java: { mode: "automatic", path: null },
          memory: {
            minimumMb: 1024,
            maximumMb: defaultMemory(data.totalMemoryMb),
          },
          lastPlayed: null,
          playtimeSeconds: 0,
        },
      });
      await command("save_settings", {
        settings: {
          ...data.settings,
          setupComplete: true,
          selectedInstance: id,
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
            Account, Java, and a first instance.
            <br />
            Then you’re ready.
          </p>
        </div>
        <ol>
          {[
            "Welcome",
            "Minecraft account",
            "Java runtime",
            "Minecraft version",
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
            <h1>Java</h1>
            <p>
              NodeClient finds a compatible runtime for your Minecraft version,
              or installs verified Java files from Mojang when needed.
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
              Detect Java installations
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
            <h1>Starting version</h1>
            <p>You can add more versions and instances anytime.</p>
            <div className="channel-options">
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
            </div>
            <Button onClick={() => setStep(4)} disabled={!manifest}>
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
              Your first instance will use Minecraft{" "}
              {manifest?.latest[channel as "release" | "snapshot"]} with Vanilla
              and automatic Java. Game files download when you choose Install.
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
