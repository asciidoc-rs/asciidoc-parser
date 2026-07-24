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
/// clock to a known instant so that the computed attributes – and any output
/// derived from them – are stable and reproducible.
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
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
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

    /// Formats the date as `%Y-%m-%d` (e.g. `2019-01-02`) – the value of
    /// `docdate` / `localdate`.
    pub(crate) fn date(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    /// Formats the time as `%H:%M:%S <zone>` (e.g. `03:04:05 +0600` or
    /// `03:04:05 UTC`) – the value of `doctime` / `localtime`.
    pub(crate) fn time(&self) -> String {
        format!(
            "{:02}:{:02}:{:02} {}",
            self.hour,
            self.minute,
            self.second,
            self.zone_label()
        )
    }

    /// Formats the year (e.g. `2019`) – the value of `docyear` / `localyear`.
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

/// The eight time-dependent document attributes computed by
/// [`DatetimeContext`], in a fixed order (the `local*` family followed by the
/// `doc*` family).
pub(crate) const DATETIME_ATTRIBUTE_NAMES: [&str; 8] = [
    "localdate",
    "localtime",
    "localdatetime",
    "localyear",
    "docdate",
    "doctime",
    "docdatetime",
    "docyear",
];

/// Returns `true` if `name` is one of the time-dependent document attributes
/// resolved by [`DatetimeContext::resolve`].
pub(crate) fn is_datetime_attribute(name: &str) -> bool {
    DATETIME_ATTRIBUTE_NAMES.contains(&name)
}

/// The parser's pinned reference-time configuration: an optional reference time
/// (the value of "now") and an optional source modification time.
///
/// This is the deterministic *input* to the time-dependent attributes, retained
/// on a [`ResolvedAttributes`](crate::parser::ResolvedAttributes) snapshot so
/// it can resolve them on demand. Both fields are commonly `None` (no clock
/// pinned), so a snapshot stores this behind a single `Option<Box<…>>` that
/// adds only a pointer and allocates nothing in the common case.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct DatetimeInputs {
    /// The pinned reference time (`Parser::with_reference_time`), if any.
    pub(crate) reference_time: Option<ReferenceTime>,

    /// The pinned source modification time (`Parser::with_input_mtime`), if
    /// any.
    pub(crate) input_mtime: Option<ReferenceTime>,
}

impl DatetimeInputs {
    /// Builds the inputs from the two optional pinned times, returning `None`
    /// when neither is set (so the snapshot stores nothing).
    pub(crate) fn new(
        reference_time: Option<ReferenceTime>,
        input_mtime: Option<ReferenceTime>,
    ) -> Option<Box<Self>> {
        if reference_time.is_none() && input_mtime.is_none() {
            None
        } else {
            Some(Box::new(Self {
                reference_time,
                input_mtime,
            }))
        }
    }

    /// Captures the reference instants from these inputs (see
    /// [`DatetimeContext::capture`]).
    pub(crate) fn capture(&self) -> DatetimeContext {
        DatetimeContext::capture(self.reference_time.as_ref(), self.input_mtime.as_ref())
    }
}

/// The reference instants from which the time-dependent document attributes are
/// computed: `now` drives the `local*` family, and `doc` (the source document's
/// modification time, defaulting to `now`) drives the `doc*` family.
///
/// This is captured once – lazily, the first time a time-dependent attribute is
/// read – so that a parse which never references one does no clock,
/// environment, or allocation work, and so that repeated reads within one parse
/// observe a single, consistent instant. It ports Asciidoctor's
/// `Document#fill_datetime_attributes`, but resolves each attribute on demand
/// rather than materializing all eight up front.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DatetimeContext {
    /// Drives the `local*` attributes.
    now: ReferenceTime,

    /// Drives the `doc*` attributes.
    doc: ReferenceTime,
}

impl DatetimeContext {
    /// Captures the reference instants from the parser's configuration.
    ///
    /// `now` resolves to, in order: the pinned `reference_time`
    /// ([`Parser::with_reference_time`]), the `SOURCE_DATE_EPOCH` environment
    /// variable, or the real wall clock (read as UTC). The `doc` instant
    /// resolves to the pinned `input_mtime` ([`Parser::with_input_mtime`]) or,
    /// absent that, to `now`.
    ///
    /// [`Parser::with_reference_time`]: crate::Parser::with_reference_time
    /// [`Parser::with_input_mtime`]: crate::Parser::with_input_mtime
    pub(crate) fn capture(
        reference_time: Option<&ReferenceTime>,
        input_mtime: Option<&ReferenceTime>,
    ) -> Self {
        let now = reference_time
            .cloned()
            .or_else(source_date_epoch_from_env)
            .unwrap_or_else(ReferenceTime::now);

        let doc = input_mtime.cloned().unwrap_or_else(|| now.clone());

        Self { now, doc }
    }

    /// Resolves the time-dependent attribute `name` to its computed value, or
    /// `None` when `name` is not a time-dependent attribute (or resolves to no
    /// value, as `docyear` / `localyear` do for an explicit date without a
    /// `YYYY-` prefix).
    ///
    /// `explicit` returns the *explicitly-set* value of a sibling attribute (an
    /// API/header/body assignment), treating a value-less "set" as an empty
    /// string and an unset/absent attribute as `None`. An explicit value always
    /// wins over the computed one, and the derived `*year` / `*datetime` are
    /// built from whichever value (explicit or computed) each sibling resolves
    /// to – mirroring Asciidoctor's `||=` semantics.
    pub(crate) fn resolve<F>(&self, name: &str, explicit: F) -> Option<String>
    where
        F: Fn(&str) -> Option<String>,
    {
        match name {
            "localdate" => Some(self.date("localdate", &self.now, &explicit)),
            "localtime" => Some(self.time("localtime", &self.now, &explicit)),
            "localyear" => self.year("localyear", "localdate", &self.now, &explicit),
            "localdatetime" => Some(self.datetime(
                "localdatetime",
                "localdate",
                "localtime",
                &self.now,
                &explicit,
            )),
            "docdate" => Some(self.date("docdate", &self.doc, &explicit)),
            "doctime" => Some(self.time("doctime", &self.doc, &explicit)),
            "docyear" => self.year("docyear", "docdate", &self.doc, &explicit),
            "docdatetime" => {
                Some(self.datetime("docdatetime", "docdate", "doctime", &self.doc, &explicit))
            }
            _ => None,
        }
    }

    /// Resolves a date attribute (`localdate` / `docdate`): an explicit value,
    /// else the reference instant's date.
    fn date<F: Fn(&str) -> Option<String>>(
        &self,
        attr: &str,
        instant: &ReferenceTime,
        explicit: &F,
    ) -> String {
        explicit(attr).unwrap_or_else(|| instant.date())
    }

    /// Resolves a time attribute (`localtime` / `doctime`): an explicit value,
    /// else the reference instant's time.
    fn time<F: Fn(&str) -> Option<String>>(
        &self,
        attr: &str,
        instant: &ReferenceTime,
        explicit: &F,
    ) -> String {
        explicit(attr).unwrap_or_else(|| instant.time())
    }

    /// Resolves a year attribute (`localyear` / `docyear`): an explicit value,
    /// else derived from the date – the reference instant's year when the date
    /// is computed, or the `YYYY-` prefix of an explicit date (which may yield
    /// no value).
    fn year<F: Fn(&str) -> Option<String>>(
        &self,
        year_attr: &str,
        date_attr: &str,
        instant: &ReferenceTime,
        explicit: &F,
    ) -> Option<String> {
        if let Some(year) = explicit(year_attr) {
            return Some(year);
        }
        match explicit(date_attr) {
            Some(date) => year_from_date(&date),
            None => Some(instant.year_string()),
        }
    }

    /// Resolves a datetime attribute (`localdatetime` / `docdatetime`): an
    /// explicit value, else `"{date} {time}"` built from the resolved date and
    /// time siblings.
    fn datetime<F: Fn(&str) -> Option<String>>(
        &self,
        datetime_attr: &str,
        date_attr: &str,
        time_attr: &str,
        instant: &ReferenceTime,
        explicit: &F,
    ) -> String {
        explicit(datetime_attr).unwrap_or_else(|| {
            let date = self.date(date_attr, instant, explicit);
            let time = self.time(time_attr, instant, explicit);
            format!("{date} {time}")
        })
    }
}

/// Reads the `SOURCE_DATE_EPOCH` environment variable as a fixed reference
/// time, if it is set to a non-empty, valid integer count of seconds since the
/// Unix epoch (interpreted as UTC), per the [reproducible builds
/// specification].
///
/// Returns `None` when the variable is unset, empty, or malformed. Asciidoctor
/// raises on a malformed value; this crate has no error channel at this point,
/// so a malformed value is ignored and the clock falls back to the next source
/// (a pinned reference time, or the real wall clock) rather than aborting the
/// parse.
///
/// [reproducible builds specification]: https://reproducible-builds.org/specs/source-date-epoch/
fn source_date_epoch_from_env() -> Option<ReferenceTime> {
    // An unset variable reads as empty, which `parse_source_date_epoch` rejects
    // just like a set-but-empty value – so the (common) unset path exercises the
    // same line as a set one, without the tests having to mutate the process
    // environment.
    parse_source_date_epoch(&std::env::var("SOURCE_DATE_EPOCH").unwrap_or_default())
}

/// Parses a `SOURCE_DATE_EPOCH` string into a reference time, returning `None`
/// when it is empty or not a valid integer. Split out from
/// [`source_date_epoch_from_env`] so the parsing rules can be tested without
/// touching the process environment.
fn parse_source_date_epoch(raw: &str) -> Option<ReferenceTime> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let secs = trimmed.parse::<i64>().ok()?;
    Some(ReferenceTime::from_unix_timestamp(secs))
}

/// Derives the year from a `docdate` / `localdate` value, mirroring
/// Asciidoctor's `(date.index '-') == 4 ? (date.slice 0, 4) : nil`.
///
/// Returns the four-character prefix only when the first `-` is at index 4 (a
/// `YYYY-` prefix); any other shape yields `None`, leaving the year attribute
/// unset.
fn year_from_date(date: &str) -> Option<String> {
    if date.find('-') == Some(4) {
        date.get(..4).map(str::to_string)
    } else {
        None
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
    #![allow(clippy::expect_used)]

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

    #[test]
    fn parse_source_date_epoch_valid() {
        let t = parse_source_date_epoch("1420106400").expect("valid epoch");
        assert_eq!(t.date(), "2015-01-01");
        assert_eq!(t.time(), "10:00:00 UTC");
    }

    #[test]
    fn parse_source_date_epoch_trims_whitespace() {
        assert!(parse_source_date_epoch("  1420106400  ").is_some());
    }

    #[test]
    fn parse_source_date_epoch_rejects_empty_and_malformed() {
        assert!(parse_source_date_epoch("").is_none());
        assert!(parse_source_date_epoch("   ").is_none());
        assert!(parse_source_date_epoch("not-a-number").is_none());
        assert!(parse_source_date_epoch("12.5").is_none());
    }

    #[test]
    fn year_from_date_extracts_yyyy_prefix() {
        assert_eq!(year_from_date("2015-01-01").as_deref(), Some("2015"));
        assert_eq!(year_from_date("2015-1-1").as_deref(), Some("2015"));
    }

    #[test]
    fn year_from_date_rejects_other_shapes() {
        assert_eq!(year_from_date("2015"), None); // no `-`
        assert_eq!(year_from_date("20-15-01"), None); // first `-` not at index 4
        assert_eq!(year_from_date("January 1, 2015"), None);
        assert_eq!(year_from_date(""), None);
    }

    #[test]
    fn is_datetime_attribute_recognizes_the_family() {
        for name in DATETIME_ATTRIBUTE_NAMES {
            assert!(is_datetime_attribute(name), "{name} should be recognized");
        }
        assert!(!is_datetime_attribute("docfile"));
        assert!(!is_datetime_attribute("frog"));
    }

    /// A resolver with no explicit overrides, driven by a fixed instant.
    fn ctx() -> DatetimeContext {
        // now: 2015-01-01T10:00:00Z; doc: 2019-01-02 03:04:05 +0600.
        DatetimeContext {
            now: ReferenceTime::from_unix_timestamp(1_420_106_400),
            doc: ReferenceTime::from_local(2019, 1, 2, 3, 4, 5, 6 * 3600),
        }
    }

    #[test]
    fn resolve_computes_the_family_from_instants() {
        let ctx = ctx();
        let none = |_: &str| None;

        assert_eq!(
            ctx.resolve("localdate", none).as_deref(),
            Some("2015-01-01")
        );
        assert_eq!(
            ctx.resolve("localtime", none).as_deref(),
            Some("10:00:00 UTC")
        );
        assert_eq!(ctx.resolve("localyear", none).as_deref(), Some("2015"));
        assert_eq!(
            ctx.resolve("localdatetime", none).as_deref(),
            Some("2015-01-01 10:00:00 UTC")
        );

        assert_eq!(ctx.resolve("docdate", none).as_deref(), Some("2019-01-02"));
        assert_eq!(
            ctx.resolve("doctime", none).as_deref(),
            Some("03:04:05 +0600")
        );
        assert_eq!(ctx.resolve("docyear", none).as_deref(), Some("2019"));
        assert_eq!(
            ctx.resolve("docdatetime", none).as_deref(),
            Some("2019-01-02 03:04:05 +0600")
        );

        assert_eq!(ctx.resolve("frog", none), None);
    }

    #[test]
    fn resolve_honors_explicit_overrides() {
        let ctx = ctx();

        // Explicit docdate + doctime: docyear from docdate, docdatetime from
        // both.
        let explicit = |name: &str| match name {
            "docdate" => Some("2015-01-01".to_string()),
            "doctime" => Some("10:00:00-0700".to_string()),
            _ => None,
        };
        assert_eq!(ctx.resolve("docyear", explicit).as_deref(), Some("2015"));
        assert_eq!(
            ctx.resolve("docdatetime", explicit).as_deref(),
            Some("2015-01-01 10:00:00-0700")
        );

        // Explicit docdate only: doctime still computed from the instant.
        let explicit = |name: &str| match name {
            "docdate" => Some("2015-01-01".to_string()),
            _ => None,
        };
        assert_eq!(
            ctx.resolve("docdatetime", explicit).as_deref(),
            Some("2015-01-01 03:04:05 +0600")
        );

        // An explicit date without a `YYYY-` prefix leaves the year unset.
        let explicit = |name: &str| match name {
            "docdate" => Some("January 1".to_string()),
            _ => None,
        };
        assert_eq!(ctx.resolve("docyear", explicit), None);

        // An explicit year wins outright over both the date-derived and the
        // instant-derived year. (The reader path never reaches this – an
        // explicitly-set attribute short-circuits before resolution – so it is
        // covered here directly.)
        let explicit = |name: &str| match name {
            "docyear" => Some("1999".to_string()),
            _ => None,
        };
        assert_eq!(ctx.resolve("docyear", explicit).as_deref(), Some("1999"));
    }
}
