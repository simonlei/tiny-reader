plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
    id("org.jetbrains.kotlin.plugin.serialization")
}

android {
    namespace = "com.simonlei.tinyreader"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.simonlei.tinyreader"
        minSdk = 26
        targetSdk = 35
        // versionCode 必须单调递增。本地默认 1；CI 通过
        // ORG_GRADLE_PROJECT_versionCode 注入 github.run_number
        //（Gradle 会自动把该前缀的环境变量映射为 project property）。
        versionCode = (project.findProperty("versionCode") as String? ?: "1").toInt()
        versionName = "0.1.0"
    }

    // 签名配置：本地无 keystore 时保持未设置；
    // CI 传入 keystore 路径与口令后即可产出已签名的 release 包。
    signingConfigs {
        create("release") {
            val ksPath = project.findProperty("RELEASE_STORE_FILE") as String?
            if (!ksPath.isNullOrBlank()) {
                storeFile = file(ksPath)
                storePassword = project.findProperty("RELEASE_STORE_PASSWORD") as String?
                keyAlias = project.findProperty("RELEASE_KEY_ALIAS") as String?
                keyPassword = project.findProperty("RELEASE_KEY_PASSWORD") as String?
            }
        }
    }

    buildTypes {
        debug {
            applicationIdSuffix = ".debug"
        }
        release {
            // 仅在 keystore 配置到位时挂签名，缺失则退化为未签名包，不阻断构建。
            val ksPath = project.findProperty("RELEASE_STORE_FILE") as String?
            if (!ksPath.isNullOrBlank()) {
                signingConfig = signingConfigs.getByName("release")
            }
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    buildFeatures {
        compose = true
    }

    packaging {
        resources.excludes += setOf("/META-INF/{AL2.0,LGPL2.1}")
    }
}

dependencies {
    implementation("androidx.core:core-ktx:1.13.1")
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.7")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.7")

    implementation(platform("androidx.compose:compose-bom:2024.10.01"))
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-graphics")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-extended")
    debugImplementation("androidx.compose.ui:ui-tooling")

    implementation("com.squareup.okhttp3:okhttp:4.12.0")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.7.3")
}
