<script lang="ts">
  import { Activity, CalendarCheck, BarChart3 } from "@lucide/svelte";
  import type { DashboardSnapshot } from "$lib/types";
  import { formatDate } from "$lib/utils";
  import { exportBackupDocument, restoreBackupDocument } from "$lib/api";
  import { decryptBackup, encryptBackup } from "$lib/backup";

  export let snapshot: DashboardSnapshot;
  export let onBackupRestored: (snapshot: DashboardSnapshot) => void;

  let exportPassphrase = "";
  let exportPassphraseAgain = "";
  let encryptedBackup = "";
  let importPayload = "";
  let importPassphrase = "";
  let backupBusy = false;
  let backupError = "";
  let backupStatus = "";

  async function createBackup() {
    backupError = "";
    backupStatus = "";
    encryptedBackup = "";
    if (exportPassphrase.length < 12) {
      backupError = "Use a backup passphrase with at least 12 characters.";
      return;
    }
    if (exportPassphrase !== exportPassphraseAgain) {
      backupError = "The backup passphrases do not match.";
      return;
    }

    backupBusy = true;
    try {
      const document = await exportBackupDocument();
      encryptedBackup = await encryptBackup(document, exportPassphrase);
      backupStatus = "Encrypted backup created. Copy the complete text below and store it somewhere separate from this device.";
      exportPassphrase = "";
      exportPassphraseAgain = "";
    } catch (error) {
      backupError = error instanceof Error ? error.message : String(error);
    } finally {
      backupBusy = false;
    }
  }

  async function restoreBackup() {
    backupError = "";
    backupStatus = "";
    if (!importPayload.trim()) {
      backupError = "Paste the encrypted Miora backup first.";
      return;
    }
    if (!importPassphrase) {
      backupError = "Enter the backup passphrase.";
      return;
    }
    if (!window.confirm("Restore replaces every period and check-in currently stored on this device. Continue?")) {
      return;
    }

    backupBusy = true;
    try {
      const document = await decryptBackup(importPayload, importPassphrase);
      const updated = await restoreBackupDocument(document);
      onBackupRestored(updated);
      backupStatus = "Backup restored. Your Miora history has been replaced with the restored records.";
      importPassphrase = "";
      importPayload = "";
      encryptedBackup = "";
    } catch (error) {
      backupError = error instanceof Error ? error.message : String(error);
    } finally {
      backupBusy = false;
    }
  }
</script>

<section class="screen insights-screen">
  <div class="screen-intro">
    <p class="eyebrow">YOUR PERSONAL HISTORY</p>
    <h1>Insights</h1>
    <p class="screen-subtitle">A clearer picture, built from your own tracking.</p>
  </div>

  <div class="insight-grid">
    <section class="surface-card insight-card">
      <div class="insight-icon"><CalendarCheck size={19} /></div>
      <span class="eyebrow">COMPLETED CYCLES</span>
      <strong>{snapshot.cyclesTracked}</strong>
      <p>Intervals between recorded starts</p>
    </section>

    <section class="surface-card insight-card">
      <div class="insight-icon"><Activity size={19} /></div>
      <span class="eyebrow">AVERAGE LENGTH</span>
      <strong>{snapshot.averageCycleLength ? Math.round(snapshot.averageCycleLength) + " days" : "—"}</strong>
      <p>{snapshot.averageCycleLength ? "Based on your completed intervals" : "More period history is needed"}</p>
    </section>

    <section class="surface-card insight-card">
      <div class="insight-icon"><BarChart3 size={19} /></div>
      <span class="eyebrow">DAILY CHECK-INS</span>
      <strong>{snapshot.checkinsTracked}</strong>
      <p>Days with a saved check-in</p>
    </section>
  </div>

  {#if snapshot.phase && snapshot.symptomPatterns.length > 0}
    <section class="surface-card history-card" aria-label="Symptom patterns for the current cycle day">
      <p class="eyebrow">SYMPTOM PATTERNS · CYCLE DAY {snapshot.phase.cycleDay}</p>
      <p class="section-description">Likelihood based on your saved check-ins recorded on this cycle day across your tracked history.</p>
      <div class="recorded-dates">
        {#each snapshot.symptomPatterns as pattern (pattern.code)}
          <div class="recorded-date">
            <span>{pattern.label}</span>
            <strong>{Math.round(pattern.probability * 100)}%</strong>
            <span class="insight-footnote">{pattern.observationsUsed} check-ins</span>
          </div>
        {/each}
      </div>
    </section>
  {/if}

  <section class="surface-card history-card">
    <p class="eyebrow">RECORDED PERIOD STARTS</p>
    {#if snapshot.periodStarts.length > 0}
      <div class="recorded-dates">
        {#each [...snapshot.periodStarts].reverse() as date}
          <div class="recorded-date">
            <span class="date-marker"></span>
            <span>{formatDate(date, { weekday: "long", day: "numeric", month: "long", year: "numeric" })}</span>
          </div>
        {/each}
      </div>
    {:else}
      <div class="empty-insight">
        <p>Your insights will take shape as you log period dates and daily check-ins.</p>
      </div>
    {/if}
  </section>


  <section class="surface-card history-card backup-card">
    <p class="eyebrow">PRIVATE DATA RECOVERY</p>
    <h2>Encrypted backup</h2>
    <p class="section-description">
      Export your period history and check-ins into a portable encrypted backup. Choose a passphrase you can remember; Miora cannot recover it.
      The backup uses AES-256-GCM with PBKDF2-SHA-256 key derivation.
    </p>

    <div class="backup-fields">
      <label>
        <span class="field-label">New backup passphrase</span>
        <input class="form-input" type="password" autocomplete="new-password"
          bind:value={exportPassphrase} placeholder="At least 12 characters" disabled={backupBusy} />
      </label>
      <label>
        <span class="field-label">Confirm passphrase</span>
        <input class="form-input" type="password" autocomplete="new-password"
          bind:value={exportPassphraseAgain} placeholder="Enter it again" disabled={backupBusy} />
      </label>
    </div>
    <button class="button button-primary" type="button" onclick={createBackup} disabled={backupBusy}>
      {backupBusy ? "Working…" : "Create encrypted backup"}
    </button>

    {#if encryptedBackup}
      <label class="backup-output">
        <span class="field-label">Encrypted backup text</span>
        <textarea class="backup-textarea" rows="6" readonly value={encryptedBackup}></textarea>
      </label>
      <p class="insight-footnote">Copy the entire text, including its opening and closing braces, and store it in a password manager or separate file. Do not store the passphrase beside it.</p>
    {/if}

    <div class="backup-divider"></div>
    <p class="eyebrow">RESTORE A BACKUP</p>
    <label class="backup-output">
      <span class="field-label">Encrypted backup text</span>
      <textarea class="backup-textarea" rows="6" bind:value={importPayload}
        placeholder="Paste the complete encrypted backup text here" disabled={backupBusy}></textarea>
    </label>
    <label class="backup-output">
      <span class="field-label">Backup passphrase</span>
      <input class="form-input" type="password" autocomplete="current-password"
        bind:value={importPassphrase} disabled={backupBusy} />
    </label>
    <button class="button button-primary" type="button" onclick={restoreBackup} disabled={backupBusy}>
      {backupBusy ? "Working…" : "Restore and replace current data"}
    </button>

    {#if backupError}
      <p class="backup-message backup-error" role="alert">{backupError}</p>
    {:else if backupStatus}
      <p class="backup-message" role="status">{backupStatus}</p>
    {/if}
  </section>

  <p class="insight-footnote">Miora only shows personal estimates when there is enough recorded history.</p>
</section>
