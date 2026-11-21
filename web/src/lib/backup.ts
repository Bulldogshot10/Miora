import type { BackupDocument } from "./types";

const BACKUP_FORMAT = "miora.encrypted-backup";
const BACKUP_VERSION = 1;
const KDF_ITERATIONS = 600_000;
const MAX_BACKUP_CHARACTERS = 32_000_000;
const BACKUP_AAD = new TextEncoder().encode("miora-encrypted-backup:v1");
const VALID_SYMPTOMS = new Set([
  "cramps",
  "bloating",
  "headache",
  "fatigue",
  "mood_changes",
]);

interface EncryptedBackupEnvelope {
  format: typeof BACKUP_FORMAT;
  version: typeof BACKUP_VERSION;
  kdf: {
    algorithm: "PBKDF2-SHA-256";
    iterations: number;
    salt: string;
  };
  cipher: {
    algorithm: "AES-256-GCM";
    iv: string;
  };
  ciphertext: string;
}

function bytesToBase64(bytes: Uint8Array): string {
  let binary = "";
  const chunkSize = 0x8000;
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    const slice = bytes.subarray(offset, Math.min(offset + chunkSize, bytes.length));
    binary += String.fromCharCode(...slice);
  }
  return btoa(binary);
}

function base64ToBytes(value: string): Uint8Array {
  const binary = atob(value);
  const output = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    output[index] = binary.charCodeAt(index);
  }
  return output;
}

function copyToArrayBuffer(bytes: Uint8Array): ArrayBuffer {
  const buffer = new ArrayBuffer(bytes.byteLength);
  new Uint8Array(buffer).set(bytes);
  return buffer;
}

function wipeArrayBuffer(buffer: ArrayBuffer): void {
  new Uint8Array(buffer).fill(0);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isIsoDate(value: unknown): value is string {
  if (typeof value !== "string" || !/^\d{4}-\d{2}-\d{2}$/.test(value)) return false;
  const [year, month, day] = value.split("-").map(Number);
  if (year < 1 || year > 9999) return false;
  const parsed = new Date(0);
  parsed.setUTCFullYear(year, month - 1, day);
  parsed.setUTCHours(0, 0, 0, 0);
  return parsed.getUTCFullYear() === year &&
    parsed.getUTCMonth() === month - 1 &&
    parsed.getUTCDate() === day;
}

function assertBackupDocument(value: unknown): asserts value is BackupDocument {
  if (
    !isRecord(value) ||
    value.format !== "miora-backup-data" ||
    value.version !== 1 ||
    !Array.isArray(value.periods) ||
    !Array.isArray(value.checkins) ||
    value.periods.length > 10_000 ||
    value.checkins.length > 100_000
  ) {
    throw new Error("The decrypted data is not a supported Miora backup.");
  }

  for (const entry of value.periods) {
    if (
      !isRecord(entry) ||
      !isIsoDate(entry.startDate) ||
      (entry.endDate !== null && !isIsoDate(entry.endDate)) ||
      (entry.lastObservation !== null && !isIsoDate(entry.lastObservation)) ||
      (entry.flowIntensity !== null &&
        (!Number.isInteger(entry.flowIntensity) ||
          (entry.flowIntensity as number) < 1 ||
          (entry.flowIntensity as number) > 3)) ||
      typeof entry.endConfirmed !== "boolean"
    ) {
      throw new Error("The decrypted backup contains an invalid period record.");
    }
  }

  for (const entry of value.checkins) {
    if (
      !isRecord(entry) ||
      !isIsoDate(entry.date) ||
      !Number.isInteger(entry.moodScore) ||
      (entry.moodScore as number) < 1 ||
      (entry.moodScore as number) > 5 ||
      !Number.isInteger(entry.energyScore) ||
      (entry.energyScore as number) < 1 ||
      (entry.energyScore as number) > 5 ||
      !Array.isArray(entry.symptoms) ||
      entry.symptoms.length > VALID_SYMPTOMS.size
    ) {
      throw new Error("The decrypted backup contains an invalid daily check-in.");
    }

    const seen = new Set<string>();
    for (const symptom of entry.symptoms) {
      if (
        !isRecord(symptom) ||
        typeof symptom.code !== "string" ||
        !VALID_SYMPTOMS.has(symptom.code) ||
        !Number.isInteger(symptom.severity) ||
        (symptom.severity as number) < 1 ||
        (symptom.severity as number) > 3 ||
        seen.has(symptom.code)
      ) {
        throw new Error("The decrypted backup contains an invalid symptom entry.");
      }
      seen.add(symptom.code);
    }
  }
}

async function deriveEncryptionKey(passphrase: string, salt: Uint8Array): Promise<CryptoKey> {
  const passwordBytes = new TextEncoder().encode(passphrase);
  const passwordBuffer = copyToArrayBuffer(passwordBytes);
  const saltBuffer = copyToArrayBuffer(salt);
  try {
    const material = await crypto.subtle.importKey(
      "raw",
      passwordBuffer,
      "PBKDF2",
      false,
      ["deriveKey"],
    );
    return await crypto.subtle.deriveKey(
      {
        name: "PBKDF2",
        salt: saltBuffer,
        iterations: KDF_ITERATIONS,
        hash: "SHA-256",
      },
      material,
      { name: "AES-GCM", length: 256 },
      false,
      ["encrypt", "decrypt"],
    );
  } finally {
    passwordBytes.fill(0);
    wipeArrayBuffer(passwordBuffer);
    wipeArrayBuffer(saltBuffer);
  }
}

function assertPassphrase(passphrase: string): void {
  if (passphrase.length < 12) {
    throw new Error("Use a backup passphrase with at least 12 characters.");
  }
  if (typeof crypto === "undefined" || !crypto.subtle || !crypto.getRandomValues) {
    throw new Error("Secure encryption is unavailable in this WebView.");
  }
}

export async function encryptBackup(
  document: BackupDocument,
  passphrase: string,
): Promise<string> {
  assertPassphrase(passphrase);
  assertBackupDocument(document);

  const salt = crypto.getRandomValues(new Uint8Array(16));
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const plaintext = new TextEncoder().encode(JSON.stringify(document));
  const ivBuffer = copyToArrayBuffer(iv);
  const aadBuffer = copyToArrayBuffer(BACKUP_AAD);
  const plaintextBuffer = copyToArrayBuffer(plaintext);

  try {
    const key = await deriveEncryptionKey(passphrase, salt);
    const ciphertext = await crypto.subtle.encrypt(
      { name: "AES-GCM", iv: ivBuffer, additionalData: aadBuffer, tagLength: 128 },
      key,
      plaintextBuffer,
    );

    const envelope: EncryptedBackupEnvelope = {
      format: BACKUP_FORMAT,
      version: BACKUP_VERSION,
      kdf: {
        algorithm: "PBKDF2-SHA-256",
        iterations: KDF_ITERATIONS,
        salt: bytesToBase64(salt),
      },
      cipher: {
        algorithm: "AES-256-GCM",
        iv: bytesToBase64(iv),
      },
      ciphertext: bytesToBase64(new Uint8Array(ciphertext)),
    };
    return JSON.stringify(envelope);
  } finally {
    plaintext.fill(0);
    salt.fill(0);
    iv.fill(0);
    wipeArrayBuffer(ivBuffer);
    wipeArrayBuffer(aadBuffer);
    wipeArrayBuffer(plaintextBuffer);
  }
}

export async function decryptBackup(
  encryptedText: string,
  passphrase: string,
): Promise<BackupDocument> {
  assertPassphrase(passphrase);
  if (!encryptedText.trim() || encryptedText.length > MAX_BACKUP_CHARACTERS) {
    throw new Error("The encrypted backup is empty or too large.");
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(encryptedText);
  } catch {
    throw new Error("The pasted text is not a valid encrypted backup.");
  }
  if (!isRecord(parsed) || parsed.format !== BACKUP_FORMAT || parsed.version !== BACKUP_VERSION) {
    throw new Error("This encrypted backup format or version is not supported.");
  }

  const kdf = parsed.kdf;
  const cipher = parsed.cipher;
  if (
    !isRecord(kdf) ||
    kdf.algorithm !== "PBKDF2-SHA-256" ||
    kdf.iterations !== KDF_ITERATIONS ||
    typeof kdf.salt !== "string" ||
    !isRecord(cipher) ||
    cipher.algorithm !== "AES-256-GCM" ||
    typeof cipher.iv !== "string" ||
    typeof parsed.ciphertext !== "string"
  ) {
    throw new Error("The encrypted backup header is invalid.");
  }

  let salt: Uint8Array;
  let iv: Uint8Array;
  let ciphertext: Uint8Array;
  try {
    salt = base64ToBytes(kdf.salt);
    iv = base64ToBytes(cipher.iv);
    ciphertext = base64ToBytes(parsed.ciphertext);
  } catch {
    throw new Error("The encrypted backup encoding is invalid.");
  }
  if (salt.length !== 16 || iv.length !== 12 || ciphertext.length < 16) {
    throw new Error("The encrypted backup is incomplete or corrupted.");
  }

  const ivBuffer = copyToArrayBuffer(iv);
  const aadBuffer = copyToArrayBuffer(BACKUP_AAD);
  const ciphertextBuffer = copyToArrayBuffer(ciphertext);
  try {
    const key = await deriveEncryptionKey(passphrase, salt);
    let plaintext: ArrayBuffer;
    try {
      plaintext = await crypto.subtle.decrypt(
        { name: "AES-GCM", iv: ivBuffer, additionalData: aadBuffer, tagLength: 128 },
        key,
        ciphertextBuffer,
      );
    } catch {
      throw new Error("The passphrase is incorrect or the backup has been modified.");
    }

    const bytes = new Uint8Array(plaintext);
    try {
      const document: unknown = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes));
      assertBackupDocument(document);
      return document;
    } catch (error) {
      if (error instanceof Error && error.message.startsWith("The decrypted backup")) {
        throw error;
      }
      throw new Error("The decrypted backup content is malformed.");
    } finally {
      bytes.fill(0);
    }
  } finally {
    salt.fill(0);
    iv.fill(0);
    ciphertext.fill(0);
    wipeArrayBuffer(ivBuffer);
    wipeArrayBuffer(aadBuffer);
    wipeArrayBuffer(ciphertextBuffer);
  }
}
