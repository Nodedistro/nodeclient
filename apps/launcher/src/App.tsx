import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  ArrowDownToLine,
  BookOpen,
  ChevronDown,
  Compass,
  Download,
  Home,
  LoaderCircle,
  Plus,
  Server,
  Settings2,
  Terminal,
  UserRound,
  X,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { AccountAvatar } from "@/components/AccountAvatar";
import { AccountPanel } from "@/components/AccountPanel";
import { BrowsePage, type BrowseTab } from "@/components/BrowsePage";
import { DownloadsPage } from "@/components/DownloadsPage";
import { HomePage, useModCount } from "@/components/HomePage";
import { InstanceEditor } from "@/components/InstanceEditor";
import { LibraryPage, type LibraryTab } from "@/components/LibraryPage";
import { LogsPage } from "@/components/LogsPage";
import { ServersPage } from "@/components/ServersPage";
import { SettingsPanel } from "@/components/SettingsPanel";
import { SetupWizard } from "@/components/SetupWizard";
import type { UpdateInfo } from "@/components/UpdatePanel";
import { command, desktop, message } from "@/lib/api";
import type {
  Instance,
  Manifest,
  Progress,
  Settings,
  Snapshot,
  Status,
} from "@/lib/models";

const initial: Snapshot = {
  settings: {
    setupComplete: true,
    selectedInstance: null,
    selectedAccount: null,
    theme: "dark",
    width: 1280,
    height: 720,
    fullscreen: false,
    concurrency: 6,
    minimizeOnLaunch: false,
    rememberInstance: true,
    jvmArguments: [],
  },
  instances: [],
  accounts: [],
  status: { phase: "IDLE", message: "Ready", process: null },
  totalMemoryMb: 8192,
  installed: [],
};

const navigation = [
  { name: "Home", icon: Home },
  { name: "Library", icon: BookOpen },
  { name: "Browse", icon: Compass },
  { name: "Servers", icon: Server },
  { name: "Downloads", icon: Download },
] as const;

type PageName =
  | (typeof navigation)[number]["name"]
  | "Accounts"
  | "Settings"
  | "Logs";

export default function App() {
  const [data, setData] = useState<Snapshot>(initial);
  const [manifest, setManifest] = useState<Manifest | null>(null);
  const [page, setPage] = useState<PageName>("Home");
  const [libraryTab, setLibraryTab] = useState<LibraryTab>("Instances");
  const [browseTab, setBrowseTab] = useState<BrowseTab>("Versions");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(desktop);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [editor, setEditor] = useState(false);
  const [editing, setEditing] = useState<Instance | undefined>();
  const [preferredVersion, setPreferredVersion] = useState<
    string | undefined
  >();
  const [search, setSearch] = useState("");
  const [logKind, setLogKind] = useState("NodeClient");
  const [logText, setLogText] = useState("");
  const [logFilter, setLogFilter] = useState("All");
  const [busy, setBusy] = useState(false);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [settingsSection, setSettingsSection] = useState("General");

  const refresh = useCallback(async () => {
    if (desktop) setData(await command<Snapshot>("snapshot"));
  }, []);

  const loadVersions = useCallback(async () => {
    if (desktop) {
      try {
        setManifest(await command<Manifest>("versions"));
      } catch (e) {
        setError(message(e));
      }
    }
  }, []);

  useEffect(() => {
    void refresh()
      .catch((e) => setError(message(e)))
      .finally(() => setLoading(false));
    void loadVersions();
    if (desktop) {
      void command<UpdateInfo>("check_for_updates")
        .then(setUpdate)
        .catch(() => {
          // Offline or rate-limited; Settings > Updates can retry.
        });
    }
  }, [refresh, loadVersions]);

  useEffect(() => {
    if (!desktop) return;
    let disposed = false;
    const cleanups: (() => void)[] = [];
    const on = <T,>(name: string, handler: (v: T) => void) => {
      void listen<T>(name, (e) => handler(e.payload)).then((unlisten) => {
        if (disposed) unlisten();
        else cleanups.push(unlisten);
      });
    };
    on<Status>("status", (s) => {
      setData((d) => ({ ...d, status: s }));
      if (s.phase === "IDLE") {
        setBusy(false);
        setProgress(null);
        void refresh();
      }
    });
    on<Progress>("download-progress", setProgress);
    on<unknown>("account-changed", () => {
      void refresh();
    });
    on<string>("auth-error", setError);
    return () => {
      disposed = true;
      cleanups.forEach((f) => f());
    };
  }, [refresh]);

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () =>
      document.documentElement.classList.toggle(
        "dark",
        data.settings.theme === "dark" ||
          (data.settings.theme === "system" && media.matches),
      );
    apply();
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [data.settings.theme]);

  const selected =
    data.instances.find((i) => i.id === data.settings.selectedInstance) ??
    data.instances[0];
  const account = data.accounts.find(
    (a) => a.id === data.settings.selectedAccount,
  );
  const active = busy || data.status.phase !== "IDLE";
  const installed = !!selected && data.installed.includes(selected.id);
  const modCount = useModCount(selected?.id, desktop);

  const saveSettings = async (settings: Settings) => {
    await command("save_settings", { settings });
    await refresh();
  };

  const action = async (task: () => Promise<unknown>) => {
    setError("");
    try {
      await task();
    } catch (e) {
      setError(message(e));
    }
  };

  const selectAccount = (id: string) => {
    void action(() => saveSettings({ ...data.settings, selectedAccount: id }));
  };

  const openEditor = (instance?: Instance) => {
    setEditing(instance);
    setPreferredVersion(undefined);
    setEditor(true);
  };

  const chooseInstance = (id: string) => {
    void action(() => saveSettings({ ...data.settings, selectedInstance: id }));
  };

  const selectVersion = async (version: string) => {
    if (selected) {
      await command("save_instance", {
        instance: { ...selected, minecraftVersion: version },
      });
      await refresh();
      setPage("Home");
    } else {
      setEditing(undefined);
      setPreferredVersion(version);
      setEditor(true);
    }
  };

  async function play(opts?: { safeMode?: boolean; server?: string }) {
    if (!selected) {
      openEditor();
      return;
    }
    const installOnly = !installed && !opts?.safeMode;
    if (!installOnly && !account) {
      setPage("Accounts");
      return;
    }
    setBusy(true);
    setError("");
    try {
      if (selected.edition === "bedrock") {
        await command("launch_bedrock");
        await refresh();
        return;
      }
      await command("launch", {
        id: selected.id,
        installOnly,
        safeMode: opts?.safeMode ?? false,
        server: opts?.server,
      });
      await refresh();
    } catch (e) {
      setError(message(e));
    } finally {
      setBusy(false);
    }
  }

  async function repair(id?: string) {
    const target = id ?? selected?.id;
    if (!target) return;
    setBusy(true);
    setError("");
    try {
      await command("repair_instance", { id: target });
      await refresh();
    } catch (e) {
      setError(message(e));
    } finally {
      setBusy(false);
    }
  }

  async function explainCrash() {
    if (!selected) {
      setError("Select an instance first.");
      return;
    }
    try {
      const explained = await command<{
        title: string;
        summary: string;
        details: string[];
        actions: string[];
        reportName: string | null;
      }>("explain_crash", { id: selected.id });
      const body = [
        explained.title,
        explained.summary,
        "",
        ...explained.details.map((d) => `• ${d}`),
        explained.reportName ? `\nSource: ${explained.reportName}` : "",
      ].join("\n");
      setError(body);
      setLogKind("Crash Reports");
      setPage("Logs");
      await fetchLogs();
    } catch (e) {
      setError(message(e));
    }
  }

  const fetchLogs = useCallback(async () => {
    try {
      setLogText(
        await command<string>("read_logs", {
          id: selected?.id ?? null,
          kind: logKind,
        }),
      );
    } catch (e) {
      setError(message(e));
    }
  }, [selected?.id, logKind]);

  useEffect(() => {
    if (page === "Logs" && desktop) void fetchLogs();
  }, [page, fetchLogs]);

  const playLabel = active
    ? data.status.phase === "IDLE"
      ? "PREPARING"
      : data.status.phase
    : !selected
      ? "CREATE"
      : !installed
        ? "INSTALL"
        : !account
          ? "SIGN IN"
          : "PLAY";

  const topTitle =
    page === "Home"
      ? "Play"
      : page === "Library"
        ? `Library · ${libraryTab}`
        : page === "Browse"
          ? `Browse · ${browseTab}`
          : page;

  const notice = error && (
    <div className="error-banner" role="alert">
      <div>
        <strong>{error.split(". ")[0]}</strong>
        <details>
          <summary>Technical details</summary>
          <p>{error}</p>
        </details>
      </div>
      <Button
        variant="ghost"
        size="icon"
        aria-label="Dismiss error"
        onClick={() => setError("")}
      >
        <X size={16} />
      </Button>
    </div>
  );

  if (loading)
    return (
      <div className="app-loading">
        <img src="/nodeclient.svg" width="64" alt="NodeClient" />
        <LoaderCircle className="spin" />
        <p>Opening NodeClient…</p>
      </div>
    );

  if (!data.settings.setupComplete)
    return (
      <>
        {notice}
        <SetupWizard
          data={data}
          manifest={manifest}
          onRefresh={refresh}
          onDone={refresh}
          onError={setError}
        />
      </>
    );

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <button className="brand" onClick={() => setPage("Home")}>
          <img src="/nodeclient.svg" alt="" width="34" />
          <span className="brand-mark">
            Node<em>Client</em>
          </span>
        </button>
        <div className="nav-label">Play</div>
        <nav>
          {navigation.map(({ name, icon: Icon }) => (
            <button
              key={name}
              className={page === name ? "nav-item selected" : "nav-item"}
              onClick={() => {
                setPage(name);
                setSearch("");
              }}
            >
              <Icon size={18} />
              {name}
            </button>
          ))}
        </nav>
        <div className="sidebar-bottom">
          {[
            { name: "Logs" as const, icon: Terminal },
            { name: "Settings" as const, icon: Settings2 },
          ].map(({ name, icon: Icon }) => (
            <button
              key={name}
              className={page === name ? "nav-item selected" : "nav-item"}
              onClick={() => {
                if (name === "Settings") setSettingsSection("General");
                setPage(name);
              }}
            >
              <Icon size={18} />
              {name}
            </button>
          ))}
          <button
            className="account-switch"
            onClick={() => setPage("Accounts")}
          >
            <AccountAvatar profile={account} />
            <span>
              {account?.name ?? "Guest"}
              <small>
                {account ? "Minecraft account" : "Connect account"}
              </small>
            </span>
            <ChevronDown size={15} />
          </button>
        </div>
      </aside>
      <div className="workspace">
        <header className="topbar">
          <div className="topbar-title">{topTitle}</div>
          <div className="topbar-right">
            {page === "Library" && libraryTab === "Instances" && (
              <Button disabled={active} onClick={() => openEditor()}>
                <Plus size={16} />
                Create instance
              </Button>
            )}
            <Button
              variant="ghost"
              size="icon"
              aria-label="Accounts"
              onClick={() => setPage("Accounts")}
            >
              <UserRound size={18} />
            </Button>
          </div>
        </header>
        <main>
          {!desktop && (
            <div className="preview-note">
              Browser preview · Open the desktop app with{" "}
              <code>pnpm tauri dev</code> to sign in, install, and play.
            </div>
          )}
          {notice}
          {update?.available && (
            <div className="update-banner">
              <div>
                <strong>NodeClient {update.latestVersion} is available</strong>
                <p>You are on {update.currentVersion}.</p>
              </div>
              <Button
                onClick={() => {
                  setSettingsSection("Updates");
                  setPage("Settings");
                }}
              >
                View update
              </Button>
            </div>
          )}
          {page === "Home" && (
            <HomePage
              data={data}
              selected={selected}
              accountReady={!!account}
              installed={installed}
              active={active}
              desktop={desktop}
              progress={progress}
              playLabel={playLabel}
              modCount={modCount}
              onChooseInstance={chooseInstance}
              onPlay={() => void play()}
              onSafePlay={() => void play({ safeMode: true })}
              onEdit={() => openEditor(selected)}
              onOpenLogs={(kind) => {
                setLogKind(kind);
                setPage("Logs");
              }}
              onAction={action}
              onRefresh={refresh}
              onRepair={() => void repair()}
            />
          )}
          {page === "Accounts" && (
            <AccountPanel
              key={data.accounts.map((a) => a.id).join(",")}
              accounts={data.accounts}
              selected={data.settings.selectedAccount}
              onSelect={selectAccount}
              onRefresh={refresh}
              onError={setError}
            />
          )}
          {page === "Library" && (
            <LibraryPage
              tab={libraryTab}
              onTab={setLibraryTab}
              instances={data.instances}
              selectedId={selected?.id}
              installed={data.installed}
              active={active}
              desktop={desktop}
              onSelectInstance={chooseInstance}
              onEdit={openEditor}
              onCreate={() => openEditor()}
              onRefresh={refresh}
              onAction={action}
              onPlayInstance={(id) => {
                chooseInstance(id);
                setPage("Home");
              }}
              onRepair={(id) => void repair(id)}
            />
          )}
          {page === "Browse" && (
            <BrowsePage
              tab={browseTab}
              onTab={setBrowseTab}
              manifest={manifest}
              search={search}
              selected={selected}
              active={active}
              desktop={desktop}
              instances={data.instances}
              onSearch={setSearch}
              onRefreshVersions={() => void loadVersions()}
              onSelectVersion={selectVersion}
              onSelectInstance={chooseInstance}
              onAction={action}
            />
          )}
          {page === "Servers" && (
            <ServersPage
              instances={data.instances}
              selectedId={selected?.id}
              desktop={desktop}
              onAction={action}
              onSelectInstance={chooseInstance}
              onLaunch={(address) => play({ server: address })}
            />
          )}
          {page === "Downloads" && (
            <DownloadsPage
              status={data.status}
              progress={progress}
              active={active}
            />
          )}
          {page === "Settings" && (
            <SettingsPanel
              settings={data.settings}
              onSave={saveSettings}
              onError={setError}
              onAccounts={() => setPage("Accounts")}
              onEdit={() => openEditor(selected)}
              initialSection={settingsSection}
              selectedInstanceId={selected?.id ?? null}
            />
          )}
          {page === "Logs" && (
            <LogsPage
              logKind={logKind}
              logText={logText}
              logFilter={logFilter}
              search={search}
              selectedId={selected?.id}
              onLogKind={setLogKind}
              onLogFilter={setLogFilter}
              onSearch={setSearch}
              onRefresh={() => void fetchLogs()}
              onAction={action}
              onExplainCrash={() => void explainCrash()}
            />
          )}
        </main>
        <footer className="statusbar">
          <button
            type="button"
            className="status-link"
            onClick={() => setPage("Downloads")}
          >
            <ArrowDownToLine size={13} />
            {progress
              ? `${progress.completed} / ${progress.total} files`
              : "Downloads idle"}
          </button>
          <span>
            <span
              className={
                data.status.phase === "RUNNING" ? "live-dot" : "status-dot"
              }
            />
            {data.status.phase === "RUNNING"
              ? "Minecraft running"
              : data.status.message === "Ready"
                ? "Ready"
                : data.status.message}
          </span>
          <span className="status-version">
            {update?.available
              ? `UPDATE ${update.latestVersion}`
              : "NODECLIENT"}{" "}
            <strong>0.1.5</strong>
          </span>
        </footer>
      </div>
      <InstanceEditor
        open={editor}
        onClose={() => setEditor(false)}
        instance={editing}
        preferredVersion={preferredVersion}
        manifest={manifest}
        totalMemory={data.totalMemoryMb}
        onSaved={refresh}
        onError={setError}
      />
    </div>
  );
}
