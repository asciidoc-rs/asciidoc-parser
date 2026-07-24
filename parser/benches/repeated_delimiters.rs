use asciidoc_parser::Parser;
use codspeed_criterion_compat::{Criterion, black_box, criterion_group, criterion_main};

const BENCH_NAME: &str = "repeated example delimiters";

// A long run of consecutive `====` delimiter lines with no blank line between
// them. This shape previously parsed in O(n²) time: the quoted-paragraph parser
// rescanned the entire remaining input for every block. The bench guards that
// the delimiter-scan / block-dispatch path stays linear.
const DELIMITER_COUNT: usize = 4000;

pub fn perf(c: &mut Criterion) {
    let parse_text = "====\n".repeat(DELIMITER_COUNT);

    c.bench_function(BENCH_NAME, |b| {
        b.iter(|| Parser::default().parse(black_box(parse_text.as_str())))
    });
}

criterion_group!(benches, perf);
criterion_main!(benches);
