"""Every ``>>>`` example in the package, run as a test.

Extension-module docstrings are reachable through ``doctest.testmod``
the same way pure-Python ones are, so the examples in the Rust doc
comments are covered here too.
"""

import datetime
import doctest
import fractions

import pytest

import fasti
import fasti._fasti


@pytest.mark.parametrize(
    "module",
    [fasti._fasti, fasti, fasti.calendars],
    ids=lambda m: m.__name__,
)
def test_docstring_examples(module):
    globs = {
        "fasti": fasti,
        "calendars": fasti.calendars,
        "datetime": datetime,
        "Fraction": fractions.Fraction,
    }
    result = doctest.testmod(
        module,
        globs=globs,
        verbose=False,
        optionflags=doctest.NORMALIZE_WHITESPACE,
        report=True,
    )
    assert result.attempted > 0
    assert result.failed == 0, f"{result.failed} docstring example(s) failed"


def test_readme_examples():
    """The ``>>>`` examples in the packaged README.

    Extracted per fenced block, because doctest would otherwise read the
    closing fence as part of the expected output.
    """
    import pathlib
    import re

    readme = pathlib.Path(__file__).resolve().parents[1] / "README.md"
    if not readme.is_file():  # running against an installed wheel
        pytest.skip("README.md is not next to the tests")

    blocks = [
        block
        for block in re.findall(r"```python\n(.*?)```", readme.read_text(), re.S)
        if ">>>" in block
    ]
    assert blocks, "the README should carry runnable examples"

    parser = doctest.DocTestParser()
    runner = doctest.DocTestRunner(
        optionflags=doctest.ELLIPSIS | doctest.NORMALIZE_WHITESPACE
    )
    for i, block in enumerate(blocks):
        test = parser.get_doctest(
            block,
            {"fasti": fasti, "calendars": fasti.calendars, "datetime": datetime,
             "Fraction": fractions.Fraction},
            f"README block {i}",
            str(readme),
            0,
        )
        runner.run(test, clear_globs=False)
    assert runner.failures == 0, f"{runner.failures} README example(s) failed"
