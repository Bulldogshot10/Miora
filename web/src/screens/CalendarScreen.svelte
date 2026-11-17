<script lang="ts">
  import { ChevronLeft, ChevronRight, Plus } from "@lucide/svelte";
  import type { DashboardSnapshot } from "$lib/types";
  import { localDateIso, formatDate } from "$lib/utils";

  export let snapshot: DashboardSnapshot;
  export let onSave: (date: string) => void;
  export let busy = false;

  let displayedMonth = new Date(new Date().getFullYear(), new Date().getMonth(), 1);
  let selectedDate = localDateIso();

  type CalendarCell = {
    key: string;
    day: number | null;
    date: string | null;
    isToday: boolean;
    isPeriodStart: boolean;
  };

  function makeCells(month: Date, periodStarts: string[]): CalendarCell[] {
    const year = month.getFullYear();
    const monthIndex = month.getMonth();
    const firstWeekday = new Date(year, monthIndex, 1).getDay();
    const offset = (firstWeekday + 6) % 7;
    const daysInMonth = new Date(year, monthIndex + 1, 0).getDate();
    const cells: CalendarCell[] = [];

    for (let i = 0; i < offset; i += 1) {
      cells.push({ key: "blank-" + i, day: null, date: null, isToday: false, isPeriodStart: false });
    }

    for (let day = 1; day <= daysInMonth; day += 1) {
      const date = [
        year,
        String(monthIndex + 1).padStart(2, "0"),
        String(day).padStart(2, "0"),
      ].join("-");
      cells.push({
        key: date,
        day,
        date,
        isToday: date === localDateIso(),
        isPeriodStart: periodStarts.includes(date),
      });
    }

    return cells;
  }

  $: monthTitle = new Intl.DateTimeFormat(undefined, { month: "long", year: "numeric" }).format(displayedMonth);
  $: cells = makeCells(displayedMonth, snapshot.periodStarts);

  function moveMonth(delta: number) {
    displayedMonth = new Date(displayedMonth.getFullYear(), displayedMonth.getMonth() + delta, 1);
  }
</script>

<section class="screen calendar-screen">
  <div class="screen-intro">
    <p class="eyebrow">YOUR HISTORY</p>
    <h1>Calendar</h1>
    <p class="screen-subtitle">Your recorded period starts, laid out by date.</p>
  </div>

  <section class="surface-card calendar-card" aria-label="Period calendar">
    <div class="calendar-heading">
      <div>
        <p class="eyebrow">MONTH VIEW</p>
        <h2>{monthTitle}</h2>
      </div>
      <div class="month-controls">
        <button class="icon-action" type="button" aria-label="Previous month" onclick={() => moveMonth(-1)}><ChevronLeft size={19} /></button>
        <button class="icon-action" type="button" aria-label="Next month" onclick={() => moveMonth(1)}><ChevronRight size={19} /></button>
      </div>
    </div>

    <div class="calendar-grid">
      {#each ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"] as weekday}
        <span class="weekday-label">{weekday}</span>
      {/each}

      {#each cells as cell (cell.key)}
        {#if cell.day === null}
          <span class="calendar-day calendar-day-empty" aria-hidden="true"></span>
        {:else}
          <span
            class="calendar-day"
            class:calendar-day-today={cell.isToday}
            class:calendar-day-period={cell.isPeriodStart}
            aria-label={cell.date + (cell.isPeriodStart ? ", period start" : "") + (cell.isToday ? ", today" : "")}
          >
            {cell.day}
          </span>
        {/if}
      {/each}
    </div>

    <div class="calendar-legend">
      <span><i class="legend-dot legend-period"></i> Period start</span>
      <span><i class="legend-dot legend-today"></i> Today</span>
    </div>
  </section>

  <section class="surface-card log-card">
    <div class="section-heading">
      <div>
        <p class="eyebrow">ADD TO YOUR HISTORY</p>
        <h2>Log a period start</h2>
      </div>
      <span class="action-icon action-icon-rose"><Plus size={20} /></span>
    </div>
    <p class="section-description">Choose the first day of bleeding. You can add older dates too.</p>
    <label class="field-label" for="period-date">First day</label>
    <input id="period-date" class="form-input" type="date" bind:value={selectedDate} max={localDateIso()} />
    <button class="button button-primary button-full" type="button" disabled={busy || !selectedDate} onclick={() => onSave(selectedDate)}>
      {busy ? "Saving…" : "Save period start"}
    </button>
  </section>

  <section class="history-summary">
    <span class="history-count">{snapshot.periodStarts.length}</span>
    <div>
      <strong>{snapshot.periodStarts.length === 1 ? "One period start recorded" : snapshot.periodStarts.length + " period starts recorded"}</strong>
      <p>{snapshot.lastPeriodStart ? "Most recent: " + formatDate(snapshot.lastPeriodStart, { year: "numeric" }) : "Your cycle history will appear here as you log dates."}</p>
    </div>
  </section>
</section>
