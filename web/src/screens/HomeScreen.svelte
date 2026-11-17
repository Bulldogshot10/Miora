<script lang="ts">
  import { ArrowRight, CalendarDays, ClipboardCheck, Droplets, Plus } from "@lucide/svelte";
  import type { DashboardSnapshot, Tab } from "$lib/types";
  import { formatDate } from "$lib/utils";

  export let snapshot: DashboardSnapshot;
  export let onNavigate: (tab: Tab) => void;

  const todayLabel = new Intl.DateTimeFormat(undefined, {
    weekday: "long",
    day: "numeric",
    month: "long",
  }).format(new Date());
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
            <span class="prediction-caption">Estimated from {snapshot.cyclesTracked} completed cycle intervals.</span>
          </div>
        {:else}
          <div class="prediction-block prediction-muted">
            <span class="prediction-label">PERSONAL ESTIMATE</span>
            <strong>Still learning your rhythm</strong>
            <span class="prediction-caption">More logged period starts are needed before Miora estimates a window.</span>
          </div>
        {/if}
      {:else}
        <p class="cycle-detail">Log the first day of your most recent period. Miora will build estimates from your own history.</p>
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
