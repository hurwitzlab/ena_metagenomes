extern crate chrono;
extern crate clap;
extern crate regex;
extern crate time;
extern crate xmltree;

use chrono::{DateTime, TimeZone, Utc}; // Date
use clap::{App, Arg};
use regex::{Captures, Regex, RegexSet};
use std::error::Error;
use std::fs::{self, File};
use std::str::FromStr;
use std::collections::HashMap;
use time::Duration;
use xmltree::Element;

#[derive(Debug)]
pub struct Config {
    input: Vec<String>,
}

#[derive(Debug)]
struct Attr {
    tag: String,
    value: String,
    units: Option<String>,
}

#[derive(Debug)]
struct PossibleDate {
    tag: String,
    value: DateTime<Utc>,
    tag_ok: bool,
}

type MyResult<T> = Result<T, Box<Error>>;
//type Latitude<f64> = Latitude<f64>;

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

    let runs = get_runs(&root);
    println!("runs {:?}", runs);

    let skip_re = Regex::new(r"^ENA-").unwrap();
    let attrs = get_attributes(&root, Some(skip_re))?;
    println!("attr {:?}", attrs);

    let dates = get_dates(&attrs);
    println!("dates {:?}", dates);

    let depth = get_depth(&attrs);
    println!("depth {:?}", depth);

    let lat_lon = get_lat_lon(&attrs);
    println!("lat_lon {:?}", lat_lon);

    Ok(())
}

// --------------------------------------------------
fn get_primary_id(root: &Element) -> MyResult<String> {
    let ids = match root.get_child("IDENTIFIERS") {
        Some(x) => x,
        _ => return Err(From::from("Missing IDENTIFIERS")),
    };

    let primary_id = match ids.get_child("PRIMARY_ID") {
        Some(pid) => pid.text.as_ref(),
        _ => return Err(From::from("Missing PRIMARY_ID node")),
    };

    let id = match primary_id {
        Some(z) => z,
        _ => return Err(From::from("Missing PRIMARY_ID value")),
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
    element.get_child(tag).and_then(|child| {
        child.text.as_ref().and_then(|val| Some(val.to_string()))
    })
}

// --------------------------------------------------
fn get_attributes(root: &Element, skip: Option<Regex>) -> MyResult<Vec<Attr>> {
    let skip_tag = |tag: &str| match &skip {
        Some(re) => re.is_match(tag),
        _ => false,
    };

    match root.get_child("SAMPLE_ATTRIBUTES") {
        Some(attributes) => {
            let mut attrs: Vec<Attr> = vec![];
            for attr in attributes.children.iter() {
                if let Some(tag) = get_child_text(&attr, "TAG") {
                    if skip_tag(&tag) {
                        continue;
                    }

                    if let Some(value) = get_child_text(&attr, "VALUE") {
                        attrs.push(Attr {
                            tag: tag,
                            value: value,
                            units: get_child_text(attr, "UNITS"),
                        });
                    }
                }
            }
            println!("attr = {:?}", attrs);
            Ok(attrs)
        }
        _ => Err(From::from("Missing SAMPLE_ATTRIBUTES")),
    }
}

// --------------------------------------------------
fn get_depth(attrs: &Vec<Attr>) -> Option<f64> {
    let tag_re = Regex::new(
        r"(?i)^(?:geographic(?:al)? location [(])?depth[)]?",
    ).unwrap();

    for attr in attrs.iter() {
        if tag_re.is_match(&attr.tag) {
            return parse_depth(&attr.value);
        }
    }

    None
}

// --------------------------------------------------
fn parse_depth(val: &str) -> Option<f64> {
    println!("VAL = {}", val);
    let patterns = vec![
        // 5, 5., 5.0
        r"(?x)
        ^
        (?P<num>\d+(?:\.(?:\d+)?)?)
        \s*
        (?P<unit>\w+)?
        $
        ",
        // .5, 0.5
        r"(?x)
        ^
        (?P<num>(?:\d+)?\.\d+)
        \s*
        (?P<unit>\w+)?
        $
        ",
    ];

    for pattern in patterns {
        let re = Regex::new(&pattern).unwrap();
        if let Some(caps) = re.captures(&val) {
            let mult = match caps.name("unit") {
                Some(unit_val) => {
                    let unit_pat = r"(?ix)
                        ^
                        (?P<prefix>c(?:enti)?|m(?:illi)?)?
                        m
                        (?:eters?)?
                        $
                        ";
                    let unit_re = Regex::new(&unit_pat).unwrap();

                    if let Some(c) = unit_re.captures(&unit_val.as_str()) {
                        if let Some(m) = c.name("prefix") {
                            match m.as_str() {
                                "c" => 0.01,
                                "centi" => 0.01,
                                "m" => 0.001,
                                "milli" => 0.001,
                                _ => 1.,
                            }
                        } else {
                            1.
                        }
                    } else {
                        1.
                    }
                }
                _ => 1.,
            };

            if let Some(num) = caps.name("num") {
                if let Ok(n) = num.as_str().parse::<f64>() {
                    return Some(n * mult);
                }
            }
        }
    }

    None
}

// --------------------------------------------------
fn get_dates(attrs: &Vec<Attr>) -> Option<Vec<PossibleDate>> {
    let tag_patterns = [
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

    // cf https://docs.rs/chrono/0.4.0/chrono/format/strftime/index.html
    let tag_re = RegexSet::new(&tag_patterns).unwrap();

    let mut dates: Vec<PossibleDate> = vec![];
    for attr in attrs.iter() {
        let val = &attr.value;
        //println!("\n\n{} = {}", attr.tag, val);

        if let Some(dt) = parse_datetime(&val) {
            //println!("DATE => {:?}", dt);
            dates.push(PossibleDate {
                tag: attr.tag.to_string(),
                value: dt,
                tag_ok: tag_re.is_match(&attr.tag),
            });
        }
    }

    //if dates.len() > 0 {
    //    let num_ok = &dates.iter().filter(|d| d.tag_ok).count() as u32;
    //    if num_ok == 1 {
    //        Some(dates.iter().filter(|d| d.tag_ok).collect())
    //    } else {
    //        Some(dates)
    //    }
    //} else {
    //    None
    //}

    Some(dates)
}

// --------------------------------------------------
fn get_lat_lon(attrs: &Vec<Attr>) -> Option<&str> {
    let tag_patterns_combined = vec![
        r"(?xi)
        ^
        lat[\s_]lon
        $
        ",
        r"(?xi)
        ^(?:geographic(?:al)? location [(])?latitude and longitude(?:[)])?
        ",
    ];

    let tag_patterns_lat = vec![
        r"(?xi)
        ^
        latitude
        $
        ",
        r"(?xi)
        ^
        (?:geographic(?:al)?
        \s+
        location
        \s+[(])?
        lat(?:itude)?[)]?
        (?:\s+start)?
        $
        ",
    ];

    let tag_patterns_lon = vec![
        r"(?xi)
        ^
        longitude
        $
        ",
        r"(?xi)
        ^
        (?:geographic(?:al)?
        \s+
        location
        \s+[(])?
        lon(?:gitude)?[)]? 
        (?:\s+start)?
        $
        ",
        r"(?xi)
        ^longitude(?:_deg|\s+start)?$
        ",
    ];

    fn re_fct(v: &Vec<&str>) -> Vec<Regex> {
        v.into_iter().map(|p| Regex::new(p).unwrap()).collect()
    }

    let mut pat_map: HashMap<String, Vec<Regex>> = HashMap::new();
    pat_map.insert("combined".to_string(), re_fct(&tag_patterns_combined));
    pat_map.insert("lat".to_string(), re_fct(&tag_patterns_lat));
    pat_map.insert("lon".to_string(), re_fct(&tag_patterns_lon));

    let classify_tag = |tag: &str| {
        for (class, tag_regexes) in pat_map.iter() {
            for tag_re in tag_regexes {
                if tag_re.is_match(&tag) {
                    return Some(class);
                }
            }
        }
        None
    };

    for attr in attrs.iter() {
        if let Some(class) = classify_tag(&attr.tag) {
            println!("tag \"{}\" = {}", &attr.tag, &class);

            //let parser = match class {

            //};
        }
    }

    None
}

// --------------------------------------------------
fn dms2decimal(degree: &str, min: &str, sec: &str, negate: &bool) -> Option<String> {
    if let Ok(degree) = degree.parse::<f64>() {
        if let Ok(min) = min.parse::<f64>() {
            if let Ok(sec) = sec.parse::<f64>() {
                return Some(format!("{:.5}", degree + (min/60.0) + (sec/3600.0)));
            }
        }
    }
    None
}

// --------------------------------------------------
fn parse_lat_lon_combined(val: &str) -> Option<(String, String)> {
    // 41º40,13.5''N 2º48'00.6''E
    let p1 = r"(?x)
        ^
        \s*
        (?P<lat_degrees>\d+)
        [ºÁ]?
        (?P<lat_minutes>\d+)
        [,’']
        (?P<lat_seconds>\d+(?:\.\d+)?)
        ['’]{2}?
        (?:[\s']*(?P<lat_direction>[NS]))?
        (?:\s+|\s*,\s*)
        (?P<lon_degrees>\d+)
        [ºÁ]?
        (?P<lon_minutes>\d+)
        [,’']
        (?P<lon_seconds>\d+(?:\.\d+)?)
        ['’]{2}?
        (?:[\s']*(?P<lon_direction>[EW]))?
        $
        ";

    println!("p1 = {}", p1);
    println!("val = {}", val);
    let re1 = Regex::new(p1).unwrap();

    let (lat, lon) = if let Some(caps) = re1.captures(&val) {
        println!("caps = {:?}", caps);

        let lat1 = if let Some(deg) = caps.name("lat_degrees") {
            if let Some(min) = caps.name("lat_minutes") {
                if let Some(sec) = caps.name("lat_seconds") {
                    let negate = match caps.name("lat_direction") {
                        Some(d) => match d.as_str() {
                            "S" | "s" => true,
                            _ => false,
                        }
                        _ => false
                    };

                    dms2decimal(&deg.as_str(), 
                                &min.as_str(), 
                                &sec.as_str(), 
                                &negate)
                }
                else {
                    None
                }
            }
            else {
                None
            }
        }
        else {
            None
        };

        let lon1 = if let Some(deg) = caps.name("lon_degrees") {
            if let Some(min) = caps.name("lon_minutes") {
                if let Some(sec) = caps.name("lon_seconds") {
                    let negate = match caps.name("lon_direction") {
                        Some(d) => match d.as_str() {
                            "W" | "w" => true,
                            _ => false,
                        }
                        _ => false
                    };
                    dms2decimal(&deg.as_str(), 
                                &min.as_str(), 
                                &sec.as_str(),
                                &negate)
                }
                else {
                    None
                }
            }
            else {
                None
            }
        }
        else {
            None
        };

        (lat1, lon1)
    }
    else {
        (None, None)
    };

    println!("LAT = {:?}", lat);
    println!("LON = {:?}", lon);

    match (lat, lon) {
        (Some(n), Some(m)) => Some((n, m)),
        _ => None,
    }

//    let patterns = vec![
//        r"(?x)
//        ^
//        (?:lat:?\s*)?
//        ($RE{'num'}{'real'})
//        (?:\s*([NS]))?
//        (?:_|\s+|\s*,\s*)
//        (?:long:?\s*)?
//        ($RE{'num'}{'real'})
//        (?:\s*([EW]))?
//        (?:,\s+decimal\s+degrees)?
//        $
//        ",
//    ];

//    for pattern in patterns {
//        let re = Regex::new(pattern).unwrap();
//    }
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
                return Some(dt);
            }
        }
    }

    None
}

// --------------------------------------------------
fn cap_to_int<T: FromStr>(cap: &Captures, name: &str) -> Option<T> {
    match cap.name(name) {
        Some(val) => match val.as_str().parse::<T>() {
            Ok(i) => Some(i),
            _ => None,
        },
        _ => None,
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
    } else {
        let hour = cap_to_int::<u32>(&cap, "hour").unwrap_or(0);
        let minutes = cap_to_int::<u32>(&cap, "minutes").unwrap_or(0);
        let seconds = cap_to_int::<u32>(&cap, "seconds").unwrap_or(0);
        let day = cap_to_int::<u32>(&cap, "day").unwrap_or(1);

        match cap_to_int::<i32>(&cap, "year") {
            Some(mut year) => {
                if year < 100 {
                    year += 2000;
                }

                let maybe_month = cap_to_int::<u32>(&cap, "month").or_else(
                    || cap.name("month").and_then(|m| month_to_int(m.as_str())),
                );

                match maybe_month {
                    Some(month) => Some(
                        Utc.ymd(year, month, day)
                            .and_hms(hour, minutes, seconds),
                    ),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

// --------------------------------------------------
// HERE BE TESTS
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

// --------------------------------------------------
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

// --------------------------------------------------
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

// --------------------------------------------------
#[test]
fn test_month_to_int() {
    assert_eq!(month_to_int("nov"), Some(11));
    assert_eq!(month_to_int("JANUARY"), Some(1));
    assert_eq!(month_to_int("Jun"), Some(6));
    assert_eq!(month_to_int("foo"), None);
}

// --------------------------------------------------
#[test]
fn test_parse_depth() {
    assert_eq!(parse_depth("abc"), None);
    assert_eq!(parse_depth("5"), Some(5.));
    assert_eq!(parse_depth("5.0"), Some(5.));
    assert_eq!(parse_depth("5 m"), Some(5.));
    assert_eq!(parse_depth(".5 meter"), Some(0.5));
    assert_eq!(parse_depth("0.5 meters"), Some(0.5));
    assert_eq!(parse_depth("5meters"), Some(5.));
    assert_eq!(parse_depth("5m"), Some(5.));
    assert_eq!(parse_depth("5 cm"), Some(0.05));
    assert_eq!(parse_depth("5cm"), Some(0.05));
    assert_eq!(parse_depth("5. centimeters"), Some(0.05));
    assert_eq!(parse_depth("5centimeters"), Some(0.05));
    assert_eq!(parse_depth("5 mm"), Some(0.005));
    assert_eq!(parse_depth("5mm"), Some(0.005));
    assert_eq!(parse_depth("5 millimeter"), Some(0.005));
    assert_eq!(parse_depth("0.005m"), Some(0.005));
    assert_eq!(parse_depth("5millimeters"), Some(0.005));
}

// --------------------------------------------------
#[test]
fn test_dms2decimal() {
    assert_eq!(dms2decimal("41", "40", "13.5", &false), Some("41.67042".to_string()));
    assert_eq!(dms2decimal("2", "48", "0.6", &false), Some("2.80017".to_string()));
}

// --------------------------------------------------
#[test]
fn test_parse_lat_lon_combined() {
    assert_eq!(parse_lat_lon_combined("41º40,13.5''N 2º48'00.6''E"), 
        Some(("41.67042".to_string(), "2.80017".to_string())));
}
