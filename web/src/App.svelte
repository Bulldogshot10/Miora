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
    logPeriodStart,
    saveDailyCheckin,
  } from "$lib/api";
  import { EMPTY_SNAPSHOT, type DashboardSnapshot, type DailyCheckinInput, type Tab } from "$lib/types";

  let activeTab: Tab = "home";
  let snapshot: DashboardSnapshot = { ...EMPTY_SNAPSHOT };
  let loading = true;
  let busy = false;
  let errorMessage = "";
  let statusMessage = "";

  onMount(() => {
    void refresh();
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

  function navigate(tab: Tab) {
    activeTab = tab;
    statusMessage = "";
  }

  async function handlePeriodSave(date: string) {
    if (!date) {
      statusMessage = "Choose the first day of bleeding.";
      return;
    }

    busy = true;
    statusMessage = "";
    errorMessage = "";
    try {
      const alreadyLogged = snapshot.periodStarts.includes(date);
      snapshot = await logPeriodStart(date);
      statusMessage = alreadyLogged ? "That date is already in your history." : "Period start saved.";
      activeTab = "home";
    } catch (error) {
      errorMessage = String(error);
    } finally {
      busy = false;
    }
  }

  async function handleCheckinSave(input: DailyCheckinInput): Promise<void> {
    busy = true;
    try {
      snapshot = await saveDailyCheckin(input);
      statusMessage = "";
    } catch (error) {
      throw error;
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
        <span class="brand-mark" aria-hidden="true">m</span>
        <span class="brand-word">miora</span>
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
        <HomeScreen {snapshot} onNavigate={navigate} />
      {:else if activeTab === "calendar"}
        <CalendarScreen {snapshot} onSave={handlePeriodSave} {busy} />
      {:else if activeTab === "checkin"}
        <CheckinScreen onSave={handleCheckinSave} {busy} />
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
