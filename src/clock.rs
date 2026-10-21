//! World time and the calendar. The clock only runs while the simulation
//! does, so it stops when the game is paused.

use std::fmt;

use bevy::prelude::*;

use crate::schedule::SimSet;

/// Seconds in an hour.
pub const HOUR: f64 = 3_600.0;
/// Seconds in a day.
pub const DAY: f64 = 24.0 * HOUR;

/// The game opens on this date, in the summer of the Republic of Pirates.
const START_YEAR: u32 = 1716;
const START_MONTH: u32 = 6;
const START_DAY: u32 = 1;
const START_HOUR: f64 = 8.0;

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Time elapsed in the world since the game began.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
pub struct WorldClock {
    /// Seconds since the start of the game.
    pub elapsed: f64,
}

impl WorldClock {
    pub fn date(&self) -> GameDate {
        GameDate::at(self.elapsed)
    }

    /// How many new days began in the last `step` seconds: one every
    /// [`DAY`] since the game opened, at eight in the morning.
    pub fn days_begun(&self, step: f64) -> u32 {
        self.periods_begun(step, DAY)
    }

    /// How many new periods of `period` seconds, counted from the game's
    /// opening, began in the last `step` seconds.
    pub fn periods_begun(&self, step: f64, period: f64) -> u32 {
        let periods = |at: f64| (at / period).floor().max(0.0) as u32;
        periods(self.elapsed) - periods(self.elapsed - step)
    }
}

/// A calendar date and time of day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameDate {
    pub year: u32,
    /// 1 to 12.
    pub month: u32,
    /// 1 to 31.
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
}

impl GameDate {
    /// The date `elapsed` seconds after the game began.
    pub fn at(elapsed: f64) -> Self {
        let since_midnight = elapsed + START_HOUR * HOUR;
        let mut days = (since_midnight / DAY).floor() as u32;
        let seconds_today = since_midnight.rem_euclid(DAY);
        let (mut year, mut month, mut day) = (START_YEAR, START_MONTH, START_DAY);
        // Walk forward a month at a time; games span years, not millennia.
        loop {
            let left_in_month = days_in_month(year, month) - day + 1;
            if days < left_in_month {
                day += days;
                break;
            }
            days -= left_in_month;
            day = 1;
            month += 1;
            if month > 12 {
                month = 1;
                year += 1;
            }
        }
        Self {
            year,
            month,
            day,
            hour: (seconds_today / HOUR) as u32,
            minute: (seconds_today % HOUR / 60.0) as u32,
        }
    }
}

impl GameDate {
    /// The day without the time: "1 June 1716".
    pub fn day_name(&self) -> String {
        format!(
            "{} {} {}",
            self.day,
            MONTHS[self.month as usize - 1],
            self.year
        )
    }
}

impl fmt::Display for GameDate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}, {:02}:{:02}",
            self.day_name(),
            self.hour,
            self.minute
        )
    }
}

fn days_in_month(year: u32, month: u32) -> u32 {
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

pub struct ClockPlugin;

impl Plugin for ClockPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldClock>()
            .add_systems(FixedUpdate, advance_clock.in_set(SimSet::Navigation));
    }
}

fn advance_clock(time: Res<Time>, mut clock: ResMut<WorldClock>) {
    clock.elapsed += time.delta_secs_f64();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{run_seconds, sim_app};

    fn date(year: u32, month: u32, day: u32, hour: u32, minute: u32) -> GameDate {
        GameDate {
            year,
            month,
            day,
            hour,
            minute,
        }
    }

    #[test]
    fn the_game_opens_on_the_morning_of_the_first_of_june_1716() {
        assert_eq!(GameDate::at(0.0), date(1716, 6, 1, 8, 0));
        assert_eq!(GameDate::at(0.0).to_string(), "1 June 1716, 08:00");
        assert_eq!(GameDate::at(0.0).day_name(), "1 June 1716");
        let clock = WorldClock {
            elapsed: 2.0 * DAY + 10.0,
        };
        assert_eq!(clock.days_begun(20.0), 1);
        assert_eq!(clock.days_begun(5.0), 0);
        assert_eq!(clock.days_begun(3.0 * DAY), 2, "none before the game");
        assert_eq!(clock.periods_begun(20.0, HOUR), 1);
        assert_eq!(clock.periods_begun(2.0 * HOUR, HOUR), 2);
    }

    #[test]
    fn hours_roll_over_into_the_next_day() {
        assert_eq!(GameDate::at(16.0 * HOUR), date(1716, 6, 2, 0, 0));
        assert_eq!(GameDate::at(90.0 * 60.0), date(1716, 6, 1, 9, 30));
    }

    #[test]
    fn months_and_years_roll_over() {
        // June has 30 days.
        assert_eq!(GameDate::at(30.0 * DAY), date(1716, 7, 1, 8, 0));
        // June through December 1716 is 214 days.
        assert_eq!(GameDate::at(214.0 * DAY), date(1717, 1, 1, 8, 0));
    }

    #[test]
    fn leap_years_follow_the_gregorian_rule() {
        assert_eq!(days_in_month(1716, 2), 29);
        assert_eq!(days_in_month(1717, 2), 28);
        assert_eq!(days_in_month(1700, 2), 28);
        assert_eq!(days_in_month(1600, 2), 29);
    }

    #[test]
    fn clock_advances_with_the_simulation_and_stops_when_paused() {
        let mut app = sim_app();
        run_seconds(&mut app, 10.0);
        let elapsed = app.world().resource::<WorldClock>().elapsed;
        assert!((elapsed - 10.0).abs() < 0.1, "{elapsed}");

        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        run_seconds(&mut app, 10.0);
        assert_eq!(app.world().resource::<WorldClock>().elapsed, elapsed);
    }
}
