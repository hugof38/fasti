"""The vocabularies: one table per crate enum, and the spellings of it."""

from typing import Any

import pytest

from fasti import (
    BusinessDayConvention,
    DateGenerationRule,
    EasterMethod,
    FastiError,
    Frequency,
    Weekday,
    WeekendShift,
)

# vocabulary, attribute, canonical spelling, aliases
TABLE: list[tuple[type, str, str, list[str]]] = [
    (BusinessDayConvention, "UNADJUSTED", "Unadjusted", []),
    (BusinessDayConvention, "FOLLOWING", "Following", []),
    (BusinessDayConvention, "MODIFIED_FOLLOWING", "ModifiedFollowing", ["modfollowing"]),
    (BusinessDayConvention, "PRECEDING", "Preceding", []),
    (BusinessDayConvention, "MODIFIED_PRECEDING", "ModifiedPreceding", ["modpreceding"]),
    (Weekday, "MON", "Mon", ["Monday"]),
    (Weekday, "TUE", "Tue", ["Tuesday"]),
    (Weekday, "WED", "Wed", ["Wednesday"]),
    (Weekday, "THU", "Thu", ["Thursday"]),
    (Weekday, "FRI", "Fri", ["Friday"]),
    (Weekday, "SAT", "Sat", ["Saturday"]),
    (Weekday, "SUN", "Sun", ["Sunday"]),
    (Frequency, "ANNUAL", "Annual", ["yearly"]),
    (Frequency, "SEMIANNUAL", "Semiannual", ["halfyearly"]),
    (Frequency, "EVERY_FOURTH_MONTH", "EveryFourthMonth", []),
    (Frequency, "QUARTERLY", "Quarterly", []),
    (Frequency, "BIMONTHLY", "Bimonthly", []),
    (Frequency, "MONTHLY", "Monthly", []),
    (Frequency, "EVERY_FOURTH_WEEK", "EveryFourthWeek", []),
    (Frequency, "BIWEEKLY", "Biweekly", ["fortnightly"]),
    (Frequency, "WEEKLY", "Weekly", []),
    (Frequency, "DAILY", "Daily", []),
    (WeekendShift, "NONE", "None", []),
    (WeekendShift, "FORWARD", "Forward", []),
    (WeekendShift, "SUN_FORWARD", "SunForward", ["fed", "sifma"]),
    (WeekendShift, "SAT_BACK_SUN_FORWARD", "SatBackSunForward", []),
    (EasterMethod, "WESTERN", "Western", ["gregorian"]),
    (EasterMethod, "ORTHODOX", "Orthodox", ["julian"]),
    (DateGenerationRule, "FORWARD", "Forward", []),
    (DateGenerationRule, "BACKWARD", "Backward", []),
    (DateGenerationRule, "ZERO", "Zero", []),
]

VOCABULARIES = [
    BusinessDayConvention,
    DateGenerationRule,
    EasterMethod,
    Frequency,
    Weekday,
    WeekendShift,
]


def ident(row: tuple[type, str, str, list[str]]) -> str:
    return f"{row[0].__name__}.{row[1]}"


@pytest.mark.parametrize("row", TABLE, ids=ident)
def test_the_canonical_spelling_is_what_prints(row: tuple[type, str, str, list[str]]) -> None:
    vocabulary, attribute, canonical, _ = row
    member = getattr(vocabulary, attribute)
    assert str(member) == canonical
    assert repr(member) == f"{vocabulary.__name__}.{attribute}"


@pytest.mark.parametrize("row", TABLE, ids=ident)
def test_every_spelling_parses_to_the_same_member(
    row: tuple[type, str, str, list[str]],
) -> None:
    vocabulary, attribute, canonical, aliases = row
    member = getattr(vocabulary, attribute)
    for spelling in [canonical, *aliases]:
        for written in (
            spelling,
            spelling.upper(),
            spelling.lower(),
            f" {spelling} ".replace(" ", "_"),
            "-".join(spelling),
        ):
            assert vocabulary(written) == member
    assert vocabulary(member) == member


@pytest.mark.parametrize("row", TABLE, ids=ident)
def test_a_member_is_the_only_thing_equal_to_itself(
    row: tuple[type, str, str, list[str]],
) -> None:
    vocabulary, attribute, _, _ = row
    member = getattr(vocabulary, attribute)
    others = [getattr(vocabulary, name) for name in dir(vocabulary) if name.isupper()]
    assert sum(other == member for other in others) == 1
    assert len({hash(other) for other in others}) == len(others)


@pytest.mark.parametrize("vocabulary", VOCABULARIES, ids=lambda v: v.__name__)
def test_an_unknown_spelling_names_the_vocabulary_and_lists_the_canonical(
    vocabulary: type,
) -> None:
    with pytest.raises(FastiError) as excinfo:
        vocabulary("wensleydale")
    message = str(excinfo.value)
    assert message.startswith(f"unknown {vocabulary.__name__} 'wensleydale'; expected one of ")
    for name in dir(vocabulary):
        if name.isupper():
            assert str(getattr(vocabulary, name)) in message


@pytest.mark.parametrize("vocabulary", VOCABULARIES, ids=lambda v: v.__name__)
def test_the_wrong_type_is_a_type_error(vocabulary: type) -> None:
    with pytest.raises(TypeError, match=f"expected a {vocabulary.__name__} or a str"):
        vocabulary(3)


def test_federal_is_a_spelling_of_nothing() -> None:
    # "fed" is the Federal Reserve and SIFMA convention; the US federal
    # convention is a different one. One word for both would be a trap.
    assert WeekendShift("fed") == WeekendShift.SUN_FORWARD
    assert WeekendShift("sifma") == WeekendShift.SUN_FORWARD
    with pytest.raises(FastiError, match="unknown WeekendShift 'federal'"):
        WeekendShift("federal")


def test_two_vocabularies_may_share_a_spelling_without_sharing_a_meaning() -> None:
    shift: object = WeekendShift("forward")
    assert shift != DateGenerationRule("forward")
    assert str(WeekendShift.FORWARD) == str(DateGenerationRule.FORWARD)


def test_frequency_and_weekday_carry_their_crate_accessors() -> None:
    assert Frequency.QUARTERLY.per_year() == 4
    assert Frequency.DAILY.per_year() == 365
    assert [Weekday(name).get() for name in ("mon", "sun")] == [1, 7]


@pytest.mark.parametrize("vocabulary", VOCABULARIES, ids=lambda v: v.__name__)
def test_members_are_the_whole_vocabulary(vocabulary: type) -> None:
    members = {name for name in dir(vocabulary) if name.isupper()}
    listed = {attribute for owner, attribute, _, _ in TABLE if owner is vocabulary}
    assert members == listed


def test_a_spelling_is_accepted_wherever_the_class_is() -> None:
    import datetime

    from fasti import Calendar, DayCount, Rule, Schedule
    from fasti.calendars import WEEKENDS_ONLY

    everywhere: list[Any] = [
        Calendar("Acme", ["sat"]).weekend,
        Rule.fixed(7, 4, shift="fed"),
        Rule.easter(1, method="julian"),
        Schedule(
            datetime.date(2025, 1, 15),
            datetime.date(2026, 1, 15),
            "semiannual",
            WEEKENDS_ONLY,
            convention="following",
            termination_convention="preceding",
            rule="forward",
        ),
        DayCount.act_act_icma("quarterly"),
        WEEKENDS_ONLY.adjust(datetime.date(2025, 1, 4), "following"),
    ]
    assert all(value is not None for value in everywhere)
