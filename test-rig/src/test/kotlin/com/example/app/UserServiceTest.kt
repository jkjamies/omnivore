package com.example.app

import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test

class UserServiceTest {

    private lateinit var service: UserService

    @BeforeEach
    fun setUp() {
        service = UserService()
    }

    @Test
    fun `addUser succeeds`() {
        assertTrue(service.addUser(User(1, "Alice", "alice@example.com")))
    }

    @Test
    fun `addUser rejects duplicate id`() {
        service.addUser(User(1, "Alice", "alice@example.com"))
        assertFalse(service.addUser(User(1, "Bob", "bob@example.com")))
    }

    @Test
    fun `getUser returns user`() {
        service.addUser(User(1, "Alice", "alice@example.com"))
        val user = service.getUser(1)
        assertNotNull(user)
        assertEquals("Alice", user!!.name)
    }

    @Test
    fun `getUser returns null for missing`() {
        assertNull(service.getUser(999))
    }

    @Test
    fun `getActiveUsers filters inactive`() {
        service.addUser(User(1, "Alice", "alice@example.com", active = true))
        service.addUser(User(2, "Bob", "bob@example.com", active = false))
        assertEquals(1, service.getActiveUsers().size)
    }

    // deactivateUser intentionally not tested
    // removeUser intentionally not tested
    // findByEmail intentionally not tested
}
