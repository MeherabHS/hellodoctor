import org.jetbrains.kotlin.gradle.dsl.JvmTarget
import org.jetbrains.kotlin.gradle.tasks.KotlinCompile

allprojects {
    repositories {
        google()
        mavenCentral()
    }
}

val newBuildDir: Directory =
    rootProject.layout.buildDirectory
        .dir("../../build")
        .get()
rootProject.layout.buildDirectory.value(newBuildDir)

subprojects {
    val newSubprojectBuildDir: Directory = newBuildDir.dir(project.name)
    project.layout.buildDirectory.value(newSubprojectBuildDir)
}

// Matches apps/mobile's working configuration. Several plugins (notably
// agora_rtc_engine, published compiled against API 31) declare an older
// compileSdk than other resolved AndroidX transitives now require (34+),
// which fails Gradle's AAR metadata check. Force every Android subproject to
// compile against the app's own compileSdk rather than bumping plugin major
// versions blind.
// Must be registered before evaluationDependsOn(":app") below forces early
// evaluation, or afterEvaluate on :app throws "already evaluated".
subprojects {
    afterEvaluate {
        extensions.findByName("android")?.withGroovyBuilder {
            setProperty("compileSdkVersion", 36)
        }
    }
}

subprojects {
    project.evaluationDependsOn(":app")
}

// file_picker 10.x still compiles Java at 11 while declaring Kotlin 1.8;
// align Kotlin with that plugin's Java target under Gradle's strict check.
project(":file_picker") {
    tasks.withType<KotlinCompile>().configureEach {
        compilerOptions.jvmTarget.set(JvmTarget.JVM_11)
    }
}

tasks.register<Delete>("clean") {
    delete(rootProject.layout.buildDirectory)
}
