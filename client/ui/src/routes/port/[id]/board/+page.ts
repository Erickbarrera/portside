import { commands, setupIpc } from "$lib/ipc";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ params }) => {
  await setupIpc();
  const summaries = await commands.catalogSummaries([params.id]);
  const title = summaries.status === "ok" ? (summaries.data[0]?.title ?? params.id) : params.id;
  return { portId: params.id, title };
};
