extern crate xmltree;
extern crate clap;

use clap::{App, Arg};
use xmltree::Element;
use std::fs::File;
use std::error::Error;

#[derive(Debug)]
pub struct Config {
    xml_file: String
}

type MyResult<T> = Result<T, Box<Error>>;

// --------------------------------------------------
pub fn run(config: Config) -> MyResult<()> {
    let f = File::open(config.xml_file)?;
    let mut root = Element::parse(f)?;
    //println!("{:?}", root);

    let id = get_primary_id(&root);
    println!("id {:?}", id);

    let runs = get_runs(&root);
    println!("runs {:?}", runs);

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
