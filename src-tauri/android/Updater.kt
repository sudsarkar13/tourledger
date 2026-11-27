package app.tourledger

import android.app.Activity
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageInstaller
import android.net.ConnectivityManager
import android.net.Uri
import android.os.Build
import android.provider.Settings
import android.webkit.JavascriptInterface
import androidx.core.content.ContextCompat
import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest
import org.json.JSONObject

// Copied into the generated project by scripts/build.sh.

/**
 * Updating from the app's GitHub releases. Android installs only an APK signed
 * with the same key as the app already here, and this asks it to refuse any
 * other package. Entries stay: they are in the app's own storage, which an
 * update keeps.
 *
 * The page can only start these steps. Where the update comes from is fixed
 * here, and the page is not given a way to change it.
 */
class Updater(private val activity: Activity) {
  private val release = "https://api.github.com/repos/sudsarkar13/tourledger/releases/latest"
  private val result = "app.tourledger.UPDATE_RESULT"
  private val file = File(activity.cacheDir, "update.apk")

  // All guarded by `lock`.
  private val lock = Any()
  private var phase = "idle"
  private var version: String? = null
  private var notes: String? = null
  private var error: String? = null
  private var received = 0L
  private var total: Long? = null
  private var checked: Long? = null
  private var url: String? = null
  private var digest: String? = null
  private var downloaded: String? = null
  private var session = -1

  private val receiver =
    object : BroadcastReceiver() {
      override fun onReceive(context: Context, intent: Intent) {
        // Only the outcome of the install this app started.
        if (intent.getIntExtra(PackageInstaller.EXTRA_SESSION_ID, -2) != synchronized(lock) { session }) return
        when (intent.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE)) {
          PackageInstaller.STATUS_PENDING_USER_ACTION -> {
            @Suppress("DEPRECATION") val confirm = intent.getParcelableExtra<Intent>(Intent.EXTRA_INTENT)
            // Until the person answers, the download is still there to install.
            synchronized(lock) { phase = "ready" }
            if (confirm != null) activity.startActivity(confirm)
          }
          // Android now closes the app to replace it.
          PackageInstaller.STATUS_SUCCESS -> {}
          PackageInstaller.STATUS_FAILURE_ABORTED -> synchronized(lock) { phase = "ready" }
          else -> {
            val why = intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE)
            synchronized(lock) {
              phase = "ready"
              error = why ?: "Android refused it."
            }
          }
        }
      }
    }

  init {
    // Closed to other apps on every Android version: this receiver starts whatever
    // confirmation screen it is handed.
    ContextCompat.registerReceiver(
      activity,
      receiver,
      IntentFilter(result),
      ContextCompat.RECEIVER_NOT_EXPORTED,
    )
  }

  fun close() {
    activity.unregisterReceiver(receiver)
  }

  @JavascriptInterface
  fun status(): String =
    synchronized(lock) {
      JSONObject()
        .put("supported", true)
        .put("current", BuildConfig.VERSION_NAME)
        .put("phase", phase)
        .put("version", version ?: JSONObject.NULL)
        .put("notes", notes ?: JSONObject.NULL)
        .put("received", received)
        .put("total", total ?: JSONObject.NULL)
        .put("error", error ?: JSONObject.NULL)
        .put("checked_ms", checked ?: JSONObject.NULL)
        .put("metered", metered())
        .toString()
    }

  @JavascriptInterface
  fun check() {
    synchronized(lock) {
      if (busy()) return
      phase = "checking"
      error = null
    }
    background {
      val json = JSONObject(open(release).use { it.bufferedReader().readText() })
      val latest = json.getString("tag_name").removePrefix("v")
      var apk: JSONObject? = null
      val assets = json.getJSONArray("assets")
      for (i in 0 until assets.length()) {
        val asset = assets.getJSONObject(i)
        if (asset.getString("name").endsWith("_android.apk")) apk = asset
      }
      synchronized(lock) {
        checked = System.currentTimeMillis()
        // A release without an APK has nothing for this device yet.
        if (apk == null || !newer(latest, BuildConfig.VERSION_NAME)) {
          phase = "up_to_date"
          version = null
          notes = null
          return@synchronized
        }
        version = latest
        notes = json.optString("body").substringBefore("\n\n").ifBlank { null }
        url = apk.getString("browser_download_url")
        total = apk.getLong("size")
        digest = apk.optString("digest").removePrefix("sha256:").ifBlank { null }
        phase = "available"
      }
      // A download kept from before, even from an earlier run, is used if it matches.
      val expected = synchronized(lock) { if (phase == "available") digest else null }
      if (expected != null && file.exists() && sha256(file) == expected) {
        synchronized(lock) {
          downloaded = latest
          phase = "ready"
        }
      }
    }
  }

  @JavascriptInterface
  fun download() {
    val from: String
    val expected: String?
    val size: Long?
    val wanted: String?
    synchronized(lock) {
      if (busy() || url == null) return
      from = url!!
      expected = digest
      size = total
      wanted = version
      phase = "downloading"
      error = null
      received = 0
      // The file is about to be overwritten.
      downloaded = null
    }
    background {
      val sha = MessageDigest.getInstance("SHA-256")
      open(from).use { input ->
        file.outputStream().use { output ->
          val buffer = ByteArray(64 * 1024)
          while (true) {
            val count = input.read(buffer)
            if (count < 0) break
            output.write(buffer, 0, count)
            sha.update(buffer, 0, count)
            synchronized(lock) { received += count }
          }
        }
      }
      val actual = sha.digest().joinToString("") { "%02x".format(it) }
      if ((size != null && file.length() != size) || (expected != null && actual != expected)) {
        file.delete()
        throw IllegalStateException("The download was incomplete or changed. Try again.")
      }
      synchronized(lock) {
        downloaded = wanted
        phase = "ready"
      }
    }
  }

  @JavascriptInterface
  fun install() {
    synchronized(lock) {
      if ((phase != "ready" && phase != "needs_permission") || !file.exists()) return
      // Android asks once whether this app may install apps.
      if (!activity.packageManager.canRequestPackageInstalls()) {
        phase = "needs_permission"
        activity.runOnUiThread {
          activity.startActivity(
            Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:${activity.packageName}"))
          )
        }
        return
      }
      phase = "installing"
      error = null
    }
    background {
      val installer = activity.packageManager.packageInstaller
      val params = PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL)
      params.setAppPackageName(activity.packageName)
      if (Build.VERSION.SDK_INT >= 31) {
        // Honoured once this app has installed itself before.
        params.setRequireUserAction(PackageInstaller.SessionParams.USER_ACTION_NOT_REQUIRED)
      }
      val id = installer.createSession(params)
      synchronized(lock) { session = id }
      installer.openSession(id).use { session ->
        session.openWrite("update", 0, file.length()).use { output ->
          file.inputStream().use { it.copyTo(output) }
          session.fsync(output)
        }
        // Android adds the outcome to this intent, so it has to be mutable.
        val flags =
          PendingIntent.FLAG_UPDATE_CURRENT or
            (if (Build.VERSION.SDK_INT >= 31) PendingIntent.FLAG_MUTABLE else 0)
        val done =
          PendingIntent.getBroadcast(activity, id, Intent(result).setPackage(activity.packageName), flags)
        session.commit(done.intentSender)
      }
    }
  }

  private fun sha256(of: File): String {
    val sha = MessageDigest.getInstance("SHA-256")
    of.inputStream().use { input ->
      val buffer = ByteArray(64 * 1024)
      while (true) {
        val count = input.read(buffer)
        if (count < 0) break
        sha.update(buffer, 0, count)
      }
    }
    return sha.digest().joinToString("") { "%02x".format(it) }
  }

  private fun busy() = phase == "checking" || phase == "downloading" || phase == "installing"

  private fun metered(): Boolean =
    (activity.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager)
      .isActiveNetworkMetered

  private fun background(work: () -> Unit) {
    Thread {
        try {
          work()
        } catch (e: Exception) {
          synchronized(lock) {
            // A finished download stays on offer; a failed install says why.
            if (downloaded != null && downloaded == version && file.exists()) {
              error = if (phase == "installing") e.message ?: "It could not be started." else null
              phase = "ready"
            } else {
              phase = "failed"
              error = e.message ?: "The update did not work."
            }
          }
        }
      }
      .start()
  }

  /** HTTPS only, following GitHub's redirect to the file. */
  private fun open(address: String): java.io.InputStream {
    require(address.startsWith("https://")) { "Updates are fetched over HTTPS only." }
    val connection = URL(address).openConnection() as HttpURLConnection
    connection.connectTimeout = 15_000
    connection.readTimeout = 30_000
    // GitHub asks every client to name itself.
    connection.setRequestProperty("User-Agent", "TourLedger")
    if (connection.responseCode != 200) {
      throw IllegalStateException("GitHub answered ${connection.responseCode}. Try again later.")
    }
    return connection.inputStream
  }

  /** Compares dotted version numbers, for example 0.2.0 and 0.10.1. */
  private fun newer(candidate: String, current: String): Boolean {
    val a = candidate.split('.').map { it.toIntOrNull() ?: return false }
    val b = current.split('.').map { it.toIntOrNull() ?: 0 }
    for (i in 0 until maxOf(a.size, b.size)) {
      val difference = a.getOrElse(i) { 0 } - b.getOrElse(i) { 0 }
      if (difference != 0) return difference > 0
    }
    return false
  }
}
