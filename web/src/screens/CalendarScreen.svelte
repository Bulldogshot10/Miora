<script lang="ts">
  import { ChevronLeft, ChevronRight, Plus } from "@lucide/svelte";
  import type { DashboardSnapshot } from "$lib/types";
  import { localDateIso, formatDate } from "$lib/utils";

  export let snapshot: DashboardSnapshot;
  export let onSave: (date: string, endDate: string | null) => void;
  export let busy = false;

  let displayedMonth = new Date(new Date().getFullYear(), new Date().getMonth(), 1);
  let selectedDate = localDateIso();
  let selectedEndDate = "";

  type CalendarCell = {
    key: string;
    day: number | null;
    date: string | null;
    isToday: boolean;
    isPeriodStart: boolean;
    isCheckin: boolean;
  };

  function makeCells(
    month: Date,
    periodStarts: string[],
    checkinDates: string[],
  ): CalendarCell[] {
    const year = month.getFullYear();
    const monthIndex = month.getMonth();
    const firstWeekday = new Date(year, monthIndex, 1).getDay();
    const offset = (firstWeekday + 6) % 7;
    const daysInMonth = new Date(year, monthIndex + 1, 0).getDate();
    const cells: CalendarCell[] = [];

    for (let i = 0; i < offset; i += 1) {
      cells.push({
        key: "blank-" + i,
        day: null,
        date: null,
        isToday: false,
        isPeriodStart: false,
        isCheckin: false,
      });
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
        isCheckin: checkinDates.includes(date),
      });
    }

    return cells;
  }

  $: monthTitle = new Intl.DateTimeFormat(undefined, {
    month: "long",
    year: "numeric",
  }).format(displayedMonth);
  $: cells = makeCells(displayedMonth, snapshot.periodStarts, snapshot.checkinDates);

  function moveMonth(delta: number) {
    displayedMonth = new Date(
      displayedMonth.getFullYear(),
      displayedMonth.getMonth() + delta,
      1,
    );
  }

  function handleStartDateChange(event: Event) {
    selectedDate = (event.currentTarget as HTMLInputElement).value;
    if (selectedEndDate && selectedEndDate < selectedDate) {
      selectedEndDate = "";
    }
  }
</script>

<section class="screen calendar-screen">
  <div class="screen-intro">
    <p class="eyebrow">YOUR HISTORY</p>
    <h1>Calendar</h1>
    <p class="screen-subtitle">Period dates and completed daily check-ins in one view.</p>
  </div>

  <section class="surface-card calendar-card" aria-label="Period and check-in calendar">
    <div class="calendar-heading">
      <div>
        <p class="eyebrow">MONTH VIEW</p>
        <h2>{monthTitle}</h2>
      </div>
      <div class="month-controls">
        <button class="icon-action" type="button" aria-label="Previous month" onclick={() => moveMonth(-1)}>
          <ChevronLeft size={19} />
        </button>
        <button class="icon-action" type="button" aria-label="Next month" onclick={() => moveMonth(1)}>
          <ChevronRight size={19} />
        </button>
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
            class:calendar-day-checkin={cell.isCheckin}
            aria-label={
              cell.date +
              (cell.isPeriodStart ? ", period start" : "") +
              (cell.isCheckin ? ", daily check-in saved" : "") +
              (cell.isToday ? ", today" : "")
            }
          >
            {cell.day}
          </span>
        {/if}
      {/each}
    </div>

    <div class="calendar-legend">
      <span><i class="legend-dot legend-period"></i> Period start</span>
      <span><i class="legend-dot legend-checkin"></i> Check-in saved</span>
      <span><i class="legend-dot legend-today"></i> Today</span>
    </div>
  </section>

  <section class="surface-card log-card">
    <div class="section-heading">
      <div>
        <p class="eyebrow">ADD TO YOUR HISTORY</p>
        <h2>Log a period</h2>
      </div>
      <span class="action-icon action-icon-rose"><Plus size={20} /></span>
    </div>
    <p class="section-description">
      Add the first day and, when it has finished, the last day of bleeding. This helps Miora estimate the menstrual phase more accurately.
    </p>

    <label class="field-label" for="period-date">First day of bleeding</label>
    <input
      id="period-date"
      class="form-input"
      type="date"
      value={selectedDate}
      max={localDateIso()}
      onchange={handleStartDateChange}
    />

    <label class="field-label optional-date-label" for="period-end-date">Last day of bleeding <span>Optional</span></label>
    <input
      id="period-end-date"
      class="form-input"
      type="date"
      bind:value={selectedEndDate}
      min={selectedDate}
      max={localDateIso()}
    />

    <button
      class="button button-primary button-full"
      type="button"
      disabled={busy || !selectedDate || (!!selectedEndDate && selectedEndDate < selectedDate)}
      onclick={() => onSave(selectedDate, selectedEndDate || null)}
    >
      {busy ? "Saving…" : "Save period dates"}
    </button>
  </section>

  <section class="history-summary">
    <span class="history-count">{snapshot.periodStarts.length}</span>
    <div>
      <strong>
        {snapshot.periodStarts.length === 1
          ? "One period start recorded"
          : snapshot.periodStarts.length + " period starts recorded"}
      </strong>
      <p>
        {snapshot.lastPeriodStart
          ? "Most recent: " + formatDate(snapshot.lastPeriodStart, { year: "numeric" })
          : "Your cycle history will appear here as you log dates."}
      </p>
    </div>
  </section>

  <section class="history-summary checkin-history-summary">
    <span class="history-count checkin-history-count">{snapshot.checkinsTracked}</span>
    <div>
      <strong>
        {snapshot.checkinsTracked === 1
          ? "One daily check-in saved"
          : snapshot.checkinsTracked + " daily check-ins saved"}
      </strong>
      <p>Teal markers on the calendar show days with a saved check-in.</p>
    </div>
  </section>
</section>
