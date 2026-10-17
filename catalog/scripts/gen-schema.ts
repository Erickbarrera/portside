// Writes schemas/manifest.v1.json from the TypeBox manifest schema. Commit the output.
import { writeFileSync } from "node:fs";
import { ManifestSchema } from "../src/schema/manifest.js";

const out = new URL("../schemas/manifest.v1.json", import.meta.url);
const schema = { $schema: "https://json-schema.org/draft/2020-12/schema", ...ManifestSchema };
writeFileSync(out, `${JSON.stringify(schema, null, 2)}\n`);
console.log(`wrote ${out.pathname}`);
