"""What the free-threading classifier claims, exercised.

These pass under a GIL build too — they just do not run in parallel
there. What only the free-threaded build can show is the first
assertion: that importing fasti leaves the GIL off.
"""

import datetime
import os
import pickle
import sys
import sysconfig
import threading
from typing import Any

import pytest

from fasti import DayCount, Period, Rule, Schedule
from fasti.calendars import TARGET, france, uk, us

CALENDARS = [us.SETTLEMENT, us.NYSE, uk.SETTLEMENT, france.SETTLEMENT, TARGET]
START, END = datetime.date(2015, 1, 1), datetime.date(2030, 1, 1)
THREADS = 8


def test_importing_fasti_does_not_re_enable_the_gil() -> None:
    # An extension that does not declare gil_used = false turns the GIL
    # back on for the whole process, silently, at import. fasti is
    # imported by the time this runs, so the GIL still being off is the
    # evidence — and it is what the free-threading classifier rests on.
    if not sysconfig.get_config_var("Py_GIL_DISABLED"):
        pytest.skip("not a free-threaded build")
    if os.environ.get("PYTHON_GIL") == "1":
        pytest.skip("the GIL was forced back on from the environment")
    # Reached through getattr because typeshed gates the attribute on
    # 3.13+, and mypy here runs against the abi3 floor.
    is_gil_enabled = getattr(sys, "_is_gil_enabled")  # noqa: B009
    assert not is_gil_enabled()


def test_the_whole_surface_survives_concurrent_use() -> None:
    reference = {
        calendar.name: (
            calendar.business_days(START, END),
            calendar.holidays(START, END),
        )
        for calendar in CALENDARS
    }
    failures: list[str] = []
    start_together = threading.Barrier(THREADS)

    def hammer(n: int) -> None:
        start_together.wait()
        try:
            for i in range(3):
                calendar = CALENDARS[(n + i) % len(CALENDARS)]
                assert (
                    calendar.business_days(START, END),
                    calendar.holidays(START, END),
                ) == reference[calendar.name]
                derived = (
                    calendar.union(france.SETTLEMENT)
                    .with_rule(Rule.one_off(datetime.date(2026, 8, 3)))
                    .with_name(f"worker {n}")
                )
                assert derived.is_holiday(datetime.date(2026, 8, 3))
                schedule = Schedule(
                    datetime.date(2025, 1, 15),
                    datetime.date(2035, 1, 15),
                    "semiannual",
                    calendar,
                )
                bound = DayCount.act_act_icma("semiannual").bind(schedule)
                assert bound.year_fraction(schedule[0], schedule[1]) > 0
                assert pickle.loads(pickle.dumps(derived)) == derived
                assert pickle.loads(pickle.dumps(schedule)) == schedule
                assert hash(calendar) == hash(pickle.loads(pickle.dumps(calendar)))
                assert datetime.date(2026, 1, 15) + Period.months(6) == (
                    datetime.date(2026, 7, 15)
                )
        except Exception as error:  # noqa: BLE001 - reported, not swallowed
            failures.append(f"thread {n}: {type(error).__name__}: {error}")

    workers = [threading.Thread(target=hammer, args=(n,)) for n in range(THREADS)]
    for worker in workers:
        worker.start()
    for worker in workers:
        worker.join()
    assert failures == []


def test_the_cached_boundary_types_initialise_once_under_contention() -> None:
    # date_out caches datetime.date.fromordinal in a PyOnceLock. Racing
    # threads through a cold-ish cache is what would surface a bad init.
    results: list[Any] = []
    lock = threading.Lock()
    start_together = threading.Barrier(THREADS)

    def convert() -> None:
        start_together.wait()
        dates = us.SETTLEMENT.business_days(
            datetime.date(2024, 1, 1), datetime.date(2024, 2, 1)
        )
        with lock:
            results.append(dates)

    workers = [threading.Thread(target=convert) for _ in range(THREADS)]
    for worker in workers:
        worker.start()
    for worker in workers:
        worker.join()
    assert len(results) == THREADS
    assert all(run == results[0] for run in results)
    assert all(type(day) is datetime.date for day in results[0])
