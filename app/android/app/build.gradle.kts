import java.util.Properties

plugins {
    id("com.android.application")
    // The Flutter Gradle Plugin must be applied after the Android and Kotlin Gradle plugins.
    id("dev.flutter.flutter-gradle-plugin")
}

// The upload key that signs release builds for Google Play: android/key.properties locally
// (never committed), or COMPTER_UPLOAD_* variables in CI. See app/README.md.
val uploadKey = Properties().apply {
    val file = rootProject.file("key.properties")
    if (file.exists()) {
        file.inputStream().use { load(it) }
    } else {
        System.getenv("COMPTER_UPLOAD_STORE_FILE")?.let { put("storeFile", it) }
        System.getenv("COMPTER_UPLOAD_STORE_PASSWORD")?.let { put("storePassword", it) }
        System.getenv("COMPTER_UPLOAD_KEY_ALIAS")?.let { put("keyAlias", it) }
        System.getenv("COMPTER_UPLOAD_KEY_PASSWORD")?.let { put("keyPassword", it) }
    }
}
val hasUploadKey = uploadKey.getProperty("storeFile") != null

android {
    namespace = "app.compter"
    compileSdk = flutter.compileSdkVersion
    ndkVersion = flutter.ndkVersion

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    defaultConfig {
        applicationId = "app.compter"
        minSdk = flutter.minSdkVersion
        targetSdk = flutter.targetSdkVersion
        // Uses the version code from pubspec.yaml. When using split APKs, 1000 * ABI_VERSION
        // is added automatically by Flutter. (https://developer.android.com/studio/build/configure-apk-splits#configure-APK-versions)
        // You can force using the value of versionCode by specifying the `-P force-version-code-ignoring-abi=true`
        // flag during build.
        versionCode = flutter.versionCode
        versionName = flutter.versionName
    }

    signingConfigs {
        if (hasUploadKey) {
            create("upload") {
                storeFile = file(uploadKey.getProperty("storeFile"))
                storePassword = uploadKey.getProperty("storePassword")
                keyAlias = uploadKey.getProperty("keyAlias")
                keyPassword = uploadKey.getProperty("keyPassword")
            }
        }
    }

    buildTypes {
        release {
            // Without an upload key, the debug key: `flutter run --release` still works, and
            // the release workflow refuses to publish such a build
            signingConfig = signingConfigs.getByName(if (hasUploadKey) "upload" else "debug")
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }
}

kotlin {
    compilerOptions {
        jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17
    }
}

flutter {
    source = "../.."
}

dependencies {
    // Google Play's in-app review, asked for through MainActivity; a Flutter plugin for it
    // would apply the Kotlin Gradle Plugin, which Flutter is dropping for plugins
    implementation("com.google.android.play:review:2.0.2")
}
