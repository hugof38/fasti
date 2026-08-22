"""Every >>> example in the package and the README runs."""

import doctest
import inspect
import pathlib
from collections.abc import Iterator

import pytest

import fasti
from fasti import _fasti, calendars
from fasti.calendars import france, uk, us

MODULES = [fasti, _fasti, calendars, france, uk, us]
# Doc comments on the Rust items become these docstrings, so this is
# also what checks them.
OPTIONS = doctest.ELLIPSIS


def documented() -> Iterator[tuple[str, str]]:
    """Every docstring in the package, named by where it was found."""
    for module in MODULES:
        yield module.__name__, module.__doc__ or ""
        for name, member in vars(module).items():
            if name.startswith("_"):
                continue
            if inspect.isbuiltin(member):
                yield f"{module.__name__}.{name}", member.__doc__ or ""
            if not inspect.isclass(member):
                continue
            yield f"{module.__name__}.{name}", member.__doc__ or ""
            for attribute, value in vars(member).items():
                # A class constant's __doc__ is the class's own; a
                # staticmethod's is only its own through the class.
                if attribute.startswith("_") or isinstance(value, member):
                    continue
                bound = getattr(member, attribute)
                yield f"{module.__name__}.{name}.{attribute}", bound.__doc__ or ""


def with_examples() -> list[pytest.param]:  # type: ignore[valid-type]
    # The same class reached through the package and through the
    # extension is one docstring, and one test.
    seen: dict[str, str] = {}
    for name, text in documented():
        if ">>>" in text:
            seen.setdefault(text, name)
    return [pytest.param(name, text, id=name) for text, name in seen.items()]


WITH_EXAMPLES = with_examples()

# The binding's own README, and the crate's, which advertises it. The
# sdist keeps this layout, so both are found from an unpacked one.
READMES = [
    pathlib.Path(__file__).parents[1] / "README-py.md",
    pathlib.Path(__file__).parents[3] / "README.md",
]


def run(name: str, text: str) -> None:
    parser = doctest.DocTestParser()
    test = parser.get_doctest(text, {}, name, None, None)
    runner = doctest.DocTestRunner(optionflags=OPTIONS)
    output: list[str] = []
    runner.run(test, out=output.append)
    assert runner.failures == 0, "".join(output)
    assert test.examples, f"{name} has no examples"


@pytest.mark.parametrize(("name", "text"), WITH_EXAMPLES)
def test_the_examples_in_the_docstring_run(name: str, text: str) -> None:
    run(name, text)


@pytest.mark.parametrize("readme", READMES, ids=lambda path: path.name)
def test_the_examples_in_the_readme_run(readme: pathlib.Path) -> None:
    if not readme.exists():  # pragma: no cover - only in a trimmed sdist
        pytest.skip(f"no {readme.name} in this tree")
    run(readme.name, readme.read_text())


def test_there_are_examples_to_run() -> None:
    # A docstring that loses its examples should fail here, not silently
    # stop being checked.
    assert len(WITH_EXAMPLES) >= 20
