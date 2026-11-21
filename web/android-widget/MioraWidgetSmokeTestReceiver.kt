package __PACKAGE__

import android.appwidget.AppWidgetManager
import android.content.BroadcastReceiver
import android.content.ComponentName
import android.content.Context
import android.content.Intent

/**
 * Debug-build bridge for Android runtime tests. The shell sends this app-private
 * action; this receiver forwards the real widget update as Miora's own UID.
 *
 * The generator removes the receiver and this class from release builds.
 */
class MioraWidgetSmokeTestReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != ACTION) return

        val widgetIds = intent.getIntArrayExtra(AppWidgetManager.EXTRA_APPWIDGET_IDS)
            ?: return
        if (widgetIds.isEmpty()) return

        val updateIntent = Intent(AppWidgetManager.ACTION_APPWIDGET_UPDATE).apply {
            component = ComponentName(context, MioraWidgetProvider::class.java)
            putExtra(AppWidgetManager.EXTRA_APPWIDGET_IDS, widgetIds)
        }
        context.applicationContext.sendBroadcast(updateIntent)
    }

    companion object {
        const val ACTION = "__PACKAGE__.CI_WIDGET_UPDATE"
    }
}
