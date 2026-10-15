use std::cell::RefCell;
use std::hint::black_box;
use std::time::Duration;

use criterion::measurement::WallTime;
use criterion::{BatchSize, BenchmarkGroup, Criterion, Throughput};
use ferroni::api::{Regex, SearchOptions};
use ferroni::ffi::{CRegex, CRegion};
use ferroni::oniguruma::{
    ONIG_OPTION_CAPTURE_GROUP, ONIG_OPTION_EXTEND, ONIG_OPTION_IGNORECASE, ONIG_OPTION_MULTILINE,
    ONIG_OPTION_NEGATE_SINGLELINE, ONIG_OPTION_SINGLELINE,
};
use serde_json::{Value, json};

use crate::engines::{CompileOptions, Compiled, Engine};

const CATALOG: &str = include_str!("catalog.json");
const TOKENIZER_CATALOG: &str = include_str!("../tokenizer_regex/catalog.json");
type Bounds = Vec<Option<(usize, usize)>>;

#[derive(Debug, PartialEq)]
enum Output {
    Boolean(bool),
    Match(Option<(usize, usize)>),
    Captures(Option<Bounds>),
    Matches(Vec<Bounds>),
    Spans(Vec<(usize, usize)>),
    Text(String),
    Chunks(Vec<String>),
}

struct Case {
    id: String,
    operation: String,
    pattern: String,
    flags: CompileOptions,
    inputs: Vec<Input>,
    replacement: String,
    pending: Option<String>,
    slice_at_start: bool,
}

struct Input {
    text: String,
    start: usize,
    expected: Output,
}

impl Case {
    fn parse(value: &Value) -> Self {
        let operation = value["operation"].as_str().unwrap().to_owned();
        let pending = value["pending_reason"].as_str().map(str::to_owned);
        let inputs = value["inputs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|input| Input {
                text: input["text"].as_str().unwrap().to_owned(),
                start: input["start"].as_u64().unwrap_or(0) as usize,
                expected: parse_output(&operation, &input["expected"]),
            })
            .collect();
        let flag = |name: &str| value["flags"][name].as_bool().unwrap_or(false);
        Self {
            id: value["id"].as_str().unwrap().to_owned(),
            operation,
            pattern: value["pattern"].as_str().unwrap().to_owned(),
            flags: CompileOptions {
                ignore_case: flag("ignore_case"),
                multiline: flag("multiline"),
                dotall: flag("dotall"),
                extended: flag("extended"),
                capture_group: true,
            },
            inputs,
            replacement: value["replacement"].as_str().unwrap_or("").to_owned(),
            pending,
            slice_at_start: value["anchor_mode"].as_str() == Some("slice"),
        }
    }

    fn bytes(&self) -> u64 {
        self.inputs
            .iter()
            .map(|input| input.text.len() as u64)
            .sum()
    }
}

fn parse_bounds(value: &Value) -> Bounds {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|bound| {
            if bound.is_null() {
                None
            } else {
                Some((
                    bound[0].as_u64().unwrap() as usize,
                    bound[1].as_u64().unwrap() as usize,
                ))
            }
        })
        .collect()
}

fn parse_output(operation: &str, value: &Value) -> Output {
    match operation {
        "is_match" => Output::Boolean(value.as_bool().unwrap()),
        "find" | "find_at" => Output::Match(if value.is_null() {
            None
        } else {
            Some((
                value[0].as_u64().unwrap() as usize,
                value[1].as_u64().unwrap() as usize,
            ))
        }),
        "captures" | "captures_at" => {
            Output::Captures((!value.is_null()).then(|| parse_bounds(value)))
        }
        "all_captures" => {
            Output::Matches(value.as_array().unwrap().iter().map(parse_bounds).collect())
        }
        "all_matches" => Output::Spans(
            value
                .as_array()
                .unwrap()
                .iter()
                .map(|span| {
                    (
                        span[0].as_u64().unwrap() as usize,
                        span[1].as_u64().unwrap() as usize,
                    )
                })
                .collect(),
        ),
        "replace" => Output::Text(value.as_str().unwrap().to_owned()),
        "graphemes" => Output::Chunks(
            value
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s.as_str().unwrap().to_owned())
                .collect(),
        ),
        _ => panic!("unknown operation: {operation}"),
    }
}

enum Library {
    Ferroni(Regex, bool),
    C(CRegex, RefCell<CRegion>),
    Regex(regex::Regex),
    Other(Compiled),
}

impl Library {
    fn compile(engine: &str, case: &Case) -> Result<Self, String> {
        let flags = case.flags;
        match engine {
            "rust" | "rust_erroraware" => Regex::builder(&case.pattern)
                .case_insensitive(flags.ignore_case)
                .multi_line_anchors(flags.multiline)
                .dot_matches_newline(flags.dotall)
                .extended(flags.extended)
                .capture_group(true)
                .build()
                .map(|regex| Self::Ferroni(regex, engine == "rust_erroraware"))
                .map_err(|error| error.to_string()),
            "c" => {
                let mut options = ONIG_OPTION_CAPTURE_GROUP;
                for (enabled, flag) in [
                    (flags.ignore_case, ONIG_OPTION_IGNORECASE),
                    (flags.multiline, ONIG_OPTION_NEGATE_SINGLELINE),
                    (!flags.multiline, ONIG_OPTION_SINGLELINE),
                    (flags.dotall, ONIG_OPTION_MULTILINE),
                    (flags.extended, ONIG_OPTION_EXTEND),
                ] {
                    if enabled {
                        options |= flag;
                    }
                }
                CRegex::new(case.pattern.as_bytes(), options.bits())
                    .map(|regex| Self::C(regex, RefCell::new(CRegion::new())))
                    .map_err(|error| format!("Oniguruma compile error {error}"))
            }
            "regex" => regex::RegexBuilder::new(&case.pattern)
                .case_insensitive(flags.ignore_case)
                .multi_line(flags.multiline)
                .dot_matches_new_line(flags.dotall)
                .ignore_whitespace(flags.extended)
                .build()
                .map(Self::Regex)
                .map_err(|error| error.to_string()),
            _ => Engine::PER_PATTERN
                .iter()
                .find(|candidate| candidate.id() == engine)
                .ok_or_else(|| format!("engine {engine} was not built"))?
                .compile_with_options(&case.pattern, flags)
                .map(Self::Other),
        }
    }

    fn options() -> SearchOptions {
        // Matching errors must remain distinguishable from ordinary no-match.
        // The outer process timeout bounds engines without an equivalent API.
        SearchOptions::new()
    }

    fn find(&self, text: &str, start: usize) -> Result<Option<(usize, usize)>, String> {
        match self {
            Self::Ferroni(regex, error_aware) => {
                let found = if *error_aware {
                    regex
                        .find_at_with(text, start, Self::options())
                        .map_err(|error| error.to_string())?
                } else {
                    regex.find_at(text, start)
                };
                Ok(found.map(|m| (m.start(), m.end())))
            }
            Self::Regex(regex) => Ok(regex.find_at(text, start).map(|m| (m.start(), m.end()))),
            Self::Other(regex) => regex.find(text, start),
            Self::C(regex, region) => {
                let mut region = region.borrow_mut();
                let position =
                    regex.search(text.as_bytes(), start, text.len(), Some(&mut region), 0);
                if position < -1 {
                    return Err(format!("Oniguruma search error {position}"));
                }
                Ok((position >= 0).then(|| {
                    let (start, end) = region
                        .match_range()
                        .expect("successful search sets group 0");
                    (start as usize, end as usize)
                }))
            }
        }
    }

    fn captures(&self, text: &str, start: usize) -> Result<Option<Bounds>, String> {
        let raw_bounds = |raw: Vec<(i32, i32)>| {
            raw.into_iter()
                .map(|(s, e)| (s >= 0).then_some((s as usize, e as usize)))
                .collect()
        };
        match self {
            Self::Ferroni(regex, error_aware) => {
                let found = if *error_aware {
                    regex
                        .captures_at_with(text, start, Self::options())
                        .map_err(|error| error.to_string())?
                } else {
                    regex.captures_at(text, start)
                };
                Ok(found.map(|caps| {
                    caps.iter()
                        .map(|m| m.map(|m| (m.start(), m.end())))
                        .collect()
                }))
            }
            Self::Regex(regex) => Ok(regex.captures_at(text, start).map(|caps| {
                caps.iter()
                    .map(|m| m.map(|m| (m.start(), m.end())))
                    .collect()
            })),
            Self::Other(regex) => regex
                .captures(text, start)
                .map(|found| found.map(raw_bounds)),
            Self::C(regex, region) => {
                let mut region = region.borrow_mut();
                let position =
                    regex.search(text.as_bytes(), start, text.len(), Some(&mut region), 0);
                if position < -1 {
                    return Err(format!("Oniguruma search error {position}"));
                }
                Ok((position >= 0).then(|| raw_bounds(region.capture_ranges())))
            }
        }
    }

    fn all_captures(&self, input: &Input) -> Result<Vec<Bounds>, String> {
        match self {
            Self::Ferroni(regex, true) => regex
                .captures_iter_with(&input.text, Self::options())
                .map(|result| {
                    result
                        .map(|caps| {
                            caps.iter()
                                .map(|m| m.map(|m| (m.start(), m.end())))
                                .collect()
                        })
                        .map_err(|error| error.to_string())
                })
                .collect(),
            Self::Ferroni(regex, false) => Ok(regex
                .captures_iter(&input.text)
                .map(|caps| {
                    caps.iter()
                        .map(|m| m.map(|m| (m.start(), m.end())))
                        .collect()
                })
                .collect()),
            Self::Regex(regex) => Ok(regex
                .captures_iter(&input.text)
                .map(|caps| {
                    caps.iter()
                        .map(|m| m.map(|m| (m.start(), m.end())))
                        .collect()
                })
                .collect()),
            _ => {
                let mut result = Vec::new();
                let mut start = input.start;
                while let Some(captures) = self.captures(&input.text, start)? {
                    let (begin, end) = captures[0].expect("whole match must be set");
                    result.push(captures);
                    if begin == end {
                        let Some(next) = next_boundary(&input.text, end) else {
                            break;
                        };
                        start = next;
                    } else {
                        start = end;
                    }
                }
                Ok(result)
            }
        }
    }

    fn all_matches(&self, input: &Input) -> Result<Vec<(usize, usize)>, String> {
        match self {
            Self::Ferroni(regex, false) => {
                return Ok(regex
                    .find_iter(&input.text)
                    .map(|m| (m.start(), m.end()))
                    .collect());
            }
            Self::Ferroni(regex, true) => {
                return regex
                    .find_iter_with(&input.text, Self::options())
                    .map(|result| {
                        result
                            .map(|m| (m.start(), m.end()))
                            .map_err(|error| error.to_string())
                    })
                    .collect();
            }
            Self::Regex(regex) => {
                return Ok(regex
                    .find_iter(&input.text)
                    .map(|m| (m.start(), m.end()))
                    .collect());
            }
            Self::Other(regex) => {
                if let Some(matches) = regex.native_all_matches(&input.text) {
                    return matches;
                }
            }
            Self::C(_, _) => {}
        }
        let mut spans = Vec::new();
        let mut start = input.start;
        while let Some((begin, end)) = self.find(&input.text, start)? {
            spans.push((begin, end));
            if begin == end {
                let Some(next) = next_boundary(&input.text, end) else {
                    break;
                };
                start = next;
            } else {
                start = end;
            }
        }
        Ok(spans)
    }

    fn replace(&self, input: &Input, replacement: &str) -> Result<String, String> {
        match self {
            Self::Ferroni(regex, _) => Ok(regex
                .replace_all(&input.text, ferroni::NoExpand(replacement))
                .into_owned()),
            Self::Regex(regex) => Ok(regex
                .replace_all(&input.text, regex::NoExpand(replacement))
                .into_owned()),
            Self::Other(regex) => {
                if let Some(result) = regex.native_replace(&input.text, replacement) {
                    result
                } else {
                    self.replace_from_spans(input, replacement)
                }
            }
            Self::C(_, _) => self.replace_from_spans(input, replacement),
        }
    }

    fn replace_from_spans(&self, input: &Input, replacement: &str) -> Result<String, String> {
        let text = &input.text;
        let mut output = String::with_capacity(text.len());
        let (mut previous, mut position) = (0, input.start);
        while let Some((start, end)) = self.find(text, position)? {
            output.push_str(&text[previous..start]);
            output.push_str(replacement);
            previous = end;
            if start == end {
                let Some(next) = next_boundary(text, end) else {
                    break;
                };
                position = next;
            } else {
                position = end;
            }
        }
        output.push_str(&text[previous..]);
        Ok(output)
    }

    fn execute(&self, case: &Case, input: &Input) -> Result<Output, String> {
        if case.slice_at_start {
            // These context-free lexer patterns have an explicit \A wrapper.
            // A borrowed slice implements Python's match-at-position contract;
            // bounds are rebased before returning the observable result.
            let bounds = self.captures(&input.text[input.start..], 0)?;
            return Ok(Output::Captures(bounds.map(|groups| {
                groups
                    .into_iter()
                    .map(|bound| bound.map(|(s, e)| (s + input.start, e + input.start)))
                    .collect()
            })));
        }
        match case.operation.as_str() {
            "is_match" => {
                let matched = match self {
                    Self::Ferroni(regex, true) => regex
                        .is_match_with(&input.text, Self::options())
                        .map_err(|e| e.to_string())?,
                    Self::Ferroni(regex, false) => regex.is_match(&input.text),
                    Self::Regex(regex) => regex.is_match(&input.text),
                    Self::Other(regex) => regex.is_match(&input.text, input.start)?,
                    Self::C(regex, _) => {
                        let position = regex.search(
                            input.text.as_bytes(),
                            input.start,
                            input.text.len(),
                            None,
                            0,
                        );
                        if position < -1 {
                            return Err(format!("Oniguruma search error {position}"));
                        }
                        position >= 0
                    }
                };
                Ok(Output::Boolean(matched))
            }
            "find" | "find_at" => Ok(Output::Match(self.find(&input.text, input.start)?)),
            "captures" | "captures_at" => {
                Ok(Output::Captures(self.captures(&input.text, input.start)?))
            }
            "all_captures" => Ok(Output::Matches(self.all_captures(input)?)),
            "all_matches" => Ok(Output::Spans(self.all_matches(input)?)),
            "replace" => Ok(Output::Text(self.replace(input, &case.replacement)?)),
            "graphemes" => {
                let captures = self.all_captures(input)?;
                let mut chunks = Vec::new();
                let mut previous = 0;
                for groups in captures {
                    let (start, end) = groups[0].expect("whole match must be set");
                    if previous < start {
                        chunks.push(input.text[previous..start].to_owned());
                    }
                    chunks.push(input.text[start..end].to_owned());
                    previous = end;
                }
                if previous < input.text.len() {
                    chunks.push(input.text[previous..].to_owned());
                }
                Ok(Output::Chunks(chunks))
            }
            other => Err(format!("unknown operation {other}")),
        }
    }

    fn run(&self, case: &Case) -> Result<(), String> {
        for input in &case.inputs {
            // Consume each operation's output immediately, as independent
            // library calls do. Do not add an artificial batch-result vector
            // to boolean or first-span operations.
            black_box(self.execute(case, black_box(input))?);
        }
        Ok(())
    }
}

fn next_boundary(text: &str, at: usize) -> Option<usize> {
    text[at..]
        .chars()
        .next()
        .map(|character| at + character.len_utf8())
}

fn eligibility(engine: &str, case: &Case) -> Result<Library, Value> {
    let failure = |status: &str, reason: String| json!({"case":case.id,"engine":engine,"status":status,"reason":reason});
    if let Some(reason) = &case.pending {
        return Err(failure("pending", reason.clone()));
    }
    if case.inputs.is_empty() {
        return Err(failure("pending", "no oracle fixtures".to_owned()));
    }
    let library = Library::compile(engine, case).map_err(|reason| {
        failure(
            if reason.starts_with("API unsupported:") {
                "api_unsupported"
            } else {
                "syntax_rejected"
            },
            reason,
        )
    })?;
    // Plain public APIs can report a limit as no match. Validate the same
    // fixtures through the fallible profile before admitting their timings.
    let error_check = (engine == "rust")
        .then(|| Library::compile("rust_erroraware", case).expect("same Ferroni compiler"));
    for (index, input) in case.inputs.iter().enumerate() {
        if let Some(check) = &error_check {
            check
                .execute(case, input)
                .map_err(|reason| failure("execution_error", reason))?;
        }
        let actual = library
            .execute(case, input)
            .map_err(|reason| failure("execution_error", reason))?;
        if actual != input.expected {
            return Err(failure(
                "semantic_difference",
                format!(
                    "input {index}: expected {:?}, got {actual:?}",
                    input.expected
                ),
            ));
        }
    }
    Ok(library)
}

/// Applies the harness's window for `lane`, when it set one.
fn lane_budget(group: &mut BenchmarkGroup<'_, WallTime>, budgets: Option<&Value>, lane: &str) {
    if let Some(budgets) = budgets {
        let seconds = |key: &str| {
            Duration::from_secs_f64(budgets[lane][key].as_f64().expect("lane budget in seconds"))
        };
        group.warm_up_time(seconds("warmup_seconds"));
        group.measurement_time(seconds("measure_seconds"));
    }
}

fn engines() -> Vec<&'static str> {
    let mut result = vec!["rust", "rust_erroraware", "c", "regex"];
    result.extend(Engine::PER_PATTERN.iter().map(|engine| engine.id()));
    result
}

fn selected(value: &str, variable: &str) -> bool {
    std::env::var(variable).map_or(true, |wanted| wanted == value)
}

pub fn main() {
    let source = if std::env::var("PRODUCTION_CORPUS").as_deref() == Ok("tokenizer") {
        TOKENIZER_CATALOG
    } else {
        CATALOG
    };
    let catalog: Value =
        serde_json::from_str(source).expect("production catalog must be valid JSON");
    let args: Vec<_> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--catalog") {
        println!("{catalog}");
        return;
    }
    let cases: Vec<_> = catalog["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(Case::parse)
        .filter(|case| selected(&case.id, "PRODUCTION_CASE"))
        .collect();
    assert!(!cases.is_empty(), "no production cases selected");
    let validate = args.iter().any(|arg| arg == "--validate");
    let execute_once = args.iter().any(|arg| arg == "--execute-once");
    let compile_once = args.iter().any(|arg| arg == "--compile-once");
    let engines: Vec<_> = engines()
        .into_iter()
        .filter(|engine| selected(engine, "PRODUCTION_ENGINE"))
        .collect();
    assert!(!engines.is_empty(), "no engine selected");
    if validate || execute_once || compile_once {
        for case in &cases {
            for engine in &engines {
                let result = if validate {
                    eligibility(engine, case)
                } else {
                    if let Some(reason) = &case.pending {
                        Err(
                            json!({"case":case.id,"engine":engine,"status":"pending","reason":reason}),
                        )
                    } else {
                        Library::compile(engine, case).map_err(|reason| json!({
                        "case":case.id,"engine":engine,"status":"syntax_rejected","reason":reason
                    }))
                    }
                };
                match result {
                    Ok(library) => {
                        if execute_once {
                            library.run(case).unwrap();
                        }
                        println!(
                            "{}",
                            json!({"case":case.id,"engine":engine,"status":"eligible","inputs":case.inputs.len(),"bytes":case.bytes()})
                        );
                        black_box(library);
                    }
                    Err(record) => println!("{record}"),
                }
            }
        }
        return;
    }
    let mut criterion = Criterion::default().configure_from_args();
    let mut group = criterion.benchmark_group("production_regex");
    // The harness sets each lane's window here; without it, the command-line
    // window applies to every lane.
    let budgets: Option<Value> = std::env::var("PRODUCTION_LANE_BUDGETS")
        .ok()
        .map(|text| serde_json::from_str(&text).expect("lane budgets must be valid JSON"));
    for case in &cases {
        for engine in &engines {
            let library = match eligibility(engine, case) {
                Ok(library) => library,
                Err(record) => {
                    println!("ELIGIBILITY {record}");
                    continue;
                }
            };
            group.throughput(Throughput::Elements(case.inputs.len() as u64));
            lane_budget(&mut group, budgets.as_ref(), "steady");
            group.bench_function(format!("{}/steady/{engine}", case.id), |b| {
                b.iter(|| library.run(case).expect("validated execution must succeed"));
            });
            // Criterion drops the returned compiled objects outside the timed
            // routine. Both construction lanes include each engine's required
            // search contexts and any selected JIT compilation.
            group.throughput(Throughput::Elements(1));
            lane_budget(&mut group, budgets.as_ref(), "compile");
            group.bench_function(format!("{}/compile/{engine}", case.id), |b| {
                b.iter_batched(
                    || (),
                    |()| black_box(Library::compile(engine, case).unwrap()),
                    BatchSize::SmallInput,
                );
            });
            lane_budget(&mut group, budgets.as_ref(), "first_use");
            group.bench_function(format!("{}/first_use/{engine}", case.id), |b| {
                b.iter_batched(
                    || (),
                    |()| {
                        let library = Library::compile(engine, case).unwrap();
                        let output = library.execute(case, &case.inputs[0]).unwrap();
                        black_box((library, output))
                    },
                    BatchSize::SmallInput,
                );
            });
        }
    }
    group.finish();
    criterion.final_summary();
}
