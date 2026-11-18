package __PACKAGE__

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.graphics.Color
import android.net.Uri
import android.widget.RemoteViews
import org.json.JSONObject
import java.io.File
import java.io.IOException
import com.bulldogshot10.miora.uniffi.DailyCheckinInput
import com.bulldogshot10.miora.uniffi.SymptomInput
import com.bulldogshot10.miora.uniffi.widgetGetDailyCheckin
import com.bulldogshot10.miora.uniffi.widgetSaveDailyCheckin
import com.bulldogshot10.miora.uniffi.configureDatabaseKey as configureRustDatabaseKey
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.concurrent.Executors

open class MioraWidgetProvider : AppWidgetProvider() {
    override fun onUpdate(
        context: Context,
        manager: AppWidgetManager,
        appWidgetIds: IntArray,
    ) {
        val appContext = context.applicationContext
        val pendingResult = goAsync()
        worker.execute {
            try {
                val data = readWidgetData(appContext)
                appWidgetIds.forEach { widgetId ->
                    updateWidget(appContext, manager, widgetId, data)
                }
            } catch (_: Exception) {
                appWidgetIds.forEach { widgetId ->
                    preferences(appContext, widgetId).edit()
                        .putString(key("status", widgetId), "Open Miora and try refreshing the widget.")
                        .apply()
                }
            } finally {
                pendingResult.finish()
            }
        }
    }

    override fun onDeleted(context: Context, appWidgetIds: IntArray) {
        super.onDeleted(context, appWidgetIds)
        val appContext = context.applicationContext
        appWidgetIds.forEach { widgetId ->
            clearDraft(appContext, widgetId)
            preferences(appContext, widgetId).edit()
                .remove(key("status", widgetId))
                .apply()
        }
    }

    override fun onReceive(context: Context, intent: Intent) {
        super.onReceive(context, intent)
        val action = intent.action ?: return
        if (!action.startsWith(ACTION_PREFIX)) return
        // User actions are routed through the app-private receiver.
        if (this !is MioraWidgetActionReceiver) return

        val widgetId = intent.getIntExtra(
            AppWidgetManager.EXTRA_APPWIDGET_ID,
            AppWidgetManager.INVALID_APPWIDGET_ID,
        )
        if (widgetId == AppWidgetManager.INVALID_APPWIDGET_ID) return

        val pendingResult = goAsync()
        worker.execute {
            try {
                handleAction(context.applicationContext, widgetId, intent)
            } catch (_: Exception) {
                preferences(context, widgetId).edit()
                    .putString(key("status", widgetId), "Couldn't save. Open Miora and try again.")
                    .apply()
                updateAllWidgets(context.applicationContext)
            } finally {
                pendingResult.finish()
            }
        }
    }

    private fun handleAction(context: Context, widgetId: Int, intent: Intent) {
        val installedIds = AppWidgetManager.getInstance(context)
            .getAppWidgetIds(ComponentName(context, MioraWidgetProvider::class.java))
        if (widgetId !in installedIds) return

        val data = readWidgetData(context)
        if (!data.ready) {
            preferences(context, widgetId).edit()
                .putString(key("status", widgetId), "Open Miora once to initialize your tracker.")
                .apply()
            updateAllWidgets(context)
            return
        }

        val current = visibleState(context, widgetId, data.checkin, data.ready)
        val prefs = preferences(context, widgetId)

        when (intent.action) {
            ACTION_SET_MOOD -> {
                val score = intent.getStringExtra(EXTRA_VALUE)?.toIntOrNull() ?: return
                if (score !in 1..5) return
                saveDraft(context, widgetId, current.copy(mood = score), data.checkin)
            }

            ACTION_SET_ENERGY -> {
                val score = intent.getStringExtra(EXTRA_VALUE)?.toIntOrNull() ?: return
                if (score !in 1..5) return
                saveDraft(context, widgetId, current.copy(energy = score), data.checkin)
            }

            ACTION_CYCLE_SYMPTOM -> {
                val code = intent.getStringExtra(EXTRA_VALUE) ?: return
                if (code !in SYMPTOM_CODES) return
                val symptoms = current.symptoms.toMutableMap()
                val nextSeverity = when (symptoms[code] ?: 0) {
                    0 -> 1
                    1 -> 2
                    2 -> 3
                    else -> 0
                }
                if (nextSeverity == 0) symptoms.remove(code) else symptoms[code] = nextSeverity
                saveDraft(context, widgetId, current.copy(symptoms = symptoms), data.checkin)
            }

            ACTION_SAVE -> {
                saveCheckin(context, current)
                clearDraft(context, widgetId)
            }
        }

        prefs.edit().remove(key("status", widgetId)).apply()
        updateAllWidgets(context)
    }

    private fun updateAllWidgets(context: Context) {
        val manager = AppWidgetManager.getInstance(context)
        val ids = manager.getAppWidgetIds(ComponentName(context, MioraWidgetProvider::class.java))
        val data = readWidgetData(context)
        ids.forEach { updateWidget(context, manager, it, data) }
    }

    private fun updateWidget(
        context: Context,
        manager: AppWidgetManager,
        widgetId: Int,
        data: WidgetData,
    ) {
        val views = RemoteViews(context.packageName, R.layout.miora_daily_checkin_widget)
        val prefs = preferences(context, widgetId)
        val state = visibleState(context, widgetId, data.checkin, data.ready)
        val hasDraft = prefs.getBoolean(key("draft", widgetId), false)

        views.setTextViewText(R.id.widget_date, displayToday())
        views.setTextViewText(R.id.widget_status, when {
            !data.ready -> "Open Miora once to initialize your tracker."
            prefs.getString(key("status", widgetId), null) != null ->
                prefs.getString(key("status", widgetId), "")
            hasDraft -> "Unsaved changes"
            data.checkin != null -> "Saved for today"
            else -> "No check-in saved yet"
        })

        setChoice(views, R.id.mood_1, state.mood == 1)
        setChoice(views, R.id.mood_2, state.mood == 2)
        setChoice(views, R.id.mood_3, state.mood == 3)
        setChoice(views, R.id.mood_4, state.mood == 4)
        setChoice(views, R.id.mood_5, state.mood == 5)

        setChoice(views, R.id.energy_1, state.energy == 1)
        setChoice(views, R.id.energy_2, state.energy == 2)
        setChoice(views, R.id.energy_3, state.energy == 3)
        setChoice(views, R.id.energy_4, state.energy == 4)
        setChoice(views, R.id.energy_5, state.energy == 5)

        setSymptom(views, R.id.symptom_cramps, "cramps", "Cramp", state.symptoms)
        setSymptom(views, R.id.symptom_bloating, "bloating", "Bloat", state.symptoms)
        setSymptom(views, R.id.symptom_headache, "headache", "Headache", state.symptoms)
        setSymptom(views, R.id.symptom_fatigue, "fatigue", "Fatigue", state.symptoms)
        setSymptom(views, R.id.symptom_mood_changes, "mood_changes", "Mood", state.symptoms)

        (1..5).forEach { score ->
            views.setOnClickPendingIntent(
                moodViewId(score),
                clickIntent(context, widgetId, ACTION_SET_MOOD, score.toString()),
            )
            views.setOnClickPendingIntent(
                energyViewId(score),
                clickIntent(context, widgetId, ACTION_SET_ENERGY, score.toString()),
            )
        }

        SYMPTOM_CODES.forEach { code ->
            views.setOnClickPendingIntent(
                symptomViewId(code),
                clickIntent(context, widgetId, ACTION_CYCLE_SYMPTOM, code),
            )
        }

        views.setOnClickPendingIntent(
            R.id.widget_save,
            clickIntent(context, widgetId, ACTION_SAVE, "today"),
        )

        openAppIntent(context)?.let {
            views.setOnClickPendingIntent(R.id.widget_brand, it)
            views.setOnClickPendingIntent(R.id.widget_date, it)
        }
        manager.updateAppWidget(widgetId, views)
    }

    private fun initializeDatabaseKey(context: Context) {
        val databaseKey = MioraDatabaseKeyStore.getOrCreateDatabaseKey(context)
        try {
            configureRustDatabaseKey(databaseKey)
        } finally {
            databaseKey.fill(0)
        }
    }

    private fun saveCheckin(context: Context, state: WidgetState) {
        initializeDatabaseKey(context)
        val database = databaseFile(context)
        check(database.isFile) { "Miora database is not initialized." }

        val symptoms = state.symptoms.map { (code, severity) ->
            SymptomInput(code, severity)
        }
        widgetSaveDailyCheckin(
            database.absolutePath,
            DailyCheckinInput(
                todayIso(),
                state.mood,
                state.energy,
                symptoms,
            ),
        )
    }

    private fun readWidgetData(context: Context): WidgetData {
        val database = databaseFile(context)
        if (!database.isFile) return WidgetData(false, null)

        return try {
            initializeDatabaseKey(context)
            val checkin = widgetGetDailyCheckin(database.absolutePath, todayIso())
            val state = checkin?.let {
                WidgetState(
                    mood = it.moodScore,
                    energy = it.energyScore,
                    symptoms = it.symptoms.associate { symptom ->
                        symptom.code to symptom.severity
                    },
                )
            }
            WidgetData(true, state)
        } catch (_: Exception) {
            WidgetData(false, null)
        }
    }

    private fun readDraft(context: Context, widgetId: Int): WidgetDraft? {
        val prefs = preferences(context, widgetId)
        clearLegacyPlaintextDraftFields(context, widgetId)
        if (!prefs.getBoolean(key("draft", widgetId), false)) {
            prefs.edit().remove(key("draft_blob", widgetId)).apply()
            return null
        }

        return try {
            val encrypted = prefs.getString(key("draft_blob", widgetId), null)
                ?: throw IOException("The encrypted widget draft is missing.")
            val json = JSONObject(MioraDatabaseKeyStore.decryptWidgetDraft(widgetId, encrypted))
            require(json.getInt("version") == 1) { "Unknown widget draft version." }

            val date = json.getString("date")
            val baseState = json.getString("baseState")
            val mood = json.getInt("mood")
            val energy = json.getInt("energy")
            val symptoms = requireNotNull(decodeSymptoms(json.getString("symptoms")))

            require(date.matches(Regex("\\d{4}-\\d{2}-\\d{2}"))) {
                "Invalid widget draft date."
            }
            require(baseState.isNotEmpty()) { "Invalid widget draft base state." }
            require(mood in 1..5 && energy in 1..5) { "Invalid widget draft scores." }

            WidgetDraft(date, baseState, mood, energy, symptoms)
        } catch (_: Exception) {
            clearDraft(context, widgetId)
            null
        }
    }

    private fun visibleState(
        context: Context,
        widgetId: Int,
        saved: WidgetState?,
        databaseReady: Boolean,
    ): WidgetState {
        if (!preferences(context, widgetId).getBoolean(key("draft", widgetId), false)) {
            return saved ?: WidgetState(3, 3, emptyMap())
        }

        val draft = readDraft(context, widgetId)
            ?: return saved ?: WidgetState(3, 3, emptyMap())

        // Drafts belong to a local calendar date; never carry yesterday's entry forward.
        if (draft.date != todayIso()) {
            clearDraft(context, widgetId)
            return saved ?: WidgetState(3, 3, emptyMap())
        }

        // Compare with the app's current record only when SQLite was readable.
        // A transient open/lock error must never erase an unsaved widget draft.
        if (databaseReady && draft.baseState != encodeState(saved)) {
            clearDraft(context, widgetId)
            return saved ?: WidgetState(3, 3, emptyMap())
        }

        return WidgetState(draft.mood, draft.energy, draft.symptoms)
    }

    private fun saveDraft(
        context: Context,
        widgetId: Int,
        state: WidgetState,
        savedState: WidgetState?,
    ) {
        val previousDraft = readDraft(context, widgetId)
        val baseState = previousDraft?.baseState ?: encodeState(savedState)
        val json = JSONObject()
            .put("version", 1)
            .put("date", todayIso())
            .put("baseState", baseState)
            .put("mood", state.mood)
            .put("energy", state.energy)
            .put("symptoms", encodeSymptoms(state.symptoms))
        val encrypted = MioraDatabaseKeyStore.encryptWidgetDraft(widgetId, json.toString())

        preferences(context, widgetId).edit()
            .putBoolean(key("draft", widgetId), true)
            .putString(key("draft_blob", widgetId), encrypted)
            .remove(key("status", widgetId))
            .apply()
    }

    private fun clearLegacyPlaintextDraftFields(context: Context, widgetId: Int) {
        preferences(context, widgetId).edit()
            .remove(key("mood", widgetId))
            .remove(key("energy", widgetId))
            .remove(key("symptoms", widgetId))
            .remove(key("draft_date", widgetId))
            .remove(key("draft_base", widgetId))
            .apply()
    }

    private fun encodeState(state: WidgetState?): String =
        state?.let { "${it.mood}|${it.energy}|${encodeSymptoms(it.symptoms)}" } ?: "none"

    private fun clearDraft(context: Context, widgetId: Int) {
        preferences(context, widgetId).edit()
            .remove(key("draft", widgetId))
            .remove(key("draft_blob", widgetId))
            .remove(key("mood", widgetId))
            .remove(key("energy", widgetId))
            .remove(key("symptoms", widgetId))
            .remove(key("draft_date", widgetId))
            .remove(key("draft_base", widgetId))
            .apply()
    }

    private fun databaseFile(context: Context): File =
        mioraDatabaseFile(context)

    private fun todayIso(): String =
        SimpleDateFormat("yyyy-MM-dd", Locale.US).format(Date())

    private fun displayToday(): String =
        SimpleDateFormat("EEE, d MMM", Locale.getDefault()).format(Date())

    private fun preferences(context: Context, widgetId: Int) =
        context.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)

    private fun key(name: String, widgetId: Int) = name + "_" + widgetId

    private fun encodeSymptoms(symptoms: Map<String, Int>): String =
        symptoms.entries.joinToString(",") { entry -> entry.key + ":" + entry.value }

    private fun decodeSymptoms(value: String?): Map<String, Int>? {
        if (value == null) return null
        if (value.isEmpty()) return emptyMap()
        return value.split(",").mapNotNull { item ->
            val parts = item.split(":")
            if (parts.size != 2 || parts[0] !in SYMPTOM_CODES) {
                null
            } else {
                val severity = parts[1].toIntOrNull()?.coerceIn(1, 3) ?: return@mapNotNull null
                parts[0] to severity
            }
        }.toMap()
    }

    private fun setSymptom(
        views: RemoteViews,
        viewId: Int,
        code: String,
        label: String,
        symptoms: Map<String, Int>,
    ) {
        val severity = symptoms[code]
        val suffix = when (severity) {
            1 -> " · M"
            2 -> " · Mod"
            3 -> " · Sev"
            else -> ""
        }
        views.setTextViewText(viewId, label + suffix)
        setChoice(views, viewId, severity != null)
    }

    private fun setChoice(views: RemoteViews, viewId: Int, selected: Boolean) {
        views.setInt(
            viewId,
            "setBackgroundResource",
            if (selected) R.drawable.miora_widget_chip_selected else R.drawable.miora_widget_chip,
        )
        views.setTextColor(
            viewId,
            if (selected) Color.rgb(35, 31, 55) else Color.rgb(236, 233, 246),
        )
    }

    private fun moodViewId(score: Int): Int = when (score) {
        1 -> R.id.mood_1
        2 -> R.id.mood_2
        3 -> R.id.mood_3
        4 -> R.id.mood_4
        else -> R.id.mood_5
    }

    private fun energyViewId(score: Int): Int = when (score) {
        1 -> R.id.energy_1
        2 -> R.id.energy_2
        3 -> R.id.energy_3
        4 -> R.id.energy_4
        else -> R.id.energy_5
    }

    private fun symptomViewId(code: String): Int = when (code) {
        "cramps" -> R.id.symptom_cramps
        "bloating" -> R.id.symptom_bloating
        "headache" -> R.id.symptom_headache
        "fatigue" -> R.id.symptom_fatigue
        else -> R.id.symptom_mood_changes
    }

    private fun clickIntent(
        context: Context,
        widgetId: Int,
        actionName: String,
        value: String,
    ): PendingIntent {
        val click = Intent(context, MioraWidgetActionReceiver::class.java).apply {
            action = actionName
            data = Uri.parse("miora-widget://" + widgetId + "/" + actionName + "/" + value)
            putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, widgetId)
            putExtra(EXTRA_VALUE, value)
        }
        val requestCode = (widgetId.toString() + actionName + value).hashCode()
        return PendingIntent.getBroadcast(
            context,
            requestCode,
            click,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
    }

    private fun openAppIntent(context: Context): PendingIntent? {
        val launchIntent = context.packageManager.getLaunchIntentForPackage(context.packageName) ?: return null
        launchIntent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP)
        return PendingIntent.getActivity(
            context,
            9017,
            launchIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
    }

    private data class WidgetState(
        val mood: Int,
        val energy: Int,
        val symptoms: Map<String, Int>,
    )

    private data class WidgetDraft(
        val date: String,
        val baseState: String,
        val mood: Int,
        val energy: Int,
        val symptoms: Map<String, Int>,
    )

    private data class WidgetData(
        val ready: Boolean,
        val checkin: WidgetState?,
    )

    companion object {
        private const val ACTION_PREFIX = "miora.widget."
        private const val ACTION_SET_MOOD = "miora.widget.SET_MOOD"
        private const val ACTION_SET_ENERGY = "miora.widget.SET_ENERGY"
        private const val ACTION_CYCLE_SYMPTOM = "miora.widget.CYCLE_SYMPTOM"
        private const val ACTION_SAVE = "miora.widget.SAVE"
        private const val EXTRA_VALUE = "miora.widget.VALUE"
        private const val PREFERENCES = "miora_widget_state"
        private val SYMPTOM_CODES = setOf("cramps", "bloating", "headache", "fatigue", "mood_changes")
        private val worker = Executors.newSingleThreadExecutor()
    }
}

/** Private receiver used only by this app's immutable widget PendingIntents. */
class MioraWidgetActionReceiver : MioraWidgetProvider()
