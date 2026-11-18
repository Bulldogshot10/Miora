import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { webcrypto } from "node:crypto";
import test from "node:test";
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

const passphrase = "miora-backup-passphrase-2026";
const sampleBackup = {
  format: "miora-backup-data",
  version: 1,
  periods: [
    {
      startDate: "2026-11-01",
      endDate: "2026-11-05",
      flowIntensity: 2,
      lastObservation: "2026-11-09",
      endConfirmed: true,
    },
  ],
  checkins: [
    {
      date: "2026-11-03",
      moodScore: 4,
      energyScore: 2,
      symptoms: [{ code: "cramps", severity: 2 }],
    },
  ],
};

test("encrypted backup round-trips all period and check-in fields", async () => {
  const encrypted = await encryptBackup(sampleBackup, passphrase);
  const restored = await decryptBackup(encrypted, passphrase);
  assert.deepEqual(restored, sampleBackup);
});

test("each backup uses a fresh random salt and AES-GCM nonce", async () => {
  const first = await encryptBackup(sampleBackup, passphrase);
  const second = await encryptBackup(sampleBackup, passphrase);
  const firstEnvelope = JSON.parse(first);
  const secondEnvelope = JSON.parse(second);

  assert.notEqual(firstEnvelope.kdf.salt, secondEnvelope.kdf.salt);
  assert.notEqual(firstEnvelope.cipher.iv, secondEnvelope.cipher.iv);
  assert.notEqual(firstEnvelope.ciphertext, secondEnvelope.ciphertext);
});

test("wrong passphrases and modified ciphertext are rejected", async () => {
  const encrypted = await encryptBackup(sampleBackup, passphrase);
  await assert.rejects(
    decryptBackup(encrypted, "different-backup-passphrase"),
    /passphrase is incorrect or the backup has been modified/,
  );

  const envelope = JSON.parse(encrypted);
  const ciphertext = Buffer.from(envelope.ciphertext, "base64");
  ciphertext[0] ^= 0x01;
  envelope.ciphertext = ciphertext.toString("base64");
  await assert.rejects(
    decryptBackup(JSON.stringify(envelope), passphrase),
    /passphrase is incorrect or the backup has been modified/,
  );
});

test("invalid calendar dates are rejected before encryption", async () => {
  const invalid = structuredClone(sampleBackup);
  invalid.periods[0].endDate = "2026-02-30";
  await assert.rejects(
    encryptBackup(invalid, passphrase),
    /invalid period record/,
  );
});
