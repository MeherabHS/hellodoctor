# WorkManager initializes its Room database before the Flutter Activity starts.
# Room 2.5's consumer rules preserve database class names but not the public
# no-arg constructor used by reflective instantiation. R8 full mode removed
# WorkDatabase_Impl.<init>(), causing a fatal InitializationProvider exception.
# Keep the Room base type and concrete database constructors, retaining release
# optimization for the rest of the app.
-keep class androidx.room.RoomDatabase { *; }
-keep class * extends androidx.room.RoomDatabase {
    public <init>();
}

# WorkManager also instantiates persisted worker class names reflectively.
-keep class dev.fluttercommunity.workmanager.BackgroundWorker {
    public <init>(android.content.Context, androidx.work.WorkerParameters);
}

# flutter_local_notifications 18 persists scheduled notifications with Gson
# reflection. Keep model fields stable across updates and retain TypeToken's
# generic signatures; otherwise minified releases cannot reload schedules.
-keepattributes Signature,InnerClasses,EnclosingMethod
-keep class com.google.gson.reflect.TypeToken { *; }
-keep class * extends com.google.gson.reflect.TypeToken { *; }
-keep class com.dexterous.flutterlocalnotifications.** { *; }
