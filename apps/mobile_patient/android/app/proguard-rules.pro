# flutter_local_notifications and workmanager both resolve Android
# BroadcastReceiver/Worker/Service classes via reflection; without these keeps
# R8 can silently strip them in a minified release build and notifications
# stop firing without any build-time error. Verify with a real release build.
-keep class com.dexterous.** { *; }
-keep class dev.fluttercommunity.workmanager.** { *; }
-keep class androidx.work.** { *; }
-keep class * extends androidx.work.ListenableWorker {
    public <init>(android.content.Context, androidx.work.WorkerParameters);
}
-keep class * extends android.app.backup.BackupAgent
-keepclassmembers class * extends android.content.BroadcastReceiver {
    public <init>();
}

# flutter_local_notifications stores scheduled notifications (every reminder
# alarm) with Gson 2.8.9, which ships no R8 rules. R8 full mode strips the
# generic signature of its `new TypeToken<ArrayList<NotificationDetails>>(){}`,
# so zonedSchedule() fails in release builds only ("Missing type parameter")
# and no reminder alarm is ever set. Keep the signatures Gson reads.
-keepattributes Signature, InnerClasses, EnclosingMethod, *Annotation*
-keep class com.google.gson.reflect.TypeToken { *; }
-keep,allowobfuscation,allowshrinking class com.google.gson.reflect.TypeToken
-keep,allowobfuscation,allowshrinking class * extends com.google.gson.reflect.TypeToken
-keepclassmembers,allowobfuscation class * {
  @com.google.gson.annotations.SerializedName <fields>;
}
