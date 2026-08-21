"""The shape of the package: exports, typing marker, versions, enums."""

from datetime import date
import importlib.metadata

import pytest

import fasti


def test_everything_in_all_is_importable():
    for name in fasti.__all__:
        assert hasattr(fasti, name), name


def test_version_matches_the_installed_distribution():
    """The distribution is `fasti-py`; the import name is `fasti`."""
    assert fasti.__version__ == importlib.metadata.version("fasti-py")


def test_package_ships_a_typing_marker():
    import pathlib

    root = pathlib.Path(fasti.__file__).parent
    assert (root / "py.typed").is_file()
    assert (root / "_fasti.pyi").is_file()


def test_calendar_module_attributes_match_the_registry():
    from fasti import calendars

    assert calendars.load("nyse").name == calendars.US_NYSE.name
    assert set(calendars.names()) == set(fasti.Calendar.names())
    for name in calendars.names():
        assert isinstance(calendars.load(name), fasti.Calendar)


def test_weekday_numbering_matches_datetime():
    day = date(2026, 7, 6)  # a Monday
    assert fasti.Weekday.MON.isoweekday == day.isoweekday()
    assert fasti.Weekday.MON.weekday == day.weekday()
    assert int(fasti.Weekday.SUN) == 7
    assert fasti.Weekday.parse("monday") == fasti.Weekday.MON
    assert fasti.Weekday.parse(1) == fasti.Weekday.MON


def test_enum_members_parse_from_strings():
    assert fasti.BusinessDayConvention.parse("mf") is fasti.BusinessDayConvention.MODIFIED_FOLLOWING
    assert fasti.BusinessDayConvention.parse("ModifiedFollowing") == (
        fasti.BusinessDayConvention.MODIFIED_FOLLOWING
    )
    assert fasti.DateGenerationRule.parse("forwards") == fasti.DateGenerationRule.FORWARD
    assert fasti.WeekendShift.parse("us") == fasti.WeekendShift.SAT_BACK_SUN_FORWARD


def test_unknown_names_say_what_was_expected():
    with pytest.raises(fasti.FastiError, match="expected one of"):
        fasti.calendars.NULL.adjust(date(2026, 1, 1), "sideways")


def test_objects_are_immutable():
    cal = fasti.calendars.NULL
    with pytest.raises(AttributeError):
        cal.name = "other"
    with pytest.raises(AttributeError):
        fasti.Period("6M").length = 7


def test_readme_quickstart_runs():
    """The example the README leads with."""
    import fasti
    from fasti import calendars

    nyse = calendars.US_NYSE
    assert nyse.is_business_day(date(2026, 7, 6))
    schedule = fasti.Schedule(date(2025, 1, 15), date(2030, 1, 15), "6M", calendars.US_SETTLEMENT)
    accrued = sum(
        fasti.year_fraction(start, end, "ACT/ACT ISDA") for start, end in schedule.periods()
    )
    assert 4.9 < float(accrued) < 5.1


def _stub_names():
    """Top-level names declared in the stub file."""
    import ast
    import pathlib

    stub = pathlib.Path(fasti.__file__).parent / "_fasti.pyi"
    tree = ast.parse(stub.read_text())
    names = set()
    for node in tree.body:
        if isinstance(node, (ast.ClassDef, ast.FunctionDef)):
            names.add(node.name)
        elif isinstance(node, ast.AnnAssign) and isinstance(node.target, ast.Name):
            names.add(node.target.id)
    return names


def test_stub_covers_every_public_name():
    """The stub and the compiled module must not drift apart."""
    import fasti._fasti as core

    exported = {name for name in dir(core) if not name.startswith("_")} | {"__version__"}
    missing = exported - _stub_names()
    assert not missing, f"missing from _fasti.pyi: {sorted(missing)}"


def test_stub_declares_nothing_the_module_lacks():
    import fasti._fasti as core

    aliases = {name for name in _stub_names() if name.endswith("Like")}
    extra = _stub_names() - aliases - set(dir(core))
    assert not extra, f"declared in _fasti.pyi but absent from the module: {sorted(extra)}"


def test_stub_classes_declare_their_members():
    """Every method on a bound class is described by the stub."""
    import ast
    import pathlib

    stub = pathlib.Path(fasti.__file__).parent / "_fasti.pyi"
    tree = ast.parse(stub.read_text())
    import fasti._fasti as core

    for node in tree.body:
        if not isinstance(node, ast.ClassDef):
            continue
        cls = getattr(core, node.name, None)
        if cls is None:
            continue
        declared = {
            item.name
            for item in node.body
            if isinstance(item, (ast.FunctionDef, ast.AnnAssign, ast.ClassDef))
            and not isinstance(item, ast.AnnAssign)
        } | {
            item.target.id
            for item in node.body
            if isinstance(item, ast.AnnAssign) and isinstance(item.target, ast.Name)
        }
        public = {name for name in vars(cls) if not name.startswith("_")}
        missing = public - declared
        assert not missing, f"{node.name} is missing {sorted(missing)} from the stub"
