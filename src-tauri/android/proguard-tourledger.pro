# Copied into the generated project by scripts/build.sh.
#
# The QR scanner (ML Kit) creates these classes by name. Without their
# constructors a release build opens, but "Scan a code" fails.
-keep class * implements com.google.firebase.components.ComponentRegistrar { <init>(); }

# Called from the page, never from Kotlin.
-keepclassmembers class app.tourledger.** {
  @android.webkit.JavascriptInterface <methods>;
}
