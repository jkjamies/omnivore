package com.example.app

import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

class StringUtilsTest {

    @Test
    fun `reverse works`() {
        assertEquals("dcba", StringUtils.reverse("abcd"))
    }

    @Test
    fun `isPalindrome detects palindromes`() {
        assertTrue(StringUtils.isPalindrome("racecar"))
        assertTrue(StringUtils.isPalindrome("A man a plan a canal Panama"))
    }

    @Test
    fun `isPalindrome rejects non-palindromes`() {
        assertFalse(StringUtils.isPalindrome("hello"))
    }

    @Test
    fun `truncate returns input when short enough`() {
        assertEquals("hi", StringUtils.truncate("hi", 10))
    }

    @Test
    fun `truncate adds ellipsis`() {
        assertEquals("hell...", StringUtils.truncate("hello world", 7))
    }

    // truncate edge cases (maxLength < 0, maxLength <= 3) intentionally not tested

    // countWords intentionally not tested
    // toCamelCase intentionally not tested
}
