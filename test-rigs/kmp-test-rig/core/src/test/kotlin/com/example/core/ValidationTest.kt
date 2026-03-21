package com.example.core

import io.kotest.core.spec.style.FunSpec
import io.kotest.matchers.shouldBe

class ValidationTest : FunSpec({

    test("valid email passes") {
        Validation.isValidEmail("user@example.com") shouldBe true
    }

    test("blank email fails") {
        Validation.isValidEmail("") shouldBe false
        Validation.isValidEmail("   ") shouldBe false
    }

    test("email without at fails") {
        Validation.isValidEmail("userexample.com") shouldBe false
    }

    // Intentionally not testing: no dot after @, dot at end, isValidName edge cases

    test("sanitize trims and collapses whitespace") {
        Validation.sanitize("  hello   world  ") shouldBe "hello world"
    }

    test("valid id is positive") {
        Validation.isValidId(1) shouldBe true
        Validation.isValidId(0) shouldBe false
        Validation.isValidId(-5) shouldBe false
    }
})
