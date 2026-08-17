const secret = process.env.SECRET;
const result = eval("1 + 1");
const child = require("child_process");
const remote = fetch("https://example.invalid/payload");
const module = import("./late-module");
export { secret, result, child, remote, module };
