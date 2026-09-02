from lib import normalize


def test_ascii_trim():
    assert normalize(' 12 ') == '12'
