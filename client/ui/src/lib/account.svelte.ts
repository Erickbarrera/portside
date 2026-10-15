// The signed-in Bluesky account, shared by the sidebar, Settings and every social page.
import { events, type Account, type CommandError, commands } from "./ipc";

let account = $state<Account | null>(null);
let loaded = $state(false);
let signingIn = $state(false);
let started: Promise<void> | null = null;

export const auth = {
  get account() {
    return account;
  },
  get loaded() {
    return loaded;
  },
  /** True while the browser sign-in page is open. */
  get signingIn() {
    return signingIn;
  },
  /** Loads the account and follows sign-in changes. Safe to call more than once. */
  start(): Promise<void> {
    started ??= (async () => {
      await events.authChanged.listen(({ payload }) => {
        account = payload.account;
      });
      account = await commands.authStatus();
      loaded = true;
    })();
    return started;
  },
  /** Opens the browser; resolves when the person comes back (or cancels). */
  async signIn(handle: string): Promise<CommandError | null> {
    signingIn = true;
    try {
      const result = await commands.authSignIn(handle);
      if (result.status === "ok") {
        account = result.data;
        return null;
      }
      return result.error;
    } finally {
      signingIn = false;
    }
  },
  cancel() {
    void commands.authCancel();
  },
  async signOut() {
    await commands.authSignOut();
    account = null;
  },
};
