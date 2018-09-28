extern crate xmltree;
extern crate clap;
extern crate regex;
extern crate chrono;

use clap::{App, Arg};
use chrono::{Date, DateTime, Utc, TimeZone};
use regex::{Regex, RegexSet, Captures};
use std::fs::File;
use std::error::Error;
use std::str::FromStr;
use xmltree::Element;

#[derive(Debug)]
pub struct Config {
    xml_file: String
}

#[derive(Debug)]
struct Attr {
    tag: String,
    value: String,
    units: Option<String>,
}

type MyResult<T> = Result<T, Box<Error>>;

// --------------------------------------------------
pub fn run(config: Config) -> MyResult<()> {
    let f = File::open(config.xml_file)?;
    let root = Element::parse(f)?;

    let id = get_primary_id(&root)?;
    println!("id {:?}", id);

    let attrs = get_attributes(&root)?;
    println!("runs {:?}", attrs);

    let runs = get_runs(&root);
    println!("runs {:?}", runs);

    let dates = get_dates(&attrs);
    println!("dates {:?}", dates);

    Ok(())
}

// --------------------------------------------------
pub fn get_args() -> MyResult<Config> {
    let matches = App::new("MExtract")
        .version("0.1.0")
        .author("Ken Youens-Clark <kyclark@email.arizona.edu>")
        .about("Extract metadata from ENA XML")
        .arg(
            Arg::with_name("xml")
                .short("x")
                .long("xml")
                .value_name("XML_FILE")
                .help("XML filename")
                .required(true),
        ).get_matches();

    let config = Config {
        xml_file: matches.value_of("xml").unwrap().to_string(),
    };

    Ok(config)
}

// --------------------------------------------------
fn get_primary_id(root: &Element) -> MyResult<String> {
    let ids = match root.get_child("IDENTIFIERS") {
        Some(x) => x,
        _ => return Err(From::from("Missing IDENTIFIERS"))
    };

    let primary_id = match ids.get_child("PRIMARY_ID") {
        Some(pid) => pid.text.as_ref(),
        _ => return Err(From::from("Missing PRIMARY_ID node"))
    };

    let id = match primary_id {
        Some(z) => z,
        _ => return Err(From::from("Missing PRIMARY_ID value"))
    };

    Ok(id.to_string())
}

// --------------------------------------------------
fn get_runs(root: &Element) -> Option<Vec<String>> {
    let mut runs: Vec<String> = vec![];
    if let Some(links) = root.get_child("SAMPLE_LINKS") {
        for link in links.children.iter() {
            if let Some(xref) = link.get_child("XREF_LINK") {
                if let Some(run) = xref.get_child("DB") {
                    if run.text == Some("ENA-RUN".to_string()) {
                        if let Some(id) = xref.get_child("ID") {
                            if let Some(s) = id.text.as_ref() {
                                for t in s.split(",") {
                                    runs.push(t.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Some(runs)
}

// --------------------------------------------------
fn get_child_text(element: &Element, tag: &str) -> Option<String> {
    element.get_child(tag).and_then(
        |child| child.text.as_ref().and_then(|val| Some(val.to_string())))
}

// --------------------------------------------------
fn get_attributes(root: &Element) -> MyResult<Vec<Attr>> {
    match root.get_child("SAMPLE_ATTRIBUTES") {
        Some(attributes) => {
            let mut attrs: Vec<Attr> = vec![];
            for attr in attributes.children.iter() { 
                if let Some(tag) = get_child_text(&attr, "TAG") {
                    if let Some(value) = get_child_text(&attr, "VALUE") {
                        attrs.push(Attr {
                            tag : tag,
                            value : value,
                            units : get_child_text(attr, "UNITS"),
                        });
                    }
                }
            }
            println!("attr = {:?}", attrs);
            Ok(attrs)
        }
        _ => Err(From::from("Missing SAMPLE_ATTRIBUTES"))
    }
}

// --------------------------------------------------
fn get_dates(attrs: &Vec<Attr>) -> Vec<u32> {
    let mut ret = vec![];
    let tags = [
        r"(?xi)
        ^
        (?:event|collection)
        [\s_]
        date
        (?:[/]time)?
        $
        ",
        r"(?xi)
        ^
        event
        [\s_]
        date
        [\s/_]
        time
        [\s_]
        (?:start)
        $
        ",
        r"(?xi)
        ^
        date
        $
        ",
        r"(?xi)
        ^
        collection_timestamp
        $
        ",
    ];

    //let excel = Regex::new(r"^\d{5}$").unwrap();
    //let iso_pattern = r"(?x)
        //^
        //(?P<base>\d{4}-\d{2}-\d{2}T\d+:\d+)
        //(?P<secs>[:]\d+)?
        //";
    let iso_pattern = r"(?x)
        ^
        (?P<year>\d{4})
        -
        (?P<month>\d{2})
        -
        (?P<day>\d{2})
        T
        (?P<hour>\d+)
        :
        (?P<minutes>\d+)
        (?:
          [:]
          (?P<seconds>\d+)
        )?
        ";
    let iso_re = Regex::new(iso_pattern).unwrap();
        //r"^(\d{4})[-](\d{1,2})(?:\/.+)?$",

    // cf https://docs.rs/chrono/0.4.0/chrono/format/strftime/index.html
    let tag_re = RegexSet::new(&tags).unwrap();
    for attr in attrs.iter() {
        let val = &attr.value;
        let tag_match = tag_re.is_match(&attr.tag);

        println!("\n\n{} = {}", attr.tag, val);

        if let Some(cap) = iso_re.captures(val) {
            println!("cap = {:?}", cap);
            if let Some(dt) = cap_to_dt(&cap) {
                println!("dt = {:?}", dt);
            }
        }

        ret.push(if tag_match { 1 } else { 0 });
    }
    ret
}

// --------------------------------------------------
fn cap_to_int<T: FromStr>(cap: &Captures, name: &str) -> Option<T> {
    match cap.name(name) {
        Some(val) => {
            match val.as_str().parse::<T>() {
                Ok(i) => Some(i),
                _ => None,
            }
        }
        _ => None
    }
}

// --------------------------------------------------
fn cap_to_dt(cap: &Captures) -> Option<DateTime<Utc>> {
    let hour = cap_to_int::<u32>(&cap, "hour").unwrap_or(0);
    let minutes = cap_to_int::<u32>(&cap, "minutes").unwrap_or(0);
    let seconds = cap_to_int::<u32>(&cap, "seconds").unwrap_or(0);

    cap_to_int::<i32>(&cap, "year").and_then(|year|
        cap_to_int::<u32>(&cap, "month").and_then(|month|
            cap_to_int::<u32>(&cap, "day").and_then(|day|
                Some(Utc.ymd(year, month, day).and_hms(hour, minutes, seconds)))))
}
