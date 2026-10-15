import { commands, setupIpc } from "$lib/ipc";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ params }) => {
  // Layout and page loads run together; the browser mock must be installed first.
  await setupIpc();
  return { result: await commands.portGet(params.id) };
};
