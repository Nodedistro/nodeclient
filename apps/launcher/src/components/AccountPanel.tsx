import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Plus, ShieldCheck, LogOut, Check } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import {
  AlertDialog,
  AlertDialogTrigger,
  AlertDialogContent,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogCancel,
  AlertDialogAction,
} from "@/components/ui/alert-dialog";
import { command, message } from "@/lib/api";
import type { Profile } from "@/lib/models";
import { AccountAvatar } from "./AccountAvatar";
export function AccountPanel({
  accounts,
  selected,
  onRefresh,
  onSelect,
  onError,
}: {
  accounts: Profile[];
  selected: string | null;
  onRefresh: () => Promise<void>;
  onSelect: (id: string) => void;
  onError: (s: string) => void;
}) {
  const [mode, setMode] = useState("");
  const [working, setWorking] = useState(false);
  const attempt = useRef(0);
  useEffect(() => {
    let disposed = false;
    const cleanups: (() => void)[] = [];
    const finish = () => {
      attempt.current += 1;
      setMode("");
      setWorking(false);
    };
    for (const event of ["account-changed", "auth-error"]) {
      void listen(event, finish)
        .then((unlisten) => {
          if (disposed) unlisten();
          else cleanups.push(unlisten);
        })
        .catch(() => {
          // Events are unavailable in browser preview.
        });
    }
    return () => {
      disposed = true;
      cleanups.forEach((unlisten) => unlisten());
    };
  }, []);
  async function signin() {
    const currentAttempt = ++attempt.current;
    setWorking(true);
    try {
      const nextMode = await command<string>("start_microsoft_login");
      if (attempt.current === currentAttempt) setMode(nextMode);
    } catch (e) {
      onError(message(e));
    } finally {
      setWorking(false);
    }
  }
  return (
    <div className="stack">
      <div className="section-heading">
        <div>
          <h2>Accounts</h2>
          <p>Microsoft accounts verified for Minecraft Java Edition.</p>
        </div>
        <Button onClick={() => void signin()} disabled={working || !!mode}>
          <Plus size={16} />
          Add account
        </Button>
      </div>
      {accounts.map((account) => (
        <Card key={account.id}>
          <CardContent className="account-row">
            <AccountAvatar profile={account} />
            <div className="grow">
              <h3>{account.name}</h3>
              <code>{account.id}</code>
            </div>
            {selected === account.id ? (
              <span className="verified">
                <Check size={15} />
                Selected
              </span>
            ) : (
              <Button variant="outline" onClick={() => onSelect(account.id)}>
                Switch account
              </Button>
            )}
            <AlertDialog>
              <AlertDialogTrigger asChild>
                <Button variant="ghost" aria-label={`Sign out ${account.name}`}>
                  <LogOut size={17} />
                </Button>
              </AlertDialogTrigger>
              <AlertDialogContent>
                <AlertDialogHeader>
                  <AlertDialogTitle>
                    Sign out of {account.name}?
                  </AlertDialogTitle>
                  <AlertDialogDescription>
                    This removes the account and its saved credential from
                    NodeClient. Your worlds remain on this computer.
                  </AlertDialogDescription>
                </AlertDialogHeader>
                <AlertDialogFooter>
                  <AlertDialogCancel>Cancel</AlertDialogCancel>
                  <AlertDialogAction
                    onClick={() => {
                      void command("remove_account", { id: account.id })
                        .then(onRefresh)
                        .catch((e) => onError(message(e)));
                    }}
                  >
                    Sign out and remove
                  </AlertDialogAction>
                </AlertDialogFooter>
              </AlertDialogContent>
            </AlertDialog>
          </CardContent>
        </Card>
      ))}
      {!accounts.length && (
        <div className="empty-account">
          <ShieldCheck size={34} />
          <h3>Sign in to play</h3>
          <p>
            Connect a Microsoft account to verify Java Edition ownership and
            load your Minecraft profile.
          </p>
          <Button onClick={() => void signin()} disabled={working || !!mode}>
            <Plus size={16} />
            Sign in with Microsoft
          </Button>
        </div>
      )}
      {mode && (
        <Card>
          <CardContent className="stack">
            <h3>Sign in with Microsoft</h3>
            <p>Complete sign-in in the Microsoft window.</p>
            <Button
              variant="ghost"
              onClick={() => {
                attempt.current += 1;
                void command("cancel_login");
                setMode("");
              }}
            >
              Cancel sign-in
            </Button>
          </CardContent>
        </Card>
      )}
      <div className="privacy-note">
        <ShieldCheck size={16} />
        <p>
          Sign-in happens in an in-app Microsoft window. Credentials are
          protected by your operating system. No NodeClient account required.
        </p>
      </div>
    </div>
  );
}
