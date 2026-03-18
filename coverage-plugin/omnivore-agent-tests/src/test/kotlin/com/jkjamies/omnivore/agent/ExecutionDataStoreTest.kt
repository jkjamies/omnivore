package com.jkjamies.omnivore.agent

import com.jkjamies.omnivore.agent.runtime.ExecutionDataStore
import org.junit.jupiter.api.Assertions.assertArrayEquals
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNotNull
import org.junit.jupiter.api.Assertions.assertSame
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class ExecutionDataStoreTest {

    @Test
    fun `creates probe array for new class`() {
        val store = ExecutionDataStore()
        val probes = store.getOrCreateProbes(1L, "com/example/Foo", 10)

        assertEquals(10, probes.size)
        assertTrue(probes.all { !it })
    }

    @Test
    fun `returns same probe array for same class ID`() {
        val store = ExecutionDataStore()
        val probes1 = store.getOrCreateProbes(1L, "com/example/Foo", 10)
        val probes2 = store.getOrCreateProbes(1L, "com/example/Foo", 10)

        assertSame(probes1, probes2)
    }

    @Test
    fun `tracks probe hits correctly`() {
        val store = ExecutionDataStore()
        val probes = store.getOrCreateProbes(1L, "com/example/Foo", 5)

        // Simulate execution hitting probes 0 and 3
        probes[0] = true
        probes[3] = true

        val data = store.getData(1L)
        assertNotNull(data)
        assertTrue(data!!.probes[0])
        assertFalse(data.probes[1])
        assertFalse(data.probes[2])
        assertTrue(data.probes[3])
        assertFalse(data.probes[4])
    }

    @Test
    fun `getAllData returns all registered classes`() {
        val store = ExecutionDataStore()
        store.getOrCreateProbes(1L, "com/example/Foo", 5)
        store.getOrCreateProbes(2L, "com/example/Bar", 3)

        val allData = store.getAllData()
        assertEquals(2, allData.size)
    }

    @Test
    fun `isEmpty returns true when no data`() {
        val store = ExecutionDataStore()
        assertTrue(store.isEmpty())
    }

    @Test
    fun `isEmpty returns false after registration`() {
        val store = ExecutionDataStore()
        store.getOrCreateProbes(1L, "com/example/Foo", 5)
        assertFalse(store.isEmpty())
    }

    @Test
    fun `reset clears all data`() {
        val store = ExecutionDataStore()
        store.getOrCreateProbes(1L, "com/example/Foo", 5)
        store.reset()
        assertTrue(store.isEmpty())
    }
}
