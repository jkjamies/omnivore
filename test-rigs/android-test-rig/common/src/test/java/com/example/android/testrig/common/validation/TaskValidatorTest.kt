package com.example.android.testrig.common.validation

import io.kotest.core.spec.style.FunSpec
import io.kotest.matchers.shouldBe

class TaskValidatorTest : FunSpec({

    test("valid title passes validation") {
        val result = TaskValidator.validateTitle("Buy groceries")
        result.isValid shouldBe true
        result.errors shouldBe emptyList()
    }

    test("blank title fails validation") {
        val result = TaskValidator.validateTitle("")
        result.isValid shouldBe false
    }

    test("too short title fails validation") {
        val result = TaskValidator.validateTitle("ab")
        result.isValid shouldBe false
    }

    test("sanitizeTitle trims and collapses spaces") {
        val result = TaskValidator.sanitizeTitle("  hello   world  ")
        result shouldBe "hello world"
    }

    // Intentionally not testing: validateDescription, validateTask, sanitizeDescription, title > 200
})
