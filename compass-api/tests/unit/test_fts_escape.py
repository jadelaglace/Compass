"""Unit tests for FTS query escaping."""
import pytest

from src.db.database import _escape_fts_query


class TestEscapeFtsQuery:
    def test_strips_single_quotes(self):
        """Single quotes are SQL injection core vectors and must be stripped."""
        assert "'" not in _escape_fts_query("test'query")

    def test_strips_double_quotes(self):
        """Double quotes are FTS5 phrase delimiters and must be stripped."""
        assert '"' not in _escape_fts_query('test"query')

    def test_strips_fts5_operators(self):
        """FTS5 operators (*+-^:(){}[]~!) must be stripped."""
        for ch in "*+-^:(){}[]~!":
            result = _escape_fts_query(f"test{ch}query")
            assert ch not in result, f"Character {ch!r} not stripped"

    def test_strips_fts5_boolean_keywords(self):
        """AND/OR/NOT keywords must be stripped (case-insensitive, word boundaries)."""
        assert "AND" not in _escape_fts_query("test AND query").upper()
        assert "OR" not in _escape_fts_query("test OR query").upper()
        assert "NOT" not in _escape_fts_query("test NOT query").upper()

    def test_empty_query_returns_safe_phrase(self):
        """Empty/whitespace-only input returns empty phrase (match-nothing)."""
        assert _escape_fts_query("") == '""'
        assert _escape_fts_query("   ") == '""'

    def test_query_exceeding_200_chars_raises(self):
        """Queries over 200 characters must raise ValueError."""
        long_query = "a" * 201
        with pytest.raises(ValueError, match="exceeds maximum length"):
            _escape_fts_query(long_query)

    def test_200_char_query_is_ok(self):
        """Exactly 200 characters should be accepted without error."""
        query = "a" * 200
        result = _escape_fts_query(query)
        assert len(result) <= 200

    def test_normal_query_preserved(self):
        """Normal alphanumeric queries should be returned with spaces collapsed."""
        result = _escape_fts_query("hello world")
        assert "hello" in result and "world" in result
        assert "  " not in result  # no double spaces

    def test_collapses_multiple_spaces(self):
        """Multiple spaces between words should be collapsed to one."""
        result = _escape_fts_query("hello    world")
        assert "  " not in result
        assert "hello" in result and "world" in result
