//! A fixed reference instant for computing the time-dependent document
//! attributes.

/// A fixed reference instant used to compute the time-dependent document
/// attributes (`docdate`, `doctime`, `docdatetime`, `docyear`, and their
/// `local*` siblings).
///
/// AsciiDoc derives these attributes from a reference time: normally the
/// modification time of the source file (for `doc*`) and the current wall-clock
/// time (for `local*`). Because those values change from run to run, output
/// that embeds them is not reproducible. Supplying a `ReferenceTime` pins the
/// clock to a known instant so that the computed attributes — and any output
/// derived from them — are stable and reproducible.
///
/// A `ReferenceTime` records a *local* wall-clock date and time together with
/// the UTC offset in effect at that instant. The offset governs how the
/// computed `doctime` / `localtime` prints its zone: `UTC` when the offset is
/// zero, otherwise a numeric `±HHMM` suffix (matching Asciidoctor's use of
/// `%Z`/`%z`).
///
/// Install one with [`Parser::with_reference_time`] (pins the whole clock) or
/// [`Parser::with_input_mtime`] (pins only the source-file time that drives the
/// `doc*` attributes). See also the `SOURCE_DATE_EPOCH` environment variable,
/// which pins the clock when no `ReferenceTime` is supplied.
///
/// [`Parser::with_reference_time`]: crate::Parser::with_reference_time
/// [`Parser::with_input_mtime`]: crate::Parser::with_input_mtime
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceTime {
    /// Year (e.g. `2019`).
    year: i64,

    /// Month of year, `1..=12`.
    month: u32,

    /// Day of month, `1..=31`.
    day: u32,

    /// Hour of day, `0..=23`.
    hour: u32,

    /// Minute of hour, `0..=59`.
    minute: u32,

    /// Second of minute, `0..=60` (a leap second is permitted).
    second: u32,

    /// UTC offset, in seconds, of the local time recorded above. Zero prints as
    /// the zone label `UTC`; any other value prints as `±HHMM`.
    utc_offset_secs: i32,
}

impl ReferenceTime {
    /// Creates a reference time from a count of seconds since the Unix epoch
    /// (1970-01-01T00:00:00Z), interpreted as UTC.
    ///
    /// This is the shape of the `SOURCE_DATE_EPOCH` value defined by the
    /// [reproducible builds specification]. The resulting `doctime` /
    /// `localtime` prints its zone as `UTC`.
    ///
    /// [reproducible builds specification]: https://reproducible-builds.org/specs/source-date-epoch/
    pub fn from_unix_timestamp(secs: i64) -> Self {
        let days = secs.div_euclid(86_400);
        let rem = secs.rem_euclid(86_400);
        let (year, month, day) = civil_from_days(days);

        Self {
            year,
            month,
            day,
            hour: (rem / 3600) as u32,
            minute: ((rem % 3600) / 60) as u32,
            second: (rem % 60) as u32,
            utc_offset_secs: 0,
        }
    }

    /// Creates a reference time from an explicit *local* wall-clock date and
    /// time and the UTC offset (in seconds) in effect at that instant.
    ///
    /// This mirrors constructing a timezone-aware time such as Asciidoctor's
    /// `input_mtime`. The offset controls the printed zone of the computed
    /// `doctime` / `localtime`: `UTC` when `utc_offset_secs` is `0`, otherwise
    /// a numeric `±HHMM` suffix (e.g. a `+06:00` offset prints as `+0600`).
    ///
    /// The components are stored as given and are not validated or normalized;
    /// supplying out-of-range values yields correspondingly out-of-range
    /// output.
    pub fn from_local(
        year: i64,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
        utc_offset_secs: i32,
    ) -> Self {
        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
            utc_offset_secs,
        }
    }

    /// Returns the reference time for the real wall clock, read as UTC.
    ///
    /// This crate carries no timezone database, so an un-pinned clock is read
    /// as UTC rather than local time. For reproducible, timezone-correct
    /// output, pin the clock with [`Parser::with_reference_time`] /
    /// [`Parser::with_input_mtime`] or set the `SOURCE_DATE_EPOCH` environment
    /// variable.
    ///
    /// [`Parser::with_reference_time`]: crate::Parser::with_reference_time
    /// [`Parser::with_input_mtime`]: crate::Parser::with_input_mtime
    pub(crate) fn now() -> Self {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        Self::from_unix_timestamp(secs)
    }

    /// Formats the date as `%Y-%m-%d` (e.g. `2019-01-02`) — the value of
    /// `docdate` / `localdate`.
    pub(crate) fn date(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    /// Formats the time as `%H:%M:%S <zone>` (e.g. `03:04:05 +0600` or
    /// `03:04:05 UTC`) — the value of `doctime` / `localtime`.
    pub(crate) fn time(&self) -> String {
        format!(
            "{:02}:{:02}:{:02} {}",
            self.hour,
            self.minute,
            self.second,
            self.zone_label()
        )
    }

    /// Formats the year (e.g. `2019`) — the value of `docyear` / `localyear`.
    pub(crate) fn year_string(&self) -> String {
        self.year.to_string()
    }

    /// Formats the zone as Asciidoctor does: `UTC` for a zero UTC offset,
    /// otherwise a numeric `±HHMM` suffix.
    fn zone_label(&self) -> String {
        if self.utc_offset_secs == 0 {
            "UTC".to_string()
        } else {
            let sign = if self.utc_offset_secs < 0 { '-' } else { '+' };
            let abs = self.utc_offset_secs.unsigned_abs();
            let hours = abs / 3600;
            let minutes = (abs % 3600) / 60;
            format!("{sign}{hours:02}{minutes:02}")
        }
    }
}

/// Converts a count of days since the Unix epoch to a `(year, month, day)`
/// civil date, using Howard Hinnant's `civil_from_days` algorithm (valid for
/// the full range of a proleptic Gregorian calendar).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    // Shift the epoch to 0000-03-01 so that leap days fall at the end of each
    // 400-year era.
    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = z - era * 146_097; // day of era, [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // month, shifted so March is 0: [0, 11]
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let month = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32; // [1, 12]

    (if month <= 2 { y + 1 } else { y }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_unix_timestamp_epoch() {
        let t = ReferenceTime::from_unix_timestamp(0);
        assert_eq!(t.date(), "1970-01-01");
        assert_eq!(t.time(), "00:00:00 UTC");
        assert_eq!(t.year_string(), "1970");
    }

    #[test]
    fn from_unix_timestamp_known_instant() {
        // 2015-01-01T10:00:00Z == 1420106400.
        let t = ReferenceTime::from_unix_timestamp(1_420_106_400);
        assert_eq!(t.date(), "2015-01-01");
        assert_eq!(t.time(), "10:00:00 UTC");
        assert_eq!(t.year_string(), "2015");
    }

    #[test]
    fn from_unix_timestamp_leap_day() {
        // 2016-02-29T23:59:59Z == 1456790399.
        let t = ReferenceTime::from_unix_timestamp(1_456_790_399);
        assert_eq!(t.date(), "2016-02-29");
        assert_eq!(t.time(), "23:59:59 UTC");
    }

    #[test]
    fn from_unix_timestamp_before_epoch() {
        // 1969-12-31T23:59:59Z == -1.
        let t = ReferenceTime::from_unix_timestamp(-1);
        assert_eq!(t.date(), "1969-12-31");
        assert_eq!(t.time(), "23:59:59 UTC");
    }

    #[test]
    fn from_local_positive_offset() {
        // Time.new(2019, 1, 2, 3, 4, 5, "+06:00").
        let t = ReferenceTime::from_local(2019, 1, 2, 3, 4, 5, 6 * 3600);
        assert_eq!(t.date(), "2019-01-02");
        assert_eq!(t.time(), "03:04:05 +0600");
        assert_eq!(t.year_string(), "2019");
    }

    #[test]
    fn from_local_negative_offset() {
        let t = ReferenceTime::from_local(2019, 1, 2, 3, 4, 5, -(7 * 3600));
        assert_eq!(t.time(), "03:04:05 -0700");
    }

    #[test]
    fn from_local_half_hour_offset() {
        let t = ReferenceTime::from_local(2019, 1, 2, 3, 4, 5, 5 * 3600 + 30 * 60);
        assert_eq!(t.time(), "03:04:05 +0530");
    }

    #[test]
    fn from_local_zero_offset_prints_utc() {
        let t = ReferenceTime::from_local(2019, 1, 2, 3, 4, 5, 0);
        assert_eq!(t.time(), "03:04:05 UTC");
    }
}
