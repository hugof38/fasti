"""Type stubs for the compiled core.

The public names are re-exported from ``fasti``; import them from there.
"""

import datetime
from fractions import Fraction
from typing import Iterator, Sequence, TypeAlias, final, overload

__version__: str

#: The earliest date fasti can represent.
MIN_DATE: datetime.date
#: The latest date fasti can represent.
MAX_DATE: datetime.date

#: Anything accepted where a date is wanted: a ``datetime.date``, a
#: ``datetime.datetime`` (the time is dropped), or an ISO ``YYYY-MM-DD``
#: string.
DateLike: TypeAlias = datetime.date | str
#: A period, a period string such as ``"6M"``, a frequency, or a
#: whole-day ``timedelta``.
PeriodLike: TypeAlias = "Period | Frequency | str | datetime.timedelta"
#: A business-day convention or its name.
ConventionLike: TypeAlias = "BusinessDayConvention | str"
#: A generation rule or its name.
GenerationLike: TypeAlias = "DateGenerationRule | str"
#: A frequency, its name, or a period that names one.
FrequencyLike: TypeAlias = "Frequency | Period | str"
#: A weekday, its name, or its ISO number (Monday is 1).
WeekdayLike: TypeAlias = "Weekday | str | int"
#: A month number (1–12) or name.
MonthLike: TypeAlias = str | int
#: A weekend name such as ``"sat_sun"``, or an iterable of weekdays.
WeekendLike: TypeAlias = "str | Sequence[WeekdayLike]"
#: A weekend-shift policy or its name.
ShiftLike: TypeAlias = "WeekendShift | str"
#: A day-count convention or its name.
DayCountLike: TypeAlias = "DayCount | str"

class FastiError(ValueError):
    """Raised for values fasti cannot represent or names it cannot resolve."""

@final
class Weekday:
    """A day of the week, numbered as ``date.isoweekday()``."""

    MON: Weekday
    TUE: Weekday
    WED: Weekday
    THU: Weekday
    FRI: Weekday
    SAT: Weekday
    SUN: Weekday
    @property
    def isoweekday(self) -> int: ...
    @property
    def weekday(self) -> int: ...
    @staticmethod
    def parse(value: WeekdayLike) -> Weekday: ...
    def __int__(self) -> int: ...

@final
class BusinessDayConvention:
    """What to do when an adjusted date is not a business day."""

    FOLLOWING: BusinessDayConvention
    MODIFIED_FOLLOWING: BusinessDayConvention
    PRECEDING: BusinessDayConvention
    MODIFIED_PRECEDING: BusinessDayConvention
    UNADJUSTED: BusinessDayConvention
    @staticmethod
    def parse(value: ConventionLike) -> BusinessDayConvention: ...

@final
class DateGenerationRule:
    """Which end of a schedule the regular grid is anchored to."""

    FORWARD: DateGenerationRule
    BACKWARD: DateGenerationRule
    ZERO: DateGenerationRule
    @staticmethod
    def parse(value: GenerationLike) -> DateGenerationRule: ...

@final
class Frequency:
    """How many times a year a payment recurs."""

    ANNUAL: Frequency
    SEMIANNUAL: Frequency
    EVERY_FOURTH_MONTH: Frequency
    QUARTERLY: Frequency
    BIMONTHLY: Frequency
    MONTHLY: Frequency
    EVERY_FOURTH_WEEK: Frequency
    BIWEEKLY: Frequency
    WEEKLY: Frequency
    DAILY: Frequency
    @property
    def per_year(self) -> int: ...
    @property
    def period(self) -> Period: ...
    @staticmethod
    def parse(value: FrequencyLike) -> Frequency: ...

@final
class WeekendShift:
    """Which way a fixed-date holiday moves off a weekend."""

    NONE: WeekendShift
    FORWARD: WeekendShift
    SUN_FORWARD: WeekendShift
    SAT_BACK_SUN_FORWARD: WeekendShift
    @staticmethod
    def parse(value: ShiftLike) -> WeekendShift: ...

@final
class Period:
    """A signed duration tagged by its calendar unit."""

    def __init__(
        self,
        spec: PeriodLike | None = None,
        *,
        days: int | None = None,
        weeks: int | None = None,
        months: int | None = None,
        years: int | None = None,
    ) -> None: ...
    @staticmethod
    def parse(text: str) -> Period: ...
    @staticmethod
    def days(n: int) -> Period: ...
    @staticmethod
    def weeks(n: int) -> Period: ...
    @staticmethod
    def months(n: int) -> Period: ...
    @staticmethod
    def years(n: int) -> Period: ...
    @property
    def length(self) -> int: ...
    @property
    def unit(self) -> str: ...
    @property
    def is_zero(self) -> bool: ...
    @property
    def frequency(self) -> Frequency | None: ...
    def normalized(self) -> Period: ...
    def __neg__(self) -> Period: ...
    def __mul__(self, n: int) -> Period: ...
    def __rmul__(self, n: int) -> Period: ...

@final
class Rule:
    """A holiday rule, naming a holiday's natural date."""

    @staticmethod
    def fixed(
        month: MonthLike,
        day: int,
        *,
        shift: ShiftLike | None = None,
        from_year: int | None = None,
        to_year: int | None = None,
    ) -> Rule: ...
    @staticmethod
    def nth_weekday(
        n: int,
        weekday: WeekdayLike,
        month: MonthLike,
        *,
        from_year: int | None = None,
        to_year: int | None = None,
    ) -> Rule: ...
    @staticmethod
    def last_weekday(
        weekday: WeekdayLike,
        month: MonthLike,
        *,
        from_year: int | None = None,
        to_year: int | None = None,
    ) -> Rule: ...
    @staticmethod
    def easter(
        offset: int,
        *,
        method: str | None = None,
        from_year: int | None = None,
        to_year: int | None = None,
    ) -> Rule: ...
    @staticmethod
    def good_friday(*, method: str | None = None) -> Rule: ...
    @staticmethod
    def easter_monday(*, method: str | None = None) -> Rule: ...
    @staticmethod
    def ascension(*, method: str | None = None) -> Rule: ...
    @staticmethod
    def whit_monday(*, method: str | None = None) -> Rule: ...
    @staticmethod
    def corpus_christi(*, method: str | None = None) -> Rule: ...
    @staticmethod
    def one_off(date: DateLike) -> Rule: ...
    def is_holiday(self, date: DateLike) -> bool: ...

@final
class Calendar:
    """A weekend plus a set of holiday rules."""

    def __init__(self, name: str) -> None: ...
    @classmethod
    def load(cls, name: str) -> Calendar: ...
    @staticmethod
    def names() -> list[str]: ...
    @staticmethod
    def custom(
        name: str,
        *,
        weekend: WeekendLike | None = None,
        rules: Sequence[Rule] | None = None,
        holidays: Sequence[DateLike] | None = None,
    ) -> Calendar: ...
    @property
    def name(self) -> str: ...
    @property
    def weekend(self) -> list[Weekday]: ...
    def is_weekend(self, date: DateLike) -> bool: ...
    def is_holiday(self, date: DateLike) -> bool: ...
    def is_business_day(self, date: DateLike) -> bool: ...
    def business_days(self, start: DateLike, end: DateLike) -> list[datetime.date]: ...
    def count_business_days(self, start: DateLike, end: DateLike) -> int: ...
    def holidays(self, start: DateLike, end: DateLike) -> list[datetime.date]: ...
    def next_business_day(self, date: DateLike) -> datetime.date | None: ...
    def prev_business_day(self, date: DateLike) -> datetime.date | None: ...
    def first_business_day_of_month(self, date: DateLike) -> datetime.date | None: ...
    def last_business_day_of_month(self, date: DateLike) -> datetime.date | None: ...
    def adjust(
        self, date: DateLike, convention: ConventionLike | None = None
    ) -> datetime.date: ...
    def advance(
        self,
        date: DateLike,
        period: PeriodLike,
        convention: ConventionLike | None = None,
        *,
        end_of_month: bool = False,
    ) -> datetime.date: ...
    def union(self, other: Calendar) -> Calendar: ...
    def with_holidays(self, holidays: Sequence[DateLike]) -> Calendar: ...
    def with_rules(self, rules: Sequence[Rule]) -> Calendar: ...
    def with_weekend(self, weekend: WeekendLike) -> Calendar: ...
    def renamed(self, name: str) -> Calendar: ...

@final
class Schedule:
    """A generated coupon-date grid."""

    def __init__(
        self,
        effective: DateLike,
        termination: DateLike,
        tenor: PeriodLike,
        calendar: Calendar | None = None,
        *,
        convention: ConventionLike | None = None,
        termination_convention: ConventionLike | None = None,
        rule: GenerationLike | None = None,
        end_of_month: bool = False,
        first_date: DateLike | None = None,
        next_to_last_date: DateLike | None = None,
    ) -> None: ...
    @classmethod
    def from_dates(cls, dates: Sequence[DateLike]) -> Schedule: ...
    @property
    def dates(self) -> list[datetime.date]: ...
    @property
    def tenor(self) -> Period | None: ...
    @property
    def end_of_month(self) -> bool | None: ...
    def periods(self) -> list[tuple[datetime.date, datetime.date]]: ...
    def reference_periods(self) -> list[tuple[datetime.date, datetime.date]]: ...
    def previous_date(self, date: DateLike) -> datetime.date | None: ...
    def next_date(self, date: DateLike) -> datetime.date | None: ...
    def lower_bound(self, date: DateLike) -> datetime.date | None: ...
    def after(self, cutoff: DateLike) -> Schedule: ...
    def until(self, cutoff: DateLike) -> Schedule: ...
    def __len__(self) -> int: ...
    @overload
    def __getitem__(self, index: int) -> datetime.date: ...
    @overload
    def __getitem__(self, index: slice) -> list[datetime.date]: ...
    def __iter__(self) -> Iterator[datetime.date]: ...

@final
class DayCount:
    """A day-count convention, measuring time as an exact fraction."""

    def __init__(
        self,
        name: str,
        *,
        frequency: FrequencyLike | None = None,
        schedule: Schedule | None = None,
        termination: DateLike | None = None,
    ) -> None: ...
    @property
    def name(self) -> str: ...
    @property
    def frequency(self) -> Frequency | None: ...
    def day_count(self, start: DateLike, end: DateLike) -> int: ...
    def year_fraction(self, start: DateLike, end: DateLike) -> Fraction: ...
    def bind(self, schedule: Schedule) -> DayCount: ...

def year_fraction(
    start: DateLike,
    end: DateLike,
    convention: DayCountLike,
    *,
    frequency: FrequencyLike | None = None,
    schedule: Schedule | None = None,
    termination: DateLike | None = None,
) -> Fraction: ...
def day_count(
    start: DateLike,
    end: DateLike,
    convention: DayCountLike,
    *,
    frequency: FrequencyLike | None = None,
    schedule: Schedule | None = None,
    termination: DateLike | None = None,
) -> int: ...
def easter_sunday(year: int, *, method: str | None = None) -> datetime.date: ...
def easter_monday(year: int, *, method: str | None = None) -> datetime.date: ...
