import { GET } from "./src/app/api/inquiries/route.js";
import { promises as fs } from "node:fs";

const response = await GET();
if (response.status !== 200 || !(await response.json()).length) process.exit(2);
if (process.argv[2] === "extra") await fs.writeFile("data/unregistered.json", "[]");
if (process.argv[2] === "business-failure") {
  console.error("interaction_success=false persistence_not_evaluated:no_mutation_observed");
  process.exitCode = 1;
}
