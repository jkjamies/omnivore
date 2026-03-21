package com.example.app.util

import io.kotest.core.spec.style.FunSpec
import io.kotest.matchers.shouldBe

class StringUtilsTest : FunSpec({

    test("reverse works") {
        StringUtils.reverse("abcd") shouldBe "dcba"
    }

    test("isPalindrome detects palindromes") {
        StringUtils.isPalindrome("racecar") shouldBe true
        StringUtils.isPalindrome("A man a plan a canal Panama") shouldBe true
    }

    test("isPalindrome rejects non-palindromes") {
        StringUtils.isPalindrome("hello") shouldBe false
    }

    test("truncate returns input when short enough") {
        StringUtils.truncate("hi", 10) shouldBe "hi"
    }

    test("truncate adds ellipsis") {
        StringUtils.truncate("hello world", 7) shouldBe "hell..."
    }

    // truncate edge cases (maxLength < 0, maxLength <= 3) intentionally not tested
    // countWords intentionally not tested
    // toCamelCase intentionally not tested
})
