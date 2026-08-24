plugins {
    id("com.android.application")
}

android {
    namespace = "ai.nexus.shell"
    compileSdk = 36

    defaultConfig {
        applicationId = "ai.nexus.shell"
        minSdk = 26
        targetSdk = 36
        versionCode = 102
        versionName = "1.02.0"
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}
