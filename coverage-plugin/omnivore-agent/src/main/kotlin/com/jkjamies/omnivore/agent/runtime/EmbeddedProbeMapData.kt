package com.jkjamies.omnivore.agent.runtime

/**
 * Carries a class's probe map inside the class itself, for the Android
 * build-time path. See `EmbeddedProbeMap` for why.
 *
 * Binary retention: present in the instrumented `.class` files the build reads
 * back, stripped by D8 when dexing, so it costs nothing at runtime. It is a real
 * type (shipped in the runtime JAR) so no tool sees a reference to a missing
 * class.
 */
@Retention(AnnotationRetention.BINARY)
@Target(AnnotationTarget.CLASS)
annotation class EmbeddedProbeMapData(
    /** One class's probe map in the .probes format, Base64-encoded and split into chunks. */
    val chunks: Array<String>,
)
