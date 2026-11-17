<script lang="ts">
  import { Check, CheckCircle2, HeartPulse, LoaderCircle, Zap } from "@lucide/svelte";
  import type { DailyCheckinInput, SymptomInput } from "$lib/types";
  import { localDateIso } from "$lib/utils";

  export let onSave: (input: DailyCheckinInput) => void | Promise<void>;
  export let busy = false;

  let moodScore = 3;
  let energyScore = 3;
  let selectedSymptoms: Record<string, number> = {};
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
    clearFeedback();
  }

  function setSeverity(code: string, severity: number) {
    selectedSymptoms = { ...selectedSymptoms, [code]: severity };
    clearFeedback();
  }

  async function submit() {
    if (busy) return;

    const entries: SymptomInput[] = Object.entries(selectedSymptoms).map(([code, severity]) => ({
      code,
      severity,
    }));

    submitState = "idle";
    submitError = "";

    try {
      await onSave({
        date: localDateIso(),
        moodScore,
        energyScore,
        symptoms: entries,
      });
      submitState = "success";
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
    <p class="screen-subtitle">A few quick notes can help you recognise patterns over time.</p>
  </div>

  <section class="surface-card checkin-card">
    <div class="section-heading">
      <div class="action-icon action-icon-lilac"><HeartPulse size={21} /></div>
      <div class="section-heading-copy">
        <p class="eyebrow">01 · MOOD</p>
        <h2>How are you feeling?</h2>
      </div>
    </div>
    <div class="choice-grid mood-grid" role="group" aria-label="Select your mood">
      {#each moods as mood}
        <button
          class="choice-button"
          class:selected={moodScore === mood.score}
          type="button"
          aria-pressed={moodScore === mood.score}
          onclick={() => {
            moodScore = mood.score;
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
        <h2>What's your energy like?</h2>
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
      oninput={clearFeedback}
      aria-label="Energy level from 1 to 5"
    />
    <div class="range-labels"><span>Very low</span><span>Very high</span></div>
  </section>

  <section class="surface-card checkin-card">
    <div class="section-heading">
      <div>
        <p class="eyebrow">03 · BODY & MIND</p>
        <h2>Any symptoms today?</h2>
        <p class="section-description">Choose a symptom to reveal its intensity options.</p>
      </div>
    </div>

    <div class="symptom-list">
      {#each symptoms as symptom}
        <div class="symptom-row" class:chosen={symptom.code in selectedSymptoms}>
          <button
            type="button"
            class="symptom-toggle"
            aria-pressed={symptom.code in selectedSymptoms}
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

  <button
    class="button button-primary button-full save-checkin"
    type="button"
    disabled={busy}
    aria-busy={busy}
    onclick={submit}
  >
    {#if busy}
      <LoaderCircle class="button-spinner" size={17} />
      Saving check-in…
    {:else if submitState === "success"}
      <CheckCircle2 size={18} />
      Update today's check-in
    {:else}
      Save today's check-in
    {/if}
  </button>

  {#if submitState === "success"}
    <div class="save-feedback feedback-success" role="status" aria-live="polite">
      <CheckCircle2 size={21} />
      <div>
        <strong>Check-in saved</strong>
        <p>Today's mood, energy and symptoms are stored on this device.</p>
      </div>
    </div>
  {:else if submitState === "error"}
    <div class="save-feedback feedback-error" role="alert">
      <div>
        <strong>Couldn't save your check-in</strong>
        <p>{submitError || "Something went wrong. Please try again."}</p>
      </div>
    </div>
  {/if}

  <p class="privacy-note">Your check-in is stored privately on this device.</p>
</section>
