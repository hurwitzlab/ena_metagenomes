extern crate xmltree;
extern crate clap;
extern crate regex;
extern crate chrono;
extern crate time;

use clap::{App, Arg};
use chrono::{Date, DateTime, Utc, TimeZone};
use regex::{Regex, RegexSet, Captures};
use std::fs::{self, File};
use std::error::Error;
use std::str::FromStr;
use time::Duration;
use xmltree::Element;

#[derive(Debug)]
pub struct Config {
    input: Vec<String>
}

#[derive(Debug)]
struct Attr {
    tag: String,
    value: String,
    units: Option<String>,
}

type MyResult<T> = Result<T, Box<Error>>;

// --------------------------------------------------
// Public
// --------------------------------------------------
pub fn run(config: Config) -> MyResult<()> {
    let files = find_files(&config.input)?;
    println!(
        "Will process {} file{}",
        files.len(),
        if files.len() == 1 { "" } else { "s" }
    );

    for (i, file) in files.iter().enumerate() {
        println!("{}: {}", i + 1, file);
        let f = File::open(file)?;
        let root = Element::parse(f)?;

        if let Err(e) = parse_xml(root) {
            eprintln!("Error: {}", e);
        }
        break;
    }

    Ok(())
}

// --------------------------------------------------
pub fn get_args() -> MyResult<Config> {
    let matches = App::new("MExtract")
        .version("0.1.0")
        .author("Ken Youens-Clark <kyclark@email.arizona.edu>")
        .about("Extract metadata from ENA XML")
        //.arg(
            //Arg::with_name("xml")
                //.short("x")
                //.long("xml")
                //.value_name("XML_FILE")
                //.help("XML filename")
                //.required(true),
        //)
        .arg(Arg::with_name("input").value_name("file.xml").multiple(true))
        .get_matches();

    let config = Config {
        //xml_file: matches.value_of("xml").unwrap().to_string(),
        input: matches.values_of_lossy("input").unwrap(),
    };

    Ok(config)
}

// --------------------------------------------------
// Private
// --------------------------------------------------
fn find_files(paths: &Vec<String>) -> Result<Vec<String>, Box<Error>> {
    let mut files = vec![];
    for path in paths {
        let meta = fs::metadata(path)?;
        if meta.is_file() {
            files.push(path.to_owned());
        } else {
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                let meta = entry.metadata()?;
                if meta.is_file() {
                    files.push(entry.path().display().to_string());
                }
            }
        };
    }

    if files.len() == 0 {
        return Err(From::from("No input files"));
    }

    Ok(files)
}

// --------------------------------------------------
fn parse_xml(root: Element) -> MyResult<()> {
    let id = get_primary_id(&root)?;
    println!("id {:?}", id);

    let attrs = get_attributes(&root)?;
    println!("attr {:?}", attrs);

    let runs = get_runs(&root);
    println!("runs {:?}", runs);

    let dates = get_dates(&attrs);
    println!("dates {:?}", dates);

    Ok(())
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
fn parse_datetime(val: &str) -> Option<DateTime<Utc>> {
    let patterns = vec![
        // Excel
        r"^(?P<excel>\d{5})$",

        // ISO (sort of)
        r"(?x)
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
        ",

        // 2017-06-16Z
        r"(?x)
        ^
        (?P<year>\d{4})
        -
        (?P<month>\d{2})
        -
        (?P<day>\d{2})
        Z
        $
        ",

        // 2017-06-16/2017-07-09
        r"(?x)
        ^
        (?P<year>\d{4})
        -
        (?P<month>\d{2})
        -
        (?P<day>\d{2})
        /
        \d{4}
        -
        \d{2}
        -
        \d{2}
        $
        ",

        // 2015-01, 2015-01/2015-02
        r"(?x)
        ^
        (?P<year>\d{4})
        -
        (?P<month>\d{1,2})
        (?:
        /
        \d{4}
        -
        \d{1,2}
        )?
        $
        ",

        // 20100910
        r"(?x)
        ^
        (?P<year>\d{4})
        (?P<month>\d{2})
        (?P<day>\d{2})
        $
        ",

        // 12/06, 2/14-6/14
        r"(?x)
        ^
        (?P<month>\d{1,2})
        /
        (?P<year>\d{2})
        (?:
        -
        \d{1,2}
        /
        \d{2}
        )?
        $
        ",

        // Dec-2015
        r"(?xi)
        ^
        (?P<month>jan|feb|mar|apr|may|jun|jul|aug|sep|oct|nov|dec)
        [^-]*
        [,-]
        \s*
        (?P<year>\d{4})
        $
        ",

        // March-April 2017
        r"(?xi)
        ^
        (?P<month>january|february|march|april|may|june|july|
        august|september|october|november|december)
        -
        (?:january|february|march|april|may|june|july|
        august|september|october|november|december)
        \s+
        (?P<year>\d{4})
        $
        ",

        // July of 2011
        r"(?xi)
        ^
        (?P<month>january|february|march|april|may|june|july|
        august|september|october|november|december)
        \s+
        of
        \s+
        (?P<year>\d{4})
        $
        ",

        // 2008 August
        r"(?xi)
        ^
        (?P<year>\d{4})
        \s+
        (?P<month>january|february|march|april|may|june|july|
        august|september|october|november|december)
        $
        ",
    ];

    for p in patterns {
        //println!("v = {} p = {}", val, p);
        let re = Regex::new(&p).unwrap();
        if let Some(cap) = re.captures(&val) {
            //println!("YAY! {:?}", cap);
            if let Some(dt) = cap_to_dt(&cap) {
                return Some(dt)
            }
        }
    }

    None
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
fn month_to_int(month_name: &str) -> Option<u32> {
    let months = vec![
        r"(?i)^jan(uary)?",
        r"(?i)^feb(ruary)?",
        r"(?i)^mar(ch)?",
        r"(?i)^apr(il)?",
        r"(?i)^ma[yi]?",
        r"(?i)^june?",
        r"(?i)^july?",
        r"(?i)^aug(usto?)?",
        r"(?i)^sep(tember)?",
        r"(?i)^oct(tober)?",
        r"(?i)^nov(ember)?",
        r"(?i)^dec(ember)?",
    ];

    for (i, month) in months.iter().enumerate() {
        let re = Regex::new(&month).unwrap();
        if re.is_match(&month_name) {
            return Some(i as u32 + 1);
        }
    }

    None
}

// --------------------------------------------------
fn cap_to_dt(cap: &Captures) -> Option<DateTime<Utc>> {
    if let Some(days) = cap_to_int::<i64>(&cap, "excel") {
        Some(Utc.ymd(1904, 1, 1).and_hms(0, 0, 0) + Duration::days(days))
    }
    else {
        let hour = cap_to_int::<u32>(&cap, "hour").unwrap_or(0);
        let minutes = cap_to_int::<u32>(&cap, "minutes").unwrap_or(0);
        let seconds = cap_to_int::<u32>(&cap, "seconds").unwrap_or(0);
        let day = cap_to_int::<u32>(&cap, "day").unwrap_or(1);

        match cap_to_int::<i32>(&cap, "year") {
            Some(mut year) => {
                if year < 100 { 
                    year += 2000;
                }

                let maybe_month = cap_to_int::<u32>(&cap, "month").or_else(||
                    cap.name("month").and_then(|m| month_to_int(m.as_str()))
                );

                match maybe_month {
                    Some(month) => {
                        Some(Utc.ymd(year, month, day)
                             .and_hms(hour, minutes, seconds))
                    },
                    _ => None
                }
            },
            _ => None,
        }
    }
}

// --------------------------------------------------
// here be tests
// --------------------------------------------------

#[test]
fn fails_no_id() {
    let xml = r#"
    <?xml version="1.0" encoding="UTF-8"?>
    <SAMPLE alias="TARA_N000002741" center_name="Genoscope" accession="ERS494529">
         <IDENTIFIERS>
              <EXTERNAL_ID namespace="BioSample">SAMEA2623861</EXTERNAL_ID>
              <SUBMITTER_ID namespace="GSC">TARA_N000002741</SUBMITTER_ID>
         </IDENTIFIERS>
    </SAMPLE>
    "#;

    let root = Element::parse(xml.as_bytes()).unwrap();
    println!("{:?}", root);

    let res = parse_xml(root);
    assert!(res.is_err());
}

#[test]
fn fails_no_attributres() {
    let xml = r#"
    <?xml version="1.0" encoding="UTF-8"?>
    <SAMPLE alias="TARA_N000002741" center_name="Genoscope" accession="ERS494529">
         <IDENTIFIERS>
              <PRIMARY_ID>ERS494529</PRIMARY_ID>
              <EXTERNAL_ID namespace="BioSample">SAMEA2623861</EXTERNAL_ID>
              <SUBMITTER_ID namespace="GSC">TARA_N000002741</SUBMITTER_ID>
         </IDENTIFIERS>
         <TITLE>TARA_20120309T0859Z_151_EVENT_PUMP_P_S_(5 m)_PROT_NUC-RNA(100L)_W0.8-5_TARA_N000002741</TITLE>
         <SAMPLE_NAME>
              <TAXON_ID>408172</TAXON_ID>
              <SCIENTIFIC_NAME>marine metagenome</SCIENTIFIC_NAME>
         </SAMPLE_NAME>
    </SAMPLE>
    "#;

    let root = Element::parse(xml.as_bytes()).unwrap();
    println!("{:?}", root);

    let res = parse_xml(root);
    assert!(res.is_err());
}

#[test]
fn test_parse_datetime() {
    let vs = vec![
        "2012-03-09T08:59", 
        "2012-03-09T08:59:03",
        "2017-06-16Z",
        "2015-01",
        "2015-01/2015-02",
        "2015-01-03/2015-02-14",
        "20100910",
        "12/06",
        "2/14",
        "2/14-12/15",
        "2017-06-16Z",
        "34210",
        "Dec-2015",
        "March-2017",
        "May, 2017",
        "March-April 2017",
        "July of 2011",
        "2008 August",
    ];

    for v in vs {
        let d = parse_datetime(v);
        println!("v = {} : {:?}", v, d);
        assert!(d.is_some());
    }
}

#[test]
fn test_month_to_int() {
    assert_eq!(month_to_int("nov"), Some(11));
    assert_eq!(month_to_int("JANUARY"), Some(1));
    assert_eq!(month_to_int("Jun"), Some(6));
    assert_eq!(month_to_int("foo"), None);
}
