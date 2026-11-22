<script lang="ts">
  import { onMount } from "svelte";
  import { ArrowRight, CalendarDays, ClipboardCheck, Droplets, Plus } from "@lucide/svelte";
  import type { DashboardSnapshot, Tab } from "$lib/types";
  import { formatDate } from "$lib/utils";

  let confirmingBleedingEnd = false;
  let lastBleedingDate = "";

  export let snapshot: DashboardSnapshot;
  export let onNavigate: (tab: Tab) => void;
  export let onBleedingStatus: (stillBleeding: boolean, lastBleedingDate: string | null) => void;
  export let busy = false;

  let todayLabel = formatTodayLabel();

  function formatTodayLabel(): string {
    return new Intl.DateTimeFormat(undefined, {
      weekday: "long",
      day: "numeric",
      month: "long",
    }).format(new Date());
  }

  onMount(() => {
    const updateTodayLabel = () => {
      todayLabel = formatTodayLabel();
    };
    window.addEventListener("miora:refresh-checkin", updateTodayLabel);
    return () => window.removeEventListener("miora:refresh-checkin", updateTodayLabel);
  });
</script>

<section class="screen home-screen">
  <div class="screen-intro">
    <p class="eyebrow">{todayLabel}</p>
    <h1>Your cycle, in perspective.</h1>
    <p class="screen-subtitle">A calm place to notice patterns and keep track of how you feel.</p>
  </div>

  <div class="home-layout">
    <section class="cycle-card" aria-labelledby="cycle-title">
      <div class="cycle-card-top">
        <div>
          <p class="eyebrow eyebrow-inverted">CYCLE OVERVIEW</p>
          <h2 id="cycle-title">
            {#if snapshot.cycleDay}
              Day {snapshot.cycleDay}
            {:else}
              Start with day one
            {/if}
          </h2>
        </div>
        <span class="cycle-symbol" aria-hidden="true"><Droplets size={25} strokeWidth={1.7} /></span>
      </div>

      {#if snapshot.lastPeriodStart}
        <p class="cycle-detail">Your last period started on {formatDate(snapshot.lastPeriodStart, { year: "numeric" })}.</p>
        {#if snapshot.earliestNextPeriodStart && snapshot.latestNextPeriodStart}
          <div class="prediction-block">
            <span class="prediction-label">NEXT PERIOD WINDOW</span>
            <strong>{formatDate(snapshot.earliestNextPeriodStart)} – {formatDate(snapshot.latestNextPeriodStart)}</strong>
            <span class="prediction-caption">
              Conditional forecast from {snapshot.cyclesTracked} completed cycle intervals and the current cycle age.
            </span>
          </div>
        {:else}
          <div class="prediction-block prediction-muted">
            <span class="prediction-label">PERSONAL ESTIMATE</span>
            <strong>Still learning your rhythm</strong>
            <span class="prediction-caption">More logged period starts are needed before Miora estimates a window.</span>
          </div>
        {/if}
      {:else}
        <p class="cycle-detail">Log the first day of the most recent period. Miora will build estimates from your history.</p>
        <button class="button button-on-dark" type="button" onclick={() => onNavigate("calendar")}>
          <Plus size={17} /> Log period start
        </button>
      {/if}

      <div class="cycle-card-foot">
        <span>Private by design</span>
        <span>Stored on this device</span>
      </div>
    </section>

    <div class="home-side">
      <section class="surface-card action-card">
        <div class="action-icon action-icon-lilac"><ClipboardCheck size={21} strokeWidth={1.8} /></div>
        <div class="action-copy">
          <p class="eyebrow">DAILY CHECK-IN</p>
          <h2>How are you feeling today?</h2>
          <p>Record mood, energy and symptoms.</p>
        </div>
        <button class="icon-action" type="button" aria-label="Open daily check-in" onclick={() => onNavigate("checkin")}>
          <ArrowRight size={19} />
        </button>
      </section>

      <section class="surface-card action-card">
        <div class="action-icon action-icon-rose"><CalendarDays size={21} strokeWidth={1.8} /></div>
        <div class="action-copy">
          <p class="eyebrow">PERIOD HISTORY</p>
          <h2>{snapshot.periodStarts.length} {snapshot.periodStarts.length === 1 ? "start logged" : "starts logged"}</h2>
          <p>{snapshot.lastPeriodStart ? "View your dates and cycle history." : "Add a date to start your history."}</p>
        </div>
        <button class="icon-action" type="button" aria-label="Open calendar" onclick={() => onNavigate("calendar")}>
          <ArrowRight size={19} />
        </button>
      </section>
    </div>
  </div>

  {#if snapshot.phase}
    <section class="phase-card" aria-label="Estimated current cycle phase">
      <div class="phase-card-top">
        <p class="eyebrow">CURRENT CYCLE PHASE</p>
        <span class="phase-pill">{snapshot.phase.label}</span>
      </div>
      <h3>
        Cycle day {snapshot.phase.cycleDay}
        {#if snapshot.phase.estimatedCycleLength > 0}
          · {snapshot.phase.estimatedCycleLength}-day model estimate
        {:else}
          · Cycle length still learning
        {/if}
      </h3>
      <p>{snapshot.phase.description}</p>

      <div class="phase-probabilities" aria-label="Phase probability distribution">
        <div class="phase-probability">
          <span>Menstrual</span>
          <strong>{Math.round(snapshot.phase.probabilities.menstrual * 100)}%</strong>
        </div>
        <div class="phase-probability">
          <span>Follicular</span>
          <strong>{Math.round(snapshot.phase.probabilities.follicular * 100)}%</strong>
        </div>
        <div class="phase-probability">
          <span>Ovulation</span>
          <strong>{Math.round(snapshot.phase.probabilities.ovulationWindow * 100)}%</strong>
        </div>
        <div class="phase-probability">
          <span>Luteal</span>
          <strong>{Math.round(snapshot.phase.probabilities.luteal * 100)}%</strong>
        </div>
        <div class="phase-probability">
          <span>Uncertain</span>
          <strong>{Math.round(snapshot.phase.probabilities.uncertain * 100)}%</strong>
        </div>
      </div>
      <p class="phase-basis">
        {snapshot.phase.isPersonalized ? "Personalized model." : "Initial Bayesian priors."}
        {snapshot.phase.cycleHistoryAvailable
          ? " Cycle history: " + snapshot.phase.completedIntervalsUsed + " intervals."
          : " Cycle history: population prior."}
        {snapshot.phase.bleedingHistoryAvailable
          ? " Bleeding history: " + snapshot.phase.completedBleedingDurationsUsed + " confirmed durations."
          : " Bleeding history: population prior."}
        Most likely phase: {Math.round(snapshot.phase.confidence * 100)}%.
      </p>
    </section>
  {:else}
    <section class="phase-card">
      <p class="eyebrow">CYCLE PHASE</p>
      <h3>Log a period start to begin</h3>
      <p>Miora will build a phase distribution from your period dates and confirmed bleeding durations.</p>
    </section>
  {/if}

  {#if snapshot.bleedingPrompt}
    <section class="bleeding-prompt-card" aria-labelledby="bleeding-question">
      <p class="eyebrow">BLEEDING CHECK · {snapshot.bleedingPrompt.completedBleedingDurationsUsed > 0 ? "PERSONAL ESTIMATE" : "FIRST-CYCLE BASELINE"}</p>
      <h3 id="bleeding-question">Are you still bleeding today?</h3>
      <p>
        Your current model expects bleeding to last around
        {snapshot.bleedingPrompt.predictedBleedingDurationDays} days.
        Your answer updates your personal bleeding-duration model.
      </p>
      {#if confirmingBleedingEnd}
        <label class="field-label" for="last-bleeding-date">What was your last day of bleeding?</label>
        <input
          id="last-bleeding-date"
          class="form-input"
          type="date"
          min={snapshot.bleedingPrompt.periodStart}
          max={snapshot.bleedingPrompt.checkDate}
          bind:value={lastBleedingDate}
        />
        <div class="bleeding-prompt-actions">
          <button class="button button-outline" type="button" disabled={busy} onclick={() => { confirmingBleedingEnd = false; lastBleedingDate = ""; }}>
            Back
          </button>
          <button
            class="button button-primary"
            type="button"
            disabled={busy || !lastBleedingDate}
            onclick={() => {
              const confirmedDate = lastBleedingDate;
              confirmingBleedingEnd = false;
              lastBleedingDate = "";
              onBleedingStatus(false, confirmedDate);
            }}
          >
            Confirm last bleeding day
          </button>
        </div>
      {:else}
        <div class="bleeding-prompt-actions">
          <button class="button button-outline" type="button" disabled={busy} onclick={() => onBleedingStatus(true, null)}>
            Yes, still bleeding
          </button>
          <button class="button button-primary" type="button" disabled={busy} onclick={() => { confirmingBleedingEnd = true; }}>
            No, it stopped
          </button>
        </div>
      {/if}
    </section>
  {/if}

  <section class="stats-row" aria-label="Cycle statistics">
    <div class="stat-item">
      <span class="stat-value">{snapshot.cyclesTracked}</span>
      <span class="stat-label">Completed cycles</span>
    </div>
    <div class="stat-divider"></div>
    <div class="stat-item">
      <span class="stat-value">{snapshot.averageCycleLength ? Math.round(snapshot.averageCycleLength) + "d" : "—"}</span>
      <span class="stat-label">Average cycle</span>
    </div>
    <div class="stat-divider"></div>
    <div class="stat-item">
      <span class="stat-value">{snapshot.checkinsTracked}</span>
      <span class="stat-label">Daily check-ins</span>
    </div>
  </section>
</section>
