import com.diffplug.spotless.LineEnding

plugins {
    id("com.diffplug.spotless") version "8.12.0"
}

repositories {
    mavenCentral()
}

val profile = providers.gradleProperty("profile").orElse("gjf").get()

spotless {
    lineEndings = LineEnding.UNIX
    java {
        target(fileTree("work") { include("*.java") })
        when (profile) {
            "gjf" -> googleJavaFormat("2.0.0")
            "gjf-reflow" -> googleJavaFormat("2.0.0").reflowLongStrings()
            "import-order" -> importOrder()
            "unused-imports" -> removeUnusedImports()
            "toggle" -> {
                toggleOffOn()
                trimTrailingWhitespace()
            }
            "whitespace" -> {
                trimTrailingWhitespace()
                endWithNewline()
            }
            else -> throw GradleException("unknown oracle profile: $profile")
        }
    }
}
