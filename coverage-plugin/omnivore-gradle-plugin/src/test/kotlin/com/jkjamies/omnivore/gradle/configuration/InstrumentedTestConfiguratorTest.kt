package com.jkjamies.omnivore.gradle.configuration

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class InstrumentedTestConfiguratorTest {

    @Test
    fun `no devices attached`() {
        val output = "List of devices attached\n\n"
        assertEquals(emptyList<String>(), InstrumentedTestConfigurator.parseConnectedDevices(output))
    }

    @Test
    fun `daemon startup lines are not devices`() {
        val output = """
            * daemon not running; starting now at tcp:5037
            * daemon started successfully
            List of devices attached

        """.trimIndent()
        assertEquals(emptyList<String>(), InstrumentedTestConfigurator.parseConnectedDevices(output))
    }

    @Test
    fun `only devices in the device state count`() {
        val output = """
            List of devices attached
            emulator-5554	device
            R58M123ABC	unauthorized
            192.168.1.20:5555	offline
            emulator-5556	device product:sdk_gphone64 model:sdk_gphone64 transport_id:2
        """.trimIndent()
        assertEquals(
            listOf("emulator-5554", "emulator-5556"),
            InstrumentedTestConfigurator.parseConnectedDevices(output),
        )
    }
}
