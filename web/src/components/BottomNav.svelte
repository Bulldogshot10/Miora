<script lang="ts">
  import { Home, CalendarDays, ClipboardCheck, BarChart3 } from "@lucide/svelte";
  import type { Tab } from "$lib/types";

  export let active: Tab;
  export let onChange: (tab: Tab) => void;
</script>

<nav class="bottom-nav" aria-label="Main navigation">
  <button class:active={active === "home"} type="button" aria-current={active === "home" ? "page" : undefined} onclick={() => onChange("home")}>
    <Home size={20} strokeWidth={1.8} />
    <span>Home</span>
  </button>
  <button class:active={active === "calendar"} type="button" aria-current={active === "calendar" ? "page" : undefined} onclick={() => onChange("calendar")}>
    <CalendarDays size={20} strokeWidth={1.8} />
    <span>Calendar</span>
  </button>
  <button class:active={active === "checkin"} type="button" aria-current={active === "checkin" ? "page" : undefined} onclick={() => onChange("checkin")}>
    <ClipboardCheck size={20} strokeWidth={1.8} />
    <span>Check-in</span>
  </button>
  <button class:active={active === "insights"} type="button" aria-current={active === "insights" ? "page" : undefined} onclick={() => onChange("insights")}>
    <BarChart3 size={20} strokeWidth={1.8} />
    <span>Insights</span>
  </button>
</nav>
