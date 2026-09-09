import java.io.FileInputStream
import java.util.Properties

plugins {
    id("com.android.application")
    // The Flutter Gradle Plugin must be applied after the Android and Kotlin Gradle plugins.
    id("dev.flutter.flutter-gradle-plugin")
}

// 正式签名配置：android/key.properties（不入库，见 key.properties.example）。
// 首次发布前生成 keystore（务必长期备份，升级必须使用同一 keystore 签名）：
//   keytool -genkey -v -keystore ../tst_xiangqi-release.jks ^
//           -keyalg RSA -keysize 2048 -validity 36500 -alias tst_xiangqi
val keystorePropertiesFile = rootProject.file("key.properties")
val keystoreProperties = Properties().apply {
    if (keystorePropertiesFile.exists()) {
        FileInputStream(keystorePropertiesFile).use { load(it) }
    }
}

android {
    namespace = "com.tst.xiangqi"
    compileSdk = flutter.compileSdkVersion
    ndkVersion = flutter.ndkVersion

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    defaultConfig {
        // TODO: Specify your own unique Application ID (https://developer.android.com/studio/build/application-id.html).
        applicationId = "com.tst.xiangqi"
        // You can update the following values to match your application needs.
        // For more information, see: https://flutter.dev/to/review-gradle-config.
        minSdk = flutter.minSdkVersion
        targetSdk = flutter.targetSdkVersion
        ndk {
            // arm64-v8a：官方预编译引擎（真机）
            // x86_64：NDK 交叉编译（模拟器，官方未发布 x86_64 Android 版）
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
        // Uses the version code from pubspec.yaml. When using split APKs, 1000 * ABI_VERSION
        // is added automatically by Flutter. (https://developer.android.com/studio/build/configure-apk-splits#configure-APK-versions)
        // You can force using the value of versionCode by specifying the `-P force-version-code-ignoring-abi=true`
        // flag during build.
        versionCode = flutter.versionCode
        versionName = flutter.versionName
    }

    signingConfigs {
        create("release") {
            // 仅在提供 key.properties 时填充；缺失时保持未配置，
            // buildTypes.release 会回退 debug 签名（保证本地 flutter run --release 可用）
            if (keystorePropertiesFile.exists()) {
                // storeFile 支持绝对路径，或相对 android/ 目录的相对路径
                storeFile = rootProject.file(keystoreProperties.getProperty("storeFile"))
                storePassword = keystoreProperties.getProperty("storePassword")
                keyAlias = keystoreProperties.getProperty("keyAlias")
                keyPassword = keystoreProperties.getProperty("keyPassword")
            }
        }
    }

    buildTypes {
        release {
            // 有 key.properties 时用正式签名（正式分发 / 长期升级必须），
            // 否则回退 debug 签名（仅限本地开发验证，不可用于分发）
            signingConfig = if (keystorePropertiesFile.exists()) {
                signingConfigs.getByName("release")
            } else {
                signingConfigs.getByName("debug")
            }
        }
    }

    packaging {
        jniLibs {
            // 解压引擎 so 到 nativeLibraryDir：
            // AGP 默认不解压（useLegacyPackaging=false），导致
            // nativeLibraryDir 下无真实文件，Process.start 执行引擎会失败
            useLegacyPackaging = true
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
