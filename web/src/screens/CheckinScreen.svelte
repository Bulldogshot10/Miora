<script lang="ts">
  import { Check, HeartPulse, Zap } from "@lucide/svelte";
  import type { DailyCheckinInput, SymptomInput } from "$lib/types";
  import { localDateIso } from "$lib/utils";

  export let onSave: (input: DailyCheckinInput) => void | Promise<void>;
  export let busy = false;

  let moodScore = 3;
  let energyScore = 3;
  let selectedSymptoms: Record<string, number> = {};

  const moods = [
    { score: 1, label: "Low" },
    { score: 2, label: "Uneasy" },
    { score: 3, label: "Okay" },
    { score: 4, label: "Good" },
    { score: 5, label: "Great" },
  ];

  const symptoms = [
    { code: "cramps", label: "Cramps" },
    { code: "bloating", label: "Bloating" },
    { code: "headache", label: "Headache" },
    { code: "fatigue", label: "Fatigue" },
    { code: "mood_changes", label: "Mood changes" },
  ];

  function toggleSymptom(code: string) {
    const next = { ...selectedSymptoms };
    if (code in next) {
      delete next[code];
    } else {
      next[code] = 1;
    }
    selectedSymptoms = next;
  }

  function setSeverity(code: string, event: Event) {
    const value = Number((event.currentTarget as HTMLSelectElement).value);
    selectedSymptoms = { ...selectedSymptoms, [code]: value };
  }

  async function submit() {
    const entries: SymptomInput[] = Object.entries(selectedSymptoms).map(([code, severity]) => ({
      code,
      severity,
    }));

    await onSave({
      date: localDateIso(),
      moodScore,
      energyScore,
      symptoms: entries,
    });
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
          onclick={() => (moodScore = mood.score)}
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
      aria-label="Energy level from 1 to 5"
    />
    <div class="range-labels"><span>Very low</span><span>Very high</span></div>
  </section>

  <section class="surface-card checkin-card">
    <div class="section-heading">
      <div>
        <p class="eyebrow">03 · BODY & MIND</p>
        <h2>Any symptoms today?</h2>
        <p class="section-description">Select what applies and choose its intensity.</p>
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
          </button>

          {#if symptom.code in selectedSymptoms}
            <select
              class="severity-select"
              aria-label={symptom.label + " severity"}
              value={selectedSymptoms[symptom.code]}
              onchange={(event) => setSeverity(symptom.code, event)}
            >
              <option value="1">Mild</option>
              <option value="2">Moderate</option>
              <option value="3">Severe</option>
            </select>
          {/if}
        </div>
      {/each}
    </div>
  </section>

  <button class="button button-primary button-full save-checkin" type="button" disabled={busy} onclick={submit}>
    {busy ? "Saving check-in…" : "Save today's check-in"}
  </button>
  <p class="privacy-note">Your check-in is stored privately on this device.</p>
</section>
