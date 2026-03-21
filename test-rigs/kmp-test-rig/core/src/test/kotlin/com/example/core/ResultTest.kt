package com.example.core

import com.example.core.model.OpResult
import io.kotest.core.spec.style.FunSpec
import io.kotest.matchers.nulls.shouldBeNull
import io.kotest.matchers.shouldBe

class ResultTest : FunSpec({

    test("success holds value") {
        val result = OpResult.Success(42)
        result.isSuccess() shouldBe true
        result.isFailure() shouldBe false
        result.getOrNull() shouldBe 42
        result.errorOrNull().shouldBeNull()
    }

    test("failure holds message") {
        val result = OpResult.Failure("bad input")
        result.isSuccess() shouldBe false
        result.isFailure() shouldBe true
        result.getOrNull().shouldBeNull()
        result.errorOrNull() shouldBe "bad input"
    }

    // Intentionally not testing: map()
})
