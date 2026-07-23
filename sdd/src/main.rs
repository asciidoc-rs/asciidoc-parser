// Quick and dirty tool to generate spec coverage for asciidoc-parser. Not
// intended at this time to generalize to any other use case.

// If asciidoc-parser goes well, I may build a more robust version of this at a
// later time. For now, please excuse the hard-coded settings and other
// shortcuts taken.

use std::{collections::HashMap, fs, io::BufRead, path::Path};

use walkdir::{DirEntry, WalkDir};

// Spec sources whose lines we measure coverage against, as `(root, extension,
// within)` triples: the normative documentation prose (`.adoc`) of the AsciiDoc
// language description, plus the Asciidoctor reference test suite (`.rb`) that
// this crate is validated against.
//
// `within`, when `Some`, restricts a source to files whose path contains that
// segment.
//
// NOTE: The Asciidoctor test suite is being ported one file at a time (see
// `ref/asciidoctor/README.md`); every `.rb` file vendored under
// `ref/asciidoctor/test` is picked up here automatically. Coverage grows as
// more files are ported — at present `attribute_list_test.rb` is the first.
const SPEC_SOURCES: &[(&str, &str, Option<&str>)] = &[
    ("../ref/asciidoc-lang/docs/modules", ".adoc", None),
    ("../ref/asciidoctor/test", ".rb", None),
];

fn main() {
    let mut spec_coverage: HashMap<String, Vec<(String, bool)>> = HashMap::new();

    for entry in collect_files("../parser/src/tests", ".rs") {
        let path = entry.path();
        if let Some((spec_path, cov)) = parse_rs_file(path) {
            spec_coverage.insert(spec_path, cov);
        }
    }

    println!("{{\n    \"coverage\": {{");

    let mut spec_files: Vec<DirEntry> = vec![];
    for (root, extension, within) in SPEC_SOURCES {
        for entry in collect_files(root, extension) {
            // Skip files outside the source's required path segment.
            if let Some(segment) = within
                && !entry.path().to_str().unwrap().contains(segment)
            {
                continue;
            }
            spec_files.push(entry);
        }
    }

    // `saturating_sub` guards the empty case (e.g. a `ref/` source not present):
    // the loop below then simply doesn't run, emitting a valid empty coverage
    // object.
    let last_index = spec_files.len().saturating_sub(1);

    for (count, entry) in spec_files.into_iter().enumerate() {
        let path = entry.path().to_str().unwrap().trim_start_matches("../");
        // (unwrap: Should have been filtered out above.)

        // if !path.contains("/revision-line.adoc") {
        //     continue;
        // }
        println!("        {path:?}: {{");

        emit_coverage(path, spec_coverage.get(path));

        if count < last_index {
            println!("        }},");
        } else {
            println!("        }}");
        }
    }

    println!("    }}\n}}");
}

// Collect every file ending in `extension` under `root`, skipping dotfiles.
// Returns an empty vec if the root doesn't exist, so a spec source that isn't
// vendored simply contributes no coverage.
fn collect_files(root: &str, extension: &str) -> Vec<DirEntry> {
    WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            if let Some(file_name) = e.file_name().to_str() {
                !file_name.starts_with('.')
            } else {
                false
            }
        })
        .filter_map(|e| {
            let e = e.ok()?;

            if !e.file_type().is_file() {
                return None;
            }

            if let Some(file_name) = e.file_name().to_str()
                && file_name.ends_with(extension)
            {
                Some(e)
            } else {
                None
            }
        })
        .collect()
}

fn parse_rs_file(path: &Path) -> Option<(String, Vec<(String, bool)>)> {
    let rs_file = fs::read(path).unwrap();

    let mut tracked_file: Option<String> = None;
    let mut lines: Vec<(String, bool)> = vec![];
    let mut in_non_normative_block = false;
    let mut in_verifies_block = false;
    let mut expect_track_file_path = false;

    // Number of `#` guards in the raw-string delimiter that opened the block we
    // are currently inside (`Some(1)` for `r#"`, `Some(2)` for `r##"`, …), or
    // `None` when not inside a reproduced Ruby block. The closing delimiter must
    // match this length, so a reproduced line that merely starts with a shorter
    // `"#` / `"##` sequence does not close the block early.
    let mut raw_hashes: Option<usize> = None;

    for line in rs_file.lines() {
        let line = line.unwrap();

        // Single-line form: `track_file!("path");`.
        if let Some(tf) = line.strip_prefix("track_file!(\"")
            && let Some(tf) = tf.strip_suffix("\");")
        {
            if tracked_file.is_some() {
                panic!("ERROR: {path:?} contains multiple track_file! macros");
            }
            tracked_file = Some(tf.to_string());
            continue;
        }

        // Multi-line form (produced by rustfmt when the path is too long to fit
        // on one line):
        //
        //     track_file!(
        //         "path"
        //     );
        if line.trim_end() == "track_file!(" {
            expect_track_file_path = true;
            continue;
        }

        if expect_track_file_path {
            expect_track_file_path = false;
            if let Some(tf) = line.trim().strip_prefix('"')
                && let Some(tf) = tf.strip_suffix('"')
            {
                if tracked_file.is_some() {
                    panic!("ERROR: {path:?} contains multiple track_file! macros");
                }
                tracked_file = Some(tf.to_string());
            }
            continue;
        }

        if line.contains("non_normative!(") {
            // println!("NN+");
            in_non_normative_block = true;
            in_verifies_block = false;
            continue;
        }

        if line.contains("verifies!(") {
            // println!("VF+");
            in_non_normative_block = false;
            in_verifies_block = true;
            continue;
        }

        // Closing delimiter of the current block. It must match the hash length
        // of the opener (`"#` for `r#"`, `"##` for `r##"`, …); a reproduced line
        // that merely starts with a shorter guard is left as block content.
        if let Some(hashes) = raw_hashes {
            let closer = format!("\"{}", "#".repeat(hashes));
            if line.trim_end() == closer {
                // println!("QQQ");
                in_non_normative_block = false;
                in_verifies_block = false;
                raw_hashes = None;
                continue;
            }
        }

        // Opening delimiter of a reproduced Ruby block. We accept progressively
        // longer hash guards (`r#"`, `r##"`, `r###"`) so a reproduced line may
        // itself contain a shorter `"#`/`"##` sequence without prematurely
        // closing the raw string (see `links_test.rb` line 664). The matching
        // closer length is remembered in `raw_hashes` (checked above).
        if line.ends_with("r###\"") {
            raw_hashes = Some(3);
            continue;
        }
        if line.ends_with("r##\"") {
            raw_hashes = Some(2);
            continue;
        }
        if line.ends_with("r#\"") {
            raw_hashes = Some(1);
            continue;
        }

        if in_non_normative_block {
            // println!("NN  {line}");
            lines.push((line, false));
        } else if in_verifies_block {
            // println!("VF  {line}");
            lines.push((line, true));
        } else {
            // println!("--  {line}");
        }
    }

    tracked_file.map(|tracked_file| (tracked_file, lines))
}

fn emit_coverage(path: &str, coverage: Option<&Vec<(String, bool)>>) {
    // if !path.contains("/id.adoc") {
    //     return;
    // }

    let path = format!("../{path}");
    let adoc_file = fs::read(path).unwrap();

    let empty_coverage: Vec<(String, bool)> = vec![];
    let coverage = if let Some(coverage) = coverage.as_ref() {
        coverage
    } else {
        &empty_coverage
    };

    let mut coverage_lines = coverage.iter();

    let mut output_lines: Vec<String> = vec![];

    for (count, line) in adoc_file.lines().enumerate() {
        let line = line.unwrap();
        let count = count + 1;

        // println!("\n\n{count:4}: {line}");

        let coverage_line = coverage_lines.next();

        if line.is_empty() {
            continue;
        }

        if let Some((cov_line, is_normative)) = coverage_line {
            // println!("      {cov_line}");
            if cov_line == &line && *is_normative {
                output_lines.push(format!("            \"{count}\": 1"));
            }
        } else {
            output_lines.push(format!("            \"{count}\": 0"));
        }
    }

    if output_lines.is_empty() {
        return;
    }

    let last_output_line_index = output_lines.len() - 1;

    for (count, line) in output_lines.iter().enumerate() {
        if count < last_output_line_index {
            println!("{line},");
        } else {
            println!("{line}");
        }
    }
}
