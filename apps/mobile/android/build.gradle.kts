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

// Some plugins (e.g. file_picker 8.x) still declare an older compileSdk than
// flutter_plugin_android_lifecycle now requires (36+), which fails AAR
// metadata checks. Force every Android subproject to compile against the
// app's own compileSdk rather than bumping plugin major versions blind.
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

tasks.register<Delete>("clean") {
    delete(rootProject.layout.buildDirectory)
}
