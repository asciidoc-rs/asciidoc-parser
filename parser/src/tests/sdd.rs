// EXPERIMENT: Use no-op macros to indicate spec coverage.

// These macros are read by the temporary parser in the sdd folder.

// Use the track_file macro to indicate which .adoc spec file is being tracked.
macro_rules! track_file( ($($tt:tt)*) => {} );
pub(super) use track_file;

// Use the non_normative macro to signal blocks within the .adoc file that are
// non-normative (i.e. do not describe specific rules that must be obeyed).
macro_rules! non_normative( ($($tt:tt)*) => {} );
pub(super) use non_normative;

// Use the verifies macro to annotate a test block that verifies a specific
// section within the .adoc file that is normative.
macro_rules! verifies( ($($tt:tt)*) => {} );
pub(super) use verifies;

// Use the to_do_verifies macro to annotate a test block that doesn't yet verify
// a specific section within the .adoc file that is normative.
macro_rules! to_do_verifies( ($($tt:tt)*) => {} );
pub(super) use to_do_verifies;

// All lines in each .adoc file should be covered by either non_normative or
// verifies.
