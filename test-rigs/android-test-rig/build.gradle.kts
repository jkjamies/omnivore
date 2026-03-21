plugins {
    id("com.android.application") version "8.8.2" apply false
    kotlin("android") version "2.1.10" apply false
    kotlin("jvm") version "2.1.10" apply false
    id("io.github.jkjamies.omnivore")
}

omnivore {
    reports {
        json { enabled.set(true) }
        html { enabled.set(true) }
        markdown { enabled.set(true) }
    }
    instrumentedTests {
        enabled.set(true)
    }
    dependencies {
        enabled.set(true)
    }
    dashboard {
        url.set(providers.gradleProperty("omnivore.dashboard.url").orElse("http://localhost:3000"))
    }
}
