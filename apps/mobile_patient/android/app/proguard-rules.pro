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
