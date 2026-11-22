import { readFile } from "node:fs/promises";
import { webcrypto } from "node:crypto";
import ts from "typescript";

if (!globalThis.crypto) {
  Object.defineProperty(globalThis, "crypto", { value: webcrypto });
}

const source = await readFile(
  new URL("../src/lib/backup.ts", import.meta.url),
  "utf8",
);
const transpiled = ts.transpileModule(source, {
  compilerOptions: {
    target: ts.ScriptTarget.ES2022,
    module: ts.ModuleKind.ES2022,
  },
}).outputText;
const moduleUrl = `data:text/javascript;base64,${Buffer.from(transpiled).toString("base64")}`;
const { encryptBackup, decryptBackup } = await import(moduleUrl);

const input = await readFile(0, "utf8");
const document = JSON.parse(input);
const passphrase = "miora-ci-end-to-end-backup-roundtrip-2026";
const encrypted = await encryptBackup(document, passphrase);
const restored = await decryptBackup(encrypted, passphrase);

process.stdout.write(JSON.stringify(restored));
