<script lang="ts">
  import { Activity, CalendarCheck, BarChart3 } from "@lucide/svelte";
  import type { DashboardSnapshot } from "$lib/types";
  import { formatDate } from "$lib/utils";

  export let snapshot: DashboardSnapshot;
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

  <p class="insight-footnote">Miora only shows personal estimates when there is enough recorded history.</p>
</section>
