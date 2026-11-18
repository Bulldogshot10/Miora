<script lang="ts">
  import { onMount } from "svelte";
  import { ModeWatcher, toggleMode } from "mode-watcher";
  import { SunMoon } from "@lucide/svelte";

  import BottomNav from "./components/BottomNav.svelte";
  import HomeScreen from "./screens/HomeScreen.svelte";
  import CalendarScreen from "./screens/CalendarScreen.svelte";
  import CheckinScreen from "./screens/CheckinScreen.svelte";
  import InsightsScreen from "./screens/InsightsScreen.svelte";

  import {
    getDashboardSnapshot,
    getDailyCheckin,
    logPeriodStart,
    reportBleedingStatus,
    saveDailyCheckin,
  } from "$lib/api";
  import { EMPTY_SNAPSHOT, type DashboardSnapshot, type DailyCheckinInput, type Tab } from "$lib/types";

  let activeTab: Tab = "home";
  let snapshot: DashboardSnapshot = { ...EMPTY_SNAPSHOT };
  let loading = true;
  let busy = false;
  let errorMessage = "";
  let statusMessage = "";
  let hasUnsavedCheckin = false;

  onMount(() => {
    void refresh();
    document.addEventListener("visibilitychange", handleVisibilityChange);
    return () => document.removeEventListener("visibilitychange", handleVisibilityChange);
  });

  async function refresh() {
    loading = true;
    errorMessage = "";
    try {
      snapshot = await getDashboardSnapshot();
    } catch (error) {
      errorMessage = String(error);
    } finally {
      loading = false;
    }
  }

  async function handleVisibilityChange() {
    if (document.visibilityState !== "visible" || hasUnsavedCheckin || busy) return;
    try {
      snapshot = await getDashboardSnapshot();
      errorMessage = "";
      window.dispatchEvent(new Event("miora:refresh-checkin"));
    } catch (error) {
      errorMessage = String(error);
    }
  }

  function navigate(tab: Tab) {
    if (activeTab === "checkin" && tab !== "checkin" && hasUnsavedCheckin) {
      if (!window.confirm("This check-in has unsaved changes. Discard them and leave?")) {
        return;
      }
      hasUnsavedCheckin = false;
    }
    activeTab = tab;
    statusMessage = "";
  }

  async function handlePeriodSave(date: string, endDate: string | null) {
    if (!date) {
      statusMessage = "Choose the first day of bleeding.";
      return;
    }

    busy = true;
    statusMessage = "";
    errorMessage = "";
    try {
      const alreadyLogged = snapshot.periodStarts.includes(date);
      snapshot = await logPeriodStart(date, endDate);
      statusMessage = alreadyLogged
        ? (endDate ? "Bleeding end saved." : "That date is already in your history.")
        : "Period start saved.";
      activeTab = "home";
    } catch (error) {
      statusMessage = "Couldn't save period dates: " +
        (error instanceof Error ? error.message : String(error));
    } finally {
      busy = false;
    }
  }

  async function handleCheckinSave(input: DailyCheckinInput): Promise<void> {
    busy = true;
    try {
      snapshot = await saveDailyCheckin(input);
      statusMessage = "";
    } finally {
      busy = false;
    }
  }

  async function handleCheckinLoad(date: string): Promise<DailyCheckinInput | null> {
    return getDailyCheckin(date);
  }

  async function handleBleedingStatus(stillBleeding: boolean, lastBleedingDate: string | null): Promise<void> {
    const prompt = snapshot.bleedingPrompt;
    if (!prompt || busy) return;

    busy = true;
    statusMessage = "";
    try {
      snapshot = await reportBleedingStatus(
        prompt.periodStart,
        prompt.checkDate,
        stillBleeding,
        lastBleedingDate,
      );
      statusMessage = stillBleeding
        ? "Bleeding noted for today. Miora will check again tomorrow."
        : "Bleeding end confirmed. Your personal model has been updated.";
    } catch (error) {
      statusMessage = "Couldn't update bleeding status: " +
        (error instanceof Error ? error.message : String(error));
    } finally {
      busy = false;
    }
  }
</script>

<ModeWatcher />

<div class="app-shell">
  <header class="app-header">
    <div class="header-inner">
      <a class="brand" href="#home" onclick={(event) => { event.preventDefault(); navigate("home"); }}>
        <img
          class="brand-logo"
          src="/miora-wordmark.webp"
          alt="Miora — Cycle, Symptom, Insight"
          width="180"
          height="60"
          fetchpriority="high"
        />
      </a>

      <nav class="desktop-nav" aria-label="Main navigation">
        <button class:active={activeTab === "home"} onclick={() => navigate("home")}>Home</button>
        <button class:active={activeTab === "calendar"} onclick={() => navigate("calendar")}>Calendar</button>
        <button class:active={activeTab === "checkin"} onclick={() => navigate("checkin")}>Check-in</button>
        <button class:active={activeTab === "insights"} onclick={() => navigate("insights")}>Insights</button>
      </nav>

      <button class="theme-toggle" type="button" onclick={toggleMode} aria-label="Toggle light and dark mode" title="Toggle theme">
        <SunMoon size={19} strokeWidth={1.8} />
      </button>
    </div>
  </header>

  <main class="page">
    {#if loading}
      <div class="loading-state" role="status">
        <span class="loading-dot"></span>
        <span>Opening your cycle space…</span>
      </div>
    {:else if errorMessage}
      <section class="error-panel" role="alert">
        <h1>We couldn't load your data.</h1>
        <p>{errorMessage}</p>
        <button class="button button-primary" onclick={refresh}>Try again</button>
      </section>
    {:else}
      {#if activeTab === "home"}
        <HomeScreen
          {snapshot}
          onNavigate={navigate}
          onBleedingStatus={handleBleedingStatus}
          {busy}
        />
      {:else if activeTab === "calendar"}
        <CalendarScreen {snapshot} onSave={handlePeriodSave} {busy} />
      {:else if activeTab === "checkin"}
        <CheckinScreen
          onSave={handleCheckinSave}
          onLoad={handleCheckinLoad}
          onDirtyChange={(dirty) => { hasUnsavedCheckin = dirty; }}
          {busy}
        />
      {:else}
        <InsightsScreen {snapshot} />
      {/if}
    {/if}

    {#if statusMessage}
      <p class="status-message" role="status">{statusMessage}</p>
    {/if}
  </main>

  <BottomNav active={activeTab} onChange={navigate} />
</div>
