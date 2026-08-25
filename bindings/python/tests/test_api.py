"""The stub and the module say the same thing, in both directions."""

import ast
import inspect
import pathlib

import pytest

import fasti
from fasti import _fasti

STUB = ast.parse(pathlib.Path(_fasti.__file__).with_name("_fasti.pyi").read_text())


def stub_top_level() -> set[str]:
    return {
        node.name
        for node in STUB.body
        if isinstance(node, (ast.ClassDef, ast.FunctionDef))
    }


def stub_members(class_name: str) -> set[str]:
    for node in STUB.body:
        if isinstance(node, ast.ClassDef) and node.name == class_name:
            return {
                child.name
                for child in node.body
                if isinstance(child, ast.FunctionDef) and not child.name.startswith("_")
            } | {
                target.id
                for child in node.body
                if isinstance(child, ast.AnnAssign)
                for target in [child.target]
                if isinstance(target, ast.Name)
            }
    raise AssertionError(f"{class_name} is not in the stub")


def module_top_level() -> set[str]:
    return {name for name in dir(_fasti) if not name.startswith("__")}


CLASSES = sorted(
    name for name in module_top_level() if inspect.isclass(getattr(_fasti, name))
)


def test_the_stub_declares_everything_the_module_exports() -> None:
    assert module_top_level() - stub_top_level() == set()


def test_the_module_exports_everything_the_stub_declares() -> None:
    assert stub_top_level() - module_top_level() == set()


@pytest.mark.parametrize("name", CLASSES)
def test_each_class_has_the_same_members_in_both(name: str) -> None:
    if name == "FastiError":
        pytest.skip("an exception class carries only what ValueError gives it")
    runtime = {
        member for member in vars(getattr(_fasti, name)) if not member.startswith("_")
    }
    assert runtime == stub_members(name)


def test_the_package_re_exports_exactly_the_public_module() -> None:
    public = {name for name in module_top_level() if not name.startswith("_")}
    assert set(fasti.__all__) == public | {"calendars"}
    for name in fasti.__all__:
        assert getattr(fasti, name) is not None


def test_all_is_sorted_so_a_new_name_lands_where_it_belongs() -> None:
    assert fasti.__all__ == sorted(fasti.__all__)


def test_every_class_says_it_lives_in_the_package_not_the_extension() -> None:
    # Pickle looks classes up by __module__, and the package is where
    # they are re-exported.
    for name in CLASSES:
        if name == "FastiError":
            continue
        assert getattr(_fasti, name).__module__ == "fasti"


def test_the_calendars_package_lists_every_built_in() -> None:
    from fasti import calendars
    from fasti.calendars import france, uk, us

    registry = {
        f"{module.__name__.rsplit('.', 1)[1]}::{name}"
        for module in (france, uk, us)
        for name in module.__all__
    } | {name for name in calendars.__all__ if name.isupper()}
    for name in registry:
        assert isinstance(_fasti._builtin(name), fasti.Calendar)
    assert len(registry) == 12


def test_every_public_name_is_documented() -> None:
    for name in module_top_level():
        if name.startswith("_"):
            continue
        assert getattr(_fasti, name).__doc__, f"{name} has no docstring"
    for name in CLASSES:
        if name == "FastiError":
            continue
        owner = getattr(_fasti, name)
        for member in vars(owner):
            if member.startswith("_") or member.isupper():
                continue
            # Read it off the class: a staticmethod entry in the type
            # dict carries staticmethod's own docstring, not the item's.
            assert getattr(owner, member).__doc__, f"{name}.{member} has no docstring"


def test_the_version_is_the_one_in_the_manifest() -> None:
    from importlib.metadata import version

    assert fasti.__version__ == version("fasti-dates")
    manifest = pathlib.Path(__file__).parents[1] / "Cargo.toml"
    if not manifest.exists():  # pragma: no cover - only in a trimmed sdist
        return
    declared = [
        line.split("=", 1)[1].strip().strip('"')
        for line in manifest.read_text().splitlines()
        if line.startswith("version = ")
    ]
    assert declared[:1] == [fasti.__version__]
