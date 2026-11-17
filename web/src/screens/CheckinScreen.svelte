<script lang="ts">
  import { onMount } from "svelte";
  import { Check, CheckCircle2, HeartPulse, LoaderCircle, Zap, BookOpen } from "@lucide/svelte";
  import type { DailyCheckinInput, SymptomInput } from "$lib/types";
  import { symptomGuidance } from "$lib/symptom-guidance";
  import { localDateIso, formatDate } from "$lib/utils";

  export let onSave: (input: DailyCheckinInput) => void | Promise<void>;
  export let onLoad: (date: string) => Promise<DailyCheckinInput | null>;
  export let busy = false;

  let selectedDate = localDateIso();
  let moodScore = 3;
  let energyScore = 3;
  let selectedSymptoms: Record<string, number> = {};
  let loadingDate = false;
  let hasExistingCheckin = false;
  let hasUnsavedChanges = false;
  let dateMessage = "";
  let loadSequence = 0;
  let submitState: "idle" | "success" | "error" = "idle";
  let submitError = "";

  const moods = [
    { score: 1, label: "Low" },
    { score: 2, label: "Uneasy" },
    { score: 3, label: "Okay" },
    { score: 4, label: "Good" },
    { score: 5, label: "Great" },
  ];

  const severities = [
    { value: 1, label: "Mild" },
    { value: 2, label: "Moderate" },
    { value: 3, label: "Severe" },
  ];

  const symptoms = [
    { code: "cramps", label: "Cramps" },
    { code: "bloating", label: "Bloating" },
    { code: "headache", label: "Headache" },
    { code: "fatigue", label: "Fatigue" },
    { code: "mood_changes", label: "Mood changes" },
  ];

  onMount(() => {
    void loadSelectedDate();
    window.addEventListener("miora:refresh-checkin", handleExternalRefresh);
    return () => window.removeEventListener("miora:refresh-checkin", handleExternalRefresh);
  });

  function handleExternalRefresh() {
    if (!hasUnsavedChanges && !busy) {
      void loadSelectedDate();
    }
  }

  async function loadSelectedDate() {
    hasUnsavedChanges = false;
    const sequence = ++loadSequence;
    loadingDate = true;
    submitState = "idle";
    submitError = "";
    dateMessage = "";

    try {
      const existing = await onLoad(selectedDate);
      if (sequence !== loadSequence) return;

      if (existing) {
        moodScore = existing.moodScore;
        energyScore = existing.energyScore;
        selectedSymptoms = Object.fromEntries(
          existing.symptoms.map((symptom) => [symptom.code, symptom.severity]),
        );
        hasExistingCheckin = true;
        dateMessage = "Saved check-in loaded. Saving again will update this date.";
      } else {
        moodScore = 3;
        energyScore = 3;
        selectedSymptoms = {};
        hasExistingCheckin = false;
        dateMessage = "No check-in saved for this date yet.";
      }
    } catch (error) {
      if (sequence !== loadSequence) return;
      dateMessage = error instanceof Error ? error.message : String(error);
      hasExistingCheckin = false;
    } finally {
      if (sequence === loadSequence) loadingDate = false;
    }
  }

  function handleDateChange(event: Event) {
    const value = (event.currentTarget as HTMLInputElement).value;
    if (!value) return;
    selectedDate = value;
    void loadSelectedDate();
  }

  function clearFeedback() {
    if (submitState !== "idle") {
      submitState = "idle";
      submitError = "";
    }
  }

  function toggleSymptom(code: string) {
    const next = { ...selectedSymptoms };
    if (code in next) {
      delete next[code];
    } else {
      next[code] = 1;
    }
    selectedSymptoms = next;
    hasUnsavedChanges = true;
    clearFeedback();
  }

  function setSeverity(code: string, severity: number) {
    selectedSymptoms = { ...selectedSymptoms, [code]: severity };
    hasUnsavedChanges = true;
    clearFeedback();
  }

  async function submit() {
    if (busy || loadingDate || !selectedDate) return;

    const entries: SymptomInput[] = Object.entries(selectedSymptoms).map(([code, severity]) => ({
      code,
      severity,
    }));

    submitState = "idle";
    submitError = "";

    try {
      await onSave({
        date: selectedDate,
        moodScore,
        energyScore,
        symptoms: entries,
      });
      hasExistingCheckin = true;
      hasUnsavedChanges = false;
      submitState = "success";
      dateMessage = "Saved for " + formatDate(selectedDate, { year: "numeric" }) + ".";
    } catch (error) {
      submitError = error instanceof Error ? error.message : String(error);
      submitState = "error";
    }
  }
</script>

<section class="screen checkin-screen">
  <div class="screen-intro">
    <p class="eyebrow">A MOMENT FOR YOURSELF</p>
    <h1>Daily check-in</h1>
    <p class="screen-subtitle">Record how you felt on a specific day. Past dates can be added or updated.</p>
  </div>

  <section class="surface-card checkin-card checkin-date-card">
    <div class="section-heading">
      <div>
        <p class="eyebrow">CHECK-IN DATE</p>
        <h2>Which day are we tracking?</h2>
      </div>
      <span class="action-icon action-icon-lilac"><BookOpen size={20} /></span>
    </div>
    <label class="field-label" for="checkin-date">Date</label>
    <input
      id="checkin-date"
      class="form-input"
      type="date"
      value={selectedDate}
      max={localDateIso()}
      onchange={handleDateChange}
      disabled={busy}
    />
    <p class="date-record-message" class:date-record-saved={hasExistingCheckin}>
      {#if loadingDate}
        Loading saved check-in…
      {:else}
        {dateMessage}
      {/if}
    </p>
  </section>

  <section class="surface-card checkin-card">
    <div class="section-heading">
      <div class="action-icon action-icon-lilac"><HeartPulse size={21} /></div>
      <div class="section-heading-copy">
        <p class="eyebrow">01 · MOOD</p>
        <h2>How were you feeling?</h2>
      </div>
    </div>
    <div class="choice-grid mood-grid" role="group" aria-label="Select your mood">
      {#each moods as mood}
        <button
          class="choice-button"
          class:selected={moodScore === mood.score}
          type="button"
          aria-pressed={moodScore === mood.score}
          disabled={loadingDate || busy}
          onclick={() => {
            moodScore = mood.score;
            hasUnsavedChanges = true;
            clearFeedback();
          }}
        >
          <span>{mood.label}</span>
          {#if moodScore === mood.score}<Check size={15} />{/if}
        </button>
      {/each}
    </div>
  </section>

  <section class="surface-card checkin-card">
    <div class="section-heading">
      <div class="action-icon action-icon-sand"><Zap size={21} /></div>
      <div class="section-heading-copy">
        <p class="eyebrow">02 · ENERGY</p>
        <h2>What was your energy like?</h2>
      </div>
      <strong class="range-value">{energyScore}/5</strong>
    </div>
    <input
      class="range-input"
      type="range"
      min="1"
      max="5"
      step="1"
      bind:value={energyScore}
      oninput={() => { hasUnsavedChanges = true; clearFeedback(); }}
      disabled={loadingDate || busy}
      aria-label="Energy level from 1 to 5"
    />
    <div class="range-labels"><span>Very low</span><span>Very high</span></div>
  </section>

  <section class="surface-card checkin-card">
    <div class="section-heading">
      <div>
        <p class="eyebrow">03 · BODY & MIND</p>
        <h2>Any symptoms on this date?</h2>
        <p class="section-description">Choose a symptom to reveal its intensity and practical, medically sourced guidance.</p>
      </div>
    </div>

    <div class="symptom-list">
      {#each symptoms as symptom}
        <div class="symptom-row" class:chosen={symptom.code in selectedSymptoms}>
          <button
            type="button"
            class="symptom-toggle"
            aria-pressed={symptom.code in selectedSymptoms}
            disabled={loadingDate || busy}
            onclick={() => toggleSymptom(symptom.code)}
          >
            <span class="symptom-check">{#if symptom.code in selectedSymptoms}<Check size={14} />{/if}</span>
            <span>{symptom.label}</span>
            <span class="symptom-state">{symptom.code in selectedSymptoms ? "Selected" : "Add"}</span>
          </button>

          {#if symptom.code in selectedSymptoms}
            <div class="severity-control" role="group" aria-label={symptom.label + " intensity"}>
              <span class="severity-label">Intensity</span>
              <div class="severity-options">
                {#each severities as severity}
                  <button
                    class="severity-button"
                    class:active={selectedSymptoms[symptom.code] === severity.value}
                    type="button"
                    aria-pressed={selectedSymptoms[symptom.code] === severity.value}
                    disabled={loadingDate || busy}
                    onclick={() => setSeverity(symptom.code, severity.value)}
                  >
                    {severity.label}
                  </button>
                {/each}
              </div>
            </div>
          {/if}
        </div>
      {/each}
    </div>
  </section>

  {#if symptoms.some((symptom) => symptom.code in selectedSymptoms)}
    <section class="symptom-guidance-section">
      <div class="section-heading guidance-heading">
        <div>
          <p class="eyebrow">EVIDENCE-BASED SELF-CARE</p>
          <h2>What may help</h2>
          <p class="section-description">Guidance for the symptoms selected above.</p>
        </div>
      </div>

      {#each symptoms.filter((symptom) => symptom.code in selectedSymptoms) as symptom}
        {@const guidance = symptomGuidance[symptom.code]}
        <details class="surface-card symptom-guide-card">
          <summary>
            <span>{guidance.title}</span>
            <span class="guide-summary-action">View guidance</span>
          </summary>
          <p class="guide-summary">{guidance.summary}</p>
          <p class="eyebrow guide-eyebrow">WHAT MAY HELP</p>
          <ul class="guide-actions">
            {#each guidance.actions as action}
              <li>{action}</li>
            {/each}
          </ul>
          <div class="guide-care-note">
            <strong>When to seek medical advice</strong>
            <p>{guidance.seekCare}</p>
          </div>
          <div class="guide-sources">
            <span class="eyebrow">SOURCES</span>
            {#each guidance.sources as source}
              <a href={source.url} target="_blank" rel="noreferrer">{source.label}</a>
            {/each}
          </div>
        </details>
      {/each}
    </section>
  {/if}

  <button
    class="button button-primary button-full save-checkin"
    type="button"
    disabled={busy || loadingDate || !selectedDate}
    aria-busy={busy}
    onclick={submit}
  >
    {#if busy}
      <LoaderCircle class="button-spinner" size={17} />
      Saving check-in…
    {:else if submitState === "success"}
      <CheckCircle2 size={18} />
      Update check-in
    {:else}
      {hasExistingCheckin ? "Update check-in" : "Save check-in"}
    {/if}
  </button>

  {#if submitState === "success"}
    <div class="save-feedback feedback-success" role="status" aria-live="polite">
      <CheckCircle2 size={21} />
      <div>
        <strong>Check-in saved</strong>
        <p>{dateMessage} The calendar has been updated.</p>
      </div>
    </div>
  {:else if submitState === "error"}
    <div class="save-feedback feedback-error" role="alert">
      <div>
        <strong>Couldn't save this check-in</strong>
        <p>{submitError || "Something went wrong. Please try again."}</p>
      </div>
    </div>
  {/if}

  <p class="privacy-note">Daily entries are stored privately on this device.</p>
</section>
