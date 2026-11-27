package app.tourledger

import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.content.res.Configuration
import android.graphics.Color
import android.net.Uri
import android.net.wifi.WifiManager
import android.os.Build
import android.os.Bundle
import android.provider.Settings
import android.view.View
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

// Copied over the generated file by scripts/build.sh (the gen/ folder is not kept).
class MainActivity : TauriActivity() {
  private var multicast: WifiManager.MulticastLock? = null
  private var updater: Updater? = null

  // The page background colours from ui/src/styles/theme.css.
  private fun background(): Int {
    val night =
      resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK ==
        Configuration.UI_MODE_NIGHT_YES
    return Color.parseColor(if (night) "#0c101a" else "#eef1f8")
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)

    // Older web views report no inset for the system buttons or the keyboard, so
    // the page would sit underneath them. Pad the whole view instead.
    val content = findViewById<View>(android.R.id.content)
    content.setBackgroundColor(background())
    ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
      val bars =
        insets.getInsets(
          WindowInsetsCompat.Type.systemBars() or
            WindowInsetsCompat.Type.displayCutout() or
            WindowInsetsCompat.Type.ime()
        )
      view.setPadding(bars.left, bars.top, bars.right, bars.bottom)
      WindowInsetsCompat.CONSUMED
    }

    // Without this lock many phones drop the Wi-Fi messages that other devices
    // use to announce themselves.
    val wifi = applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
    multicast = wifi.createMulticastLock("tourledger-sync").apply { setReferenceCounted(false) }
  }

  // Held only while the app is in front.
  override fun onResume() {
    super.onResume()
    multicast?.acquire()
  }

  override fun onPause() {
    multicast?.let { if (it.isHeld) it.release() }
    super.onPause()
  }

  override fun onWebViewCreate(webView: WebView) {
    // A new web view is white until the page paints: a flash on a dark phone.
    webView.setBackgroundColor(background())
    webView.addJavascriptInterface(LocalNetwork(), "TourLedgerAndroid")
    updater?.close()
    updater = Updater(this).also { webView.addJavascriptInterface(it, "TourLedgerUpdates") }
  }

  override fun onDestroy() {
    updater?.close()
    updater = null
    super.onDestroy()
  }

  /**
   * Android 17 blocks the local network until the person allows it. These three
   * calls are all the page can do with it.
   */
  inner class LocalNetwork {
    private val permission = "android.permission.ACCESS_LOCAL_NETWORK"

    @JavascriptInterface
    fun localNetwork(): String =
      when {
        Build.VERSION.SDK_INT < 37 -> "not_needed"
        checkSelfPermission(permission) == PackageManager.PERMISSION_GRANTED -> "granted"
        else -> "needed"
      }

    @JavascriptInterface
    fun requestLocalNetwork() {
      if (Build.VERSION.SDK_INT >= 37) {
        runOnUiThread { requestPermissions(arrayOf(permission), 4001) }
      }
    }

    /** For when Android no longer shows the question. */
    @JavascriptInterface
    fun openAppSettings() {
      runOnUiThread {
        startActivity(
          Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", packageName, null))
        )
      }
    }
  }
}
