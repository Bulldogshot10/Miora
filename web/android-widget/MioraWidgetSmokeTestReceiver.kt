package __PACKAGE__

import android.appwidget.AppWidgetManager
import android.content.BroadcastReceiver
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.util.Log

/**
 * Debug-build bridge for Android runtime tests. Shell-triggered actions run as
 * Miora's UID and are removed from release builds by the Android generator.
 */
class MioraWidgetSmokeTestReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val widgetIds = intent.getIntArrayExtra(AppWidgetManager.EXTRA_APPWIDGET_IDS)
            ?: return
        if (widgetIds.isEmpty()) return

        when (intent.action) {
            ACTION_UPDATE -> {
                val updateIntent = Intent(AppWidgetManager.ACTION_APPWIDGET_UPDATE).apply {
                    component = ComponentName(context, MioraWidgetProvider::class.java)
                    putExtra(AppWidgetManager.EXTRA_APPWIDGET_IDS, widgetIds)
                }
                context.applicationContext.sendBroadcast(updateIntent)
            }

            ACTION_ROUND_TRIP -> {
                val appContext = context.applicationContext
                val pendingResult = goAsync()
                Thread {
                    try {
                        widgetIds.forEach { widgetId ->
                            MioraWidgetActionReceiver().runSmokeRoundTrip(appContext, widgetId)
                        }
                    } catch (error: Exception) {
                        Log.e("MioraWidget", "widget_action_round_trip_failed", error)
                    } finally {
                        pendingResult.finish()
                    }
                }.start()
            }
        }
    }

    companion object {
        const val ACTION_UPDATE = "__PACKAGE__.CI_WIDGET_UPDATE"
        const val ACTION_ROUND_TRIP = "__PACKAGE__.CI_WIDGET_ACTION_ROUND_TRIP"
    }
}
