plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
    id("org.jetbrains.kotlin.plugin.serialization")
}

// 订阅推荐目录与桌面端共用同一份 src/data/feedCatalog.json，构建时拷进 assets，
// 免得两处各存一份、改了一边忘了另一边。
val feedCatalogAssetDir = layout.buildDirectory.dir("generated/assets/feedCatalog")
val copyFeedCatalog = tasks.register<Copy>("copyFeedCatalog") {
    from(rootProject.file("../src/data/feedCatalog.json"))
    into(feedCatalogAssetDir)
}
tasks.named("preBuild").configure { dependsOn(copyFeedCatalog) }

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

    // minSdk >= 24 时 AGP 默认不写 V1（JAR）证书。vivo 等安装器只查
    // META-INF 里的开发者证书，没有就报「软件包似乎无效 / 缺乏开发者证书」
    // （错误码 -103）。debug / release 都显式打开 V1+V2+V3。
    // 未传入正式 keystore 时用仓库里的 dev.keystore（口令公开，只为了能安装），
    // 避免打出完全没有证书的 app-release-unsigned.apk。
    signingConfigs {
        getByName("debug") {
            enableV1Signing = true
            enableV2Signing = true
            enableV3Signing = true
        }
        create("release") {
            val ksPath = project.findProperty("RELEASE_STORE_FILE") as String?
            if (!ksPath.isNullOrBlank()) {
                storeFile = file(ksPath)
                storePassword = project.findProperty("RELEASE_STORE_PASSWORD") as String?
                keyAlias = project.findProperty("RELEASE_KEY_ALIAS") as String?
                keyPassword = project.findProperty("RELEASE_KEY_PASSWORD") as String?
            } else {
                storeFile = rootProject.file("dev.keystore")
                storePassword = "tinyreader"
                keyAlias = "tinyreader"
                keyPassword = "tinyreader"
            }
            enableV1Signing = true
            enableV2Signing = true
            enableV3Signing = true
        }
    }

    buildTypes {
        debug {
            applicationIdSuffix = ".debug"
        }
        release {
            signingConfig = signingConfigs.getByName("release")
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

    sourceSets {
        getByName("main") { assets.srcDir(feedCatalogAssetDir) }
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

    testImplementation("junit:junit:4.13.2")
}

// AGP 在 minSdk >= 24 时即使 enableV1Signing = true 也可能不写 JAR 证书。
// 打包后再用 apksigner 强制签上 V1+V2+V3，并用 --min-sdk-version 21 验收：
// 没有 META-INF/MANIFEST.MF 时 verify 会失败（Missing META-INF/MANIFEST.MF）。
val androidSdkDir = android.sdkDirectory

fun apksignerJar(sdkDir: File): File {
    val tools = File(sdkDir, "build-tools")
        .listFiles { file -> file.isDirectory }
        ?.maxByOrNull { it.name }
        ?: error("Android build-tools not found under $sdkDir")
    val jar = File(tools, "lib/apksigner.jar")
    if (!jar.exists()) error("apksigner.jar not found in $tools")
    return jar
}

fun Project.runApksigner(jar: File, args: List<String>, storePass: String?, keyPass: String?) {
    val result = providers.exec {
        if (storePass != null) environment("TR_KS_PASS", storePass)
        if (keyPass != null) environment("TR_KEY_PASS", keyPass)
        val javaBin = File(System.getProperty("java.home"), "bin")
        val javaExe = listOf(File(javaBin, "java.exe"), File(javaBin, "java")).first { it.exists() }
        commandLine(listOf(javaExe.absolutePath, "-jar", jar.absolutePath) + args)
    }
    result.result.get().assertNormalExitValue()
}

afterEvaluate {
    listOf("debug", "release").forEach { variant ->
        val cap = variant.replaceFirstChar { it.uppercase() }
        val signing = android.signingConfigs.getByName(variant)
        val resign = tasks.register("resign${cap}Apk") {
            group = "build"
            description = "Sign $variant APK with v1+v2+v3 so sideload installers accept it"
            dependsOn("package$cap")
            doLast {
                val apk = layout.buildDirectory.file("outputs/apk/$variant/app-$variant.apk").get().asFile
                if (!apk.exists()) error("APK not found: ${apk.absolutePath}")
                val store = signing.storeFile ?: error("missing keystore for $variant")
                val alias = signing.keyAlias ?: error("missing key alias for $variant")
                val storePass = signing.storePassword ?: error("missing store password for $variant")
                val keyPass = signing.keyPassword ?: error("missing key password for $variant")
                val signer = apksignerJar(androidSdkDir)
                project.runApksigner(
                    signer,
                    listOf(
                        "sign",
                        "--ks", store.absolutePath,
                        "--ks-pass", "env:TR_KS_PASS",
                        "--key-pass", "env:TR_KEY_PASS",
                        "--ks-key-alias", alias,
                        "--v1-signing-enabled", "true",
                        "--v2-signing-enabled", "true",
                        "--v3-signing-enabled", "true",
                        apk.absolutePath,
                    ),
                    storePass,
                    keyPass,
                )
                project.runApksigner(
                    signer,
                    listOf("verify", "--min-sdk-version", "21", apk.absolutePath),
                    null,
                    null,
                )
            }
        }
        tasks.named("package$cap").configure { finalizedBy(resign) }
        tasks.named("assemble$cap").configure { dependsOn(resign) }
        tasks.named("install$cap").configure { dependsOn(resign) }
    }
}
