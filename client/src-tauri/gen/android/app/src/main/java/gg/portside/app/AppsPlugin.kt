package gg.portside.app

// Installs, starts and removes games published as APKs (client/docs/platforms.md, "Android").
// Called from Rust (src-tauri/src/android.rs), never from the webview.

import android.app.Activity
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageInstaller
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.provider.Settings
import android.webkit.WebView
import androidx.activity.result.ActivityResult
import androidx.core.content.ContextCompat
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File
import java.util.concurrent.ConcurrentHashMap

@InvokeArg
class InstallArgs {
  lateinit var path: String
  lateinit var packageName: String
}

@InvokeArg
class PackageArgs {
  lateinit var packageName: String
}

@TauriPlugin
class AppsPlugin(private val activity: Activity) : Plugin(activity) {
  private val action = "${activity.packageName}.INSTALL_STATUS"
  /** Installs waiting for Android's answer, by session id. */
  private val pending = ConcurrentHashMap<Int, Invoke>()
  /** The package an uninstall was asked for, until the system screen closes. */
  private var removing: String? = null

  private val receiver = object : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
      val session = intent.getIntExtra(PackageInstaller.EXTRA_SESSION_ID, -1)
      when (val status = intent.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE)) {
        PackageInstaller.STATUS_PENDING_USER_ACTION -> {
          // Android asks the player to confirm; the final status arrives afterwards.
          @Suppress("DEPRECATION")
          val confirm = intent.getParcelableExtra<Intent>(Intent.EXTRA_INTENT)
          if (confirm == null) {
            pending.remove(session)?.reject("Android didn't ask to confirm the install.", "E_ANDROID_INSTALL")
            return
          }
          confirm.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
          activity.startActivity(confirm)
        }
        PackageInstaller.STATUS_SUCCESS -> pending.remove(session)?.resolve()
        PackageInstaller.STATUS_FAILURE_ABORTED ->
          pending.remove(session)?.reject("The install was cancelled.", "E_CANCELLED")
        else -> {
          val detail = intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE)
          pending.remove(session)?.reject(failure(status, detail), "E_ANDROID_INSTALL")
        }
      }
    }
  }

  override fun load(webView: WebView) {
    ContextCompat.registerReceiver(
      activity,
      receiver,
      IntentFilter(action),
      ContextCompat.RECEIVER_NOT_EXPORTED,
    )
  }

  /** Hands a downloaded APK to Android's package installer; resolves once it's installed. */
  @Command
  fun install(invoke: Invoke) {
    val args = invoke.parseArgs(InstallArgs::class.java)
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O &&
      !activity.packageManager.canRequestPackageInstalls()
    ) {
      // Android only lets apps install others once the player allows it, in Settings.
      activity.startActivity(
        Intent(
          Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
          Uri.parse("package:${activity.packageName}"),
        ),
      )
      invoke.reject(
        "Allow Portside to install apps in the settings that just opened, then install again.",
        "E_INSTALL_PERMISSION",
      )
      return
    }
    Thread {
      try {
        val apk = File(args.path)
        val installer = activity.packageManager.packageInstaller
        val params = PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL)
        params.setAppPackageName(args.packageName)
        params.setSize(apk.length())
        val id = installer.createSession(params)
        installer.openSession(id).use { session ->
          apk.inputStream().use { input ->
            session.openWrite("game.apk", 0, apk.length()).use { output ->
              input.copyTo(output)
              session.fsync(output)
            }
          }
          pending[id] = invoke
          val status = Intent(action).setPackage(activity.packageName)
          // Mutable: the installer adds the status to it.
          val flags = PendingIntent.FLAG_UPDATE_CURRENT or
            (if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) PendingIntent.FLAG_MUTABLE else 0)
          val callback = PendingIntent.getBroadcast(activity, id, status, flags)
          session.commit(callback.intentSender)
        }
      } catch (e: Exception) {
        invoke.reject("Couldn't hand the game to Android's installer: ${e.message}", "E_ANDROID_INSTALL")
      }
    }.start()
  }

  /** Opens an installed game. */
  @Command
  fun launch(invoke: Invoke) {
    val args = invoke.parseArgs(PackageArgs::class.java)
    val intent = activity.packageManager.getLaunchIntentForPackage(args.packageName)
    if (intent == null) {
      invoke.reject("This game isn't installed on this device anymore.", "E_NOT_INSTALLED")
      return
    }
    intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    activity.startActivity(intent)
    invoke.resolve()
  }

  /** Whether a package is installed, and which version. */
  @Command
  fun installed(invoke: Invoke) {
    val args = invoke.parseArgs(PackageArgs::class.java)
    val result = JSObject()
    try {
      val info = activity.packageManager.getPackageInfo(args.packageName, 0)
      result.put("installed", true)
      result.put("version", info.versionName)
    } catch (e: PackageManager.NameNotFoundException) {
      result.put("installed", false)
    }
    invoke.resolve(result)
  }

  /** Asks Android to remove a game; resolves with whether it's gone. */
  @Command
  fun uninstall(invoke: Invoke) {
    val args = invoke.parseArgs(PackageArgs::class.java)
    if (!isInstalled(args.packageName)) {
      invoke.resolve(JSObject().put("removed", true))
      return
    }
    removing = args.packageName
    val intent = Intent(Intent.ACTION_DELETE, Uri.parse("package:${args.packageName}"))
    startActivityForResult(invoke, intent, "uninstallDone")
  }

  @ActivityCallback
  private fun uninstallDone(invoke: Invoke, result: ActivityResult) {
    val name = removing
    removing = null
    invoke.resolve(JSObject().put("removed", name == null || !isInstalled(name)))
  }

  private fun isInstalled(name: String): Boolean =
    try {
      activity.packageManager.getPackageInfo(name, 0)
      true
    } catch (e: PackageManager.NameNotFoundException) {
      false
    }

  private fun failure(status: Int, detail: String?): String {
    val reason = when (status) {
      PackageInstaller.STATUS_FAILURE_CONFLICT ->
        "A different copy of this game is installed (signed by someone else). Uninstall it first."
      PackageInstaller.STATUS_FAILURE_INCOMPATIBLE -> "This build doesn't run on this device."
      PackageInstaller.STATUS_FAILURE_STORAGE -> "There isn't enough free space."
      PackageInstaller.STATUS_FAILURE_INVALID -> "The download isn't a valid Android app."
      PackageInstaller.STATUS_FAILURE_BLOCKED -> "Android blocked the install."
      else -> "Android couldn't install it."
    }
    return if (detail.isNullOrBlank()) reason else "$reason ($detail)"
  }
}
