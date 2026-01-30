"""Tests for brrtrouter_tooling.openapi.fix_impl_controllers (Decimal conversion)."""

import re


class TestConvertF64ToDecimal:
    """Test convert_f64_to_decimal for various numeric strings."""

    def _convert(self, value: str) -> str:
        from brrtrouter_tooling.openapi.fix_impl_controllers import convert_f64_to_decimal

        m = re.search(r"(-?\d+\.?\d*)", value)
        assert m is not None
        return convert_f64_to_decimal(m)

    def test_simple_decimal(self) -> None:
        assert self._convert("123.45") == "rust_decimal::Decimal::new(12345, 2)"

    def test_integer_literal(self) -> None:
        assert self._convert("999.0") == "rust_decimal::Decimal::new(999, 0)"
        assert self._convert("0") == "rust_decimal::Decimal::new(0, 0)"

    def test_negative_decimal(self) -> None:
        assert self._convert("-1.5") == "rust_decimal::Decimal::new(-15, 1)"

    def test_sixteen_plus_digits_no_value_error(self) -> None:
        """16+ significant digits: str(float) uses scientific notation; Decimal avoids ValueError."""
        result = self._convert("12345678901234567.5")
        assert result == "rust_decimal::Decimal::new(123456789012345675, 1)"
        assert "e+" not in result
        assert "ValueError" not in result
